use std::{
    env,
    path::PathBuf,
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};
use waybar_wakatime::{
    actions, client,
    model::{State, render},
    store,
};

fn run() -> Result<i32, &'static str> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() > 1 {
        return Err("usage: waybar-wakatime [status|refresh|dashboard|settings]");
    }
    let action = match args.first() {
        None => "status",
        Some(s) => s.to_str().ok_or("Invalid action")?,
    };
    match action {
        "--help" | "-h" => {
            println!("usage: waybar-wakatime [status|refresh|dashboard|settings]");
            return Ok(0);
        }
        "--version" | "-V" => {
            println!("waybar-wakatime {}", env!("CARGO_PKG_VERSION"));
            return Ok(0);
        }
        "dashboard" => {
            return actions::dashboard().map_err(|_| "Could not open WakaTime Dashboard");
        }
        "settings" => {
            return actions::open_settings(
                &env::var_os("WAKATIME_DESKTOP_EXECUTABLE")
                    .unwrap_or_else(|| "wakatime-desktop".into()),
            )
            .map_err(|_| "Could not open WakaTime Desktop settings");
        }
        "status" | "refresh" => {}
        _ => return Err("usage: waybar-wakatime [status|refresh|dashboard|settings]"),
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Could not read the clock")?
        .as_secs_f64();
    let output = if let Some(home) = env::var_os("HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
    {
        let cache = env::var_os("XDG_CACHE_HOME")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".cache"))
            .join("waybar-wakatime");
        store::status_with(&home, &cache, now, action == "refresh", client::fetch)
            .unwrap_or_else(|_| render(&State::local_error(), now, false))
    } else {
        render(&State::local_error(), now, false)
    };
    println!(
        "{}",
        serde_json::to_string(&output).map_err(|_| "Could not render WakaTime status")?
    );
    if action == "refresh" {
        actions::notify_waybar().map_err(|_| "Could not refresh Waybar")?;
    }
    Ok(0)
}
fn main() -> ExitCode {
    rustix::process::umask(rustix::fs::Mode::from_bits_truncate(0o077));
    match run() {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
