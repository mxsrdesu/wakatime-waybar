use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command, time::Duration};
use waybar_wakatime::actions::settings_with_timeout;
fn script(path: &Path, body: &str) {
    let sh = Command::new("sh")
        .args(["-c", "command -v sh"])
        .output()
        .unwrap();
    let sh = String::from_utf8(sh.stdout).unwrap();
    fs::write(path, format!("#!{}\n{body}\n", sh.trim())).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
fn cmd(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_waybar-wakatime"));
    cmd.env("HOME", home)
        .env("XDG_CACHE_HOME", home.join("cache"));
    cmd
}
#[test]
fn missing_key_has_honest_json_and_private_cache() {
    let d = tempfile::tempdir().unwrap();
    let output = cmd(d.path()).output().unwrap();
    assert!(output.status.success());
    let data: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(data["text"], "—");
    assert_eq!(data["class"], json!(["error"]));
    assert!(data["tooltip"].as_str().unwrap().contains("API key"));
    assert!(output.stderr.is_empty());
    let state = fs::read_to_string(d.path().join("cache/waybar-wakatime/status.json")).unwrap();
    assert!(state.contains("configuration"));
    assert!(!state.contains("api_key"));
}
#[test]
fn process_restart_reads_existing_python_cache_schema() {
    let d = tempfile::tempdir().unwrap();
    let c = d.path().join("cache/waybar-wakatime");
    fs::create_dir_all(&c).unwrap();
    let now = chrono::Utc::now();
    let state = json!({"data":{"date":now.format("%Y-%m-%d").to_string(),"timezone":"UTC","fetched_at":now.timestamp(),"total_seconds":960},"attempt_at":now.timestamp(),"next_request_at":now.timestamp()+120});
    fs::write(c.join("status.json"), state.to_string()).unwrap();
    for _ in 0..2 {
        let output = cmd(d.path()).output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap()["text"],
            "16m"
        );
    }
}
#[test]
fn menu_actions_run_commands_and_refresh_targets_only_user_waybar() {
    let d = tempfile::tempdir().unwrap();
    let log = d.path().join("actions");
    for name in ["xdg-open", "pkill", "desktop"] {
        script(
            &d.path().join(name),
            &format!("printf '%s\\n' \"{name}:$*\" >> '{}'", log.display()),
        );
    }
    let mut dashboard = cmd(d.path());
    dashboard.env("PATH", d.path()).arg("dashboard");
    assert!(dashboard.status().unwrap().success());
    let mut settings = cmd(d.path());
    settings
        .env("WAKATIME_DESKTOP_EXECUTABLE", d.path().join("desktop"))
        .arg("settings");
    assert!(settings.status().unwrap().success());
    let out = cmd(d.path())
        .env("PATH", d.path())
        .arg("refresh")
        .output()
        .unwrap();
    assert!(out.status.success());
    let log = fs::read_to_string(log).unwrap();
    assert!(log.contains("xdg-open:https://wakatime.com/dashboard"));
    assert!(log.contains("desktop:wakatime://settings/"));
    assert!(log.contains(&format!(
        "pkill:-RTMIN+8 -u {} -x waybar",
        rustix::process::getuid().as_raw()
    )));
}
#[test]
fn settings_delivers_second_invocation_only_for_cold_start() {
    let d = tempfile::tempdir().unwrap();
    let exe = d.path().join("desktop");
    let marker = d.path().join("started");
    let log = d.path().join("calls");
    script(
        &exe,
        &format!(
            "printf '%s\\n' \"$*\" >> '{}'\nif test ! -e '{}'; then : > '{}'; exec /bin/sh -c 'sleep 0.2'; fi",
            log.display(),
            marker.display(),
            marker.display()
        ),
    );
    // Use a real sleep executable discovered from the inherited PATH.
    let sleep = Command::new("sh")
        .args(["-c", "command -v sleep"])
        .output()
        .unwrap();
    let sleep = String::from_utf8(sleep.stdout).unwrap();
    let body = fs::read_to_string(&exe).unwrap().replace(
        "exec /bin/sh -c 'sleep 0.2'",
        &format!("exec '{}' 0.2", sleep.trim()),
    );
    fs::write(&exe, body).unwrap();
    assert_eq!(
        settings_with_timeout(
            exe.as_os_str(),
            Duration::from_millis(40),
            Duration::from_secs(1)
        )
        .unwrap(),
        0
    );
    assert_eq!(fs::read_to_string(&log).unwrap().lines().count(), 2);
    fs::write(&log, "").unwrap();
    assert_eq!(
        settings_with_timeout(
            exe.as_os_str(),
            Duration::from_secs(1),
            Duration::from_secs(1)
        )
        .unwrap(),
        0
    );
    assert_eq!(fs::read_to_string(log).unwrap().lines().count(), 1);
}
#[test]
fn invalid_action_and_missing_openers_fail() {
    let d = tempfile::tempdir().unwrap();
    assert!(
        !cmd(d.path())
            .arg("unknown")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !cmd(d.path())
            .args(["status", "extra"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !cmd(d.path())
            .arg("settings")
            .env("WAKATIME_DESKTOP_EXECUTABLE", d.path().join("missing"))
            .output()
            .unwrap()
            .status
            .success()
    );
}
