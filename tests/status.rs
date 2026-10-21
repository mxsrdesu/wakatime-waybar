use chrono::DateTime;
use rustix::fs::{FlockOperation, flock};
use serde_json::{Value, json};
use std::{cell::Cell, fs, os::unix::fs::PermissionsExt};
use waybar_wakatime::{
    client::retry_at,
    model::{Data, ErrorCode, FetchError, State, parse_response, render},
    store::{load, status_with},
};
fn now() -> f64 {
    DateTime::parse_from_rfc3339("2026-10-19T14:00:00Z")
        .unwrap()
        .timestamp() as f64
}
fn payload(seconds: Value) -> Value {
    json!({"cached_at":"2026-10-19T13:59:00Z", "data": {"range":{"date":"2026-10-19", "timezone":"Asia/Tbilisi"},
        "grand_total":{"total_seconds":seconds}, "projects":[{"name":"private-project-must-not-be-cached"}]}})
}
fn data(at: f64) -> Result<Data, FetchError> {
    parse_response(&payload(json!(26162.608)), at)
}
#[test]
fn duration_and_server_zero() {
    for (seconds, expected) in [
        (0.0, "0m"),
        (59.9, "0m"),
        (960.0, "16m"),
        (3600.0, "1h 0m"),
        (26162.608, "7h 16m"),
    ] {
        let d = parse_response(&payload(json!(seconds)), now()).unwrap();
        let out = render(
            &State {
                data: Some(d),
                ..State::default()
            },
            now(),
            false,
        );
        assert_eq!(out.text, expected);
        assert_eq!(out.class, ["ready"]);
    }
}
#[test]
fn unknown_or_old_is_not_zero() {
    let mut old = payload(json!(0));
    old["data"]["range"]["date"] = json!("2026-10-18");
    let pending = json!({"data":{"range":{"timezone":"UTC"},"grand_total":{"total_seconds":0}}});
    for value in [old, pending] {
        assert_eq!(
            parse_response(&value, now()).unwrap_err().code,
            ErrorCode::Pending
        );
    }
    for seconds in [json!(-1), json!(true), json!("12"), Value::Null] {
        assert_eq!(
            parse_response(&payload(seconds), now()).unwrap_err().code,
            ErrorCode::Response
        );
    }
    assert_eq!(
        parse_response(&json!({}), now()).unwrap_err().code,
        ErrorCode::Response
    );
}
#[test]
fn account_timezone_controls_midnight() {
    let mut p = payload(json!(0));
    p["data"]["range"]["timezone"] = json!("Pacific/Auckland");
    assert_eq!(
        parse_response(&p, now()).unwrap_err().code,
        ErrorCode::Pending
    );
    p["data"]["range"]["date"] = json!("2026-10-20");
    assert!(parse_response(&p, now()).is_ok());
    p["data"]["range"]["timezone"] = json!("unknown");
    assert_eq!(
        parse_response(&p, now()).unwrap_err().code,
        ErrorCode::Pending
    );
}
#[test]
fn shared_cache_survives_restart_without_private_payload() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let calls = Cell::new(0);
    let fetch = |_: &std::path::Path, at| {
        calls.set(calls.get() + 1);
        data(at)
    };
    let first = status_with(dir.path(), &cache, now(), false, fetch).unwrap();
    let second = status_with(dir.path(), &cache, now() + 30.0, false, fetch).unwrap();
    assert_eq!(first.text, second.text);
    assert_eq!(calls.get(), 1);
    let raw = fs::read_to_string(cache.join("status.json")).unwrap();
    assert!(!raw.contains("private-project"));
    assert!(!raw.contains("api_key"));
    assert_eq!(
        fs::metadata(cache.join("status.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&cache).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        load(&cache.join("status.json")).data.unwrap().total_seconds,
        26162.608
    );
}
#[test]
fn offline_retains_today_but_not_yesterday_and_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let c = dir.path().join("cache");
    status_with(dir.path(), &c, now(), false, |_, at| data(at)).unwrap();
    let fail = |_: &std::path::Path, _| Err(ErrorCode::Network.into());
    let out = status_with(dir.path(), &c, now() + 121.0, false, fail).unwrap();
    assert_eq!(out.text, "7h 16m !");
    assert_eq!(out.class, ["stale"]);
    assert!(out.tooltip.contains("network error"));
    let out = status_with(dir.path(), &c, now() + 130.0, true, |_, at| data(at)).unwrap();
    assert_eq!(out.text, "7h 16m");
    assert_eq!(out.class, ["ready"]);
    let midnight = DateTime::parse_from_rfc3339("2026-10-19T20:01:00Z")
        .unwrap()
        .timestamp() as f64;
    let out = status_with(dir.path(), &c, midnight, false, fail).unwrap();
    assert_eq!(out.text, "—");
    assert_eq!(out.class, ["error"]);
}
#[test]
fn lock_is_nonblocking_and_does_not_duplicate_fetch() {
    let dir = tempfile::tempdir().unwrap();
    let c = dir.path().join("cache");
    status_with(dir.path(), &c, now(), false, |_, at| data(at)).unwrap();
    let lock = fs::OpenOptions::new()
        .append(true)
        .open(c.join("fetch.lock"))
        .unwrap();
    flock(&lock, FlockOperation::NonBlockingLockExclusive).unwrap();
    let out = status_with(dir.path(), &c, now() + 130.0, false, |_, _| {
        panic!("duplicate fetch")
    })
    .unwrap();
    assert_eq!(out.text, "7h 16m");
    assert!(out.tooltip.contains("Updating"));
}
#[test]
fn refresh_coalesces_clicks_and_respects_rate_limit() {
    let dir = tempfile::tempdir().unwrap();
    let c = dir.path().join("cache");
    status_with(dir.path(), &c, now(), false, |_, at| data(at)).unwrap();
    status_with(dir.path(), &c, now() + 3.0, true, |_, _| {
        panic!("rapid click")
    })
    .unwrap();
    status_with(dir.path(), &c, now() + 10.0, true, |_, at| data(at)).unwrap();
    status_with(dir.path(), &c, now() + 140.0, false, |_, _| {
        Err(FetchError {
            code: ErrorCode::RateLimit,
            retry_at: now() + 1000.0,
        })
    })
    .unwrap();
    status_with(dir.path(), &c, now() + 150.0, true, |_, _| {
        panic!("rate-limited click")
    })
    .unwrap();
    assert_eq!(retry_at("900", now()), now() + 900.0);
    assert_eq!(
        retry_at("Mon, 19 Oct 2026 14:15:00 GMT", now()),
        now() + 900.0
    );
    assert_eq!(retry_at("invalid", now()), now() + 120.0);
}
#[test]
fn server_cache_cannot_regress() {
    let dir = tempfile::tempdir().unwrap();
    let c = dir.path().join("cache");
    status_with(dir.path(), &c, now(), false, |_, at| data(at)).unwrap();
    let out = status_with(dir.path(), &c, now() + 121.0, false, |_, at| {
        let mut p = payload(json!(100));
        p["cached_at"] = json!("2026-10-19T13:00:00Z");
        parse_response(&p, at)
    })
    .unwrap();
    assert_eq!(out.text, "7h 16m !");
    assert_eq!(out.class, ["stale"]);
}
#[test]
fn malformed_cache_recovers_and_unwritable_cache_reports_error() {
    let dir = tempfile::tempdir().unwrap();
    let c = dir.path().join("cache");
    fs::create_dir(&c).unwrap();
    for raw in ["not-json", "[]", r#"{"data":{}}"#, r#"{"attempt_at":true}"#] {
        fs::write(c.join("status.json"), raw).unwrap();
        assert_eq!(
            status_with(dir.path(), &c, now(), false, |_, at| data(at))
                .unwrap()
                .text,
            "7h 16m"
        );
    }
    fs::remove_file(c.join("status.json")).unwrap();
    fs::create_dir(c.join("status.json")).unwrap();
    let out = status_with(dir.path(), &c, now(), false, |_, at| data(at)).unwrap();
    assert_eq!(out.class, ["stale"]);
    assert!(out.tooltip.contains("local cache"));
}
#[test]
fn fixed_errors_and_styling_are_external() {
    let out = render(
        &State {
            data: Some(data(now()).unwrap()),
            error: Some(ErrorCode::Network),
            ..State::default()
        },
        now(),
        false,
    );
    assert_eq!(out.text, "7h 16m !");
    assert!(!out.text.contains("span"));
    assert!(!out.tooltip.contains("Click"));
    let out = render(
        &State {
            error: Some(ErrorCode::Pending),
            ..State::default()
        },
        now(),
        false,
    );
    assert_eq!(out.class, ["loading"]);
    assert_eq!(out.text, "—");
}
