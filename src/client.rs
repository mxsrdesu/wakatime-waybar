use crate::{
    INTERVAL, TIMEOUT,
    model::{Data, ErrorCode, FetchError, parse_response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::blocking::Client;
use std::{io::Read, path::Path, time::Duration};

const URL: &str = "https://api.wakatime.com/api/v1/users/current/status_bar/today";
const MAX_RESPONSE: u64 = 1_048_576;

fn read_key(home: &Path) -> Result<String, FetchError> {
    let config = ini::Ini::load_from_file_opt(
        home.join(".wakatime.cfg"),
        ini::ParseOption {
            enabled_quote: false,
            enabled_escape: false,
        },
    )
    .map_err(|_| FetchError::from(ErrorCode::Configuration))?;
    // Match ConfigParser's strict duplicate handling and literal values.
    let mut sections = std::collections::HashSet::new();
    for (name, values) in &config {
        if name.is_none() && values.iter().next().is_some() {
            return Err(ErrorCode::Configuration.into());
        }
        if !sections.insert(name) {
            return Err(ErrorCode::Configuration.into());
        }
        let mut keys = std::collections::HashSet::new();
        for (key, _) in values.iter() {
            if !keys.insert(key.to_lowercase()) {
                return Err(ErrorCode::Configuration.into());
            }
        }
    }
    let section = config
        .section(Some("settings"))
        .ok_or(FetchError::from(ErrorCode::Configuration))?;
    let key_in = |section: &ini::Properties| {
        section
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("api_key"))
            .map(|(_, v)| v.to_owned())
    };
    let key = key_in(section)
        .or_else(|| config.section(Some("DEFAULT")).and_then(key_in))
        .map(|value| value.trim().to_owned())
        .filter(|s| !s.is_empty())
        .ok_or(FetchError::from(ErrorCode::Configuration))?;
    Ok(key)
}
pub fn retry_at(value: &str, now: f64) -> f64 {
    let at = value
        .parse::<i64>()
        .ok()
        .map(|n| now + n as f64)
        .or_else(|| {
            httpdate::parse_http_date(value)
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs_f64())
        })
        .unwrap_or(0.0);
    (now + INTERVAL).max(at)
}
pub fn fetch(home: &Path, now: f64) -> Result<Data, FetchError> {
    fetch_url(home, now, URL)
}
fn fetch_url(home: &Path, now: f64, url: &str) -> Result<Data, FetchError> {
    let key = read_key(home)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(TIMEOUT))
        .build()
        .map_err(|_| FetchError::from(ErrorCode::Network))?;
    let response = client
        .get(url)
        .header(
            "Authorization",
            format!("Basic {}", STANDARD.encode(key.as_bytes())),
        )
        .header("User-Agent", "Waybar-WakaTime/1.0.1")
        .send()
        .map_err(|_| FetchError::from(ErrorCode::Network))?;
    match response.status().as_u16() {
        200..=299 => {}
        401 | 403 => return Err(ErrorCode::Authorization.into()),
        429 => {
            return Err(FetchError {
                code: ErrorCode::RateLimit,
                retry_at: retry_at(
                    response
                        .headers()
                        .get("Retry-After")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or(""),
                    now,
                ),
            });
        }
        _ => return Err(ErrorCode::Server.into()),
    }
    let mut raw = Vec::new();
    response
        .take(MAX_RESPONSE + 1)
        .read_to_end(&mut raw)
        .map_err(|_| FetchError::from(ErrorCode::Network))?;
    if raw.len() as u64 > MAX_RESPONSE {
        return Err(ErrorCode::Response.into());
    }
    let value = serde_json::from_slice(&raw).map_err(|_| FetchError::from(ErrorCode::Response))?;
    parse_response(&value, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    fn request(response: String) -> (Result<Data, FetchError>, String) {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(
            home.path().join(".wakatime.cfg"),
            "[settings]\napi_key = private-test-key%literal\n",
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/today", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut b = [0];
                stream.read_exact(&mut b).unwrap();
                request.push(b[0]);
                if request.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8(request).unwrap()
        });
        let data = fetch_url(home.path(), 1792418400.0, &url);
        (data, server.join().unwrap())
    }
    #[test]
    fn http_auth_and_real_response_decoding() {
        let body = r#"{"cached_at":"2026-10-19T13:59:00Z","data":{"range":{"date":"2026-10-19","timezone":"Asia/Tbilisi"},"grand_total":{"total_seconds":26162.608}}}"#;
        let (result, raw) = request(format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ));
        assert_eq!(result.unwrap().total_seconds, 26162.608);
        assert!(
            raw.to_lowercase().contains(
                &format!(
                    "authorization: basic {}",
                    STANDARD.encode("private-test-key%literal")
                )
                .to_lowercase()
            )
        );
    }
    #[test]
    fn http_failures_and_rate_limit() {
        for (status, expected) in [
            (401, ErrorCode::Authorization),
            (403, ErrorCode::Authorization),
            (500, ErrorCode::Server),
            (429, ErrorCode::RateLimit),
        ] {
            let (result, _) = request(format!(
                "HTTP/1.1 {status} Error\r\nRetry-After: 900\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            ));
            let error = result.unwrap_err();
            assert_eq!(error.code, expected);
            if status == 429 {
                assert_eq!(error.retry_at, 1792419300.0);
            }
        }
    }
    #[test]
    fn credentials_are_literal_and_duplicate_config_is_rejected() {
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join(".wakatime.cfg");
        for raw in [
            "[settings]\napi_key = 'literal%value\\path'\n",
            "[DEFAULT]\napi_key = 'literal%value\\path'\n[settings]\n",
        ] {
            std::fs::write(&file, raw).unwrap();
            assert_eq!(read_key(home.path()).unwrap(), "'literal%value\\path'");
        }
        for raw in [
            "[settings]\napi_key=one\nAPI_KEY=two\n",
            "[settings]\napi_key=one\n[settings]\napi_key=two\n",
            "[Settings]\napi_key=one\n",
            "[settings]\napi_key=\n",
        ] {
            std::fs::write(&file, raw).unwrap();
            assert_eq!(
                read_key(home.path()).unwrap_err().code,
                ErrorCode::Configuration
            );
        }
    }
    #[test]
    fn missing_config_and_bad_json() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(
            fetch(home.path(), 0.0).unwrap_err().code,
            ErrorCode::Configuration
        );
        assert_eq!(
            request("HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nfoo".into())
                .0
                .unwrap_err()
                .code,
            ErrorCode::Response
        );
    }
}
