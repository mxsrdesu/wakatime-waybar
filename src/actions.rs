use crate::SIGNAL;
use std::{
    io,
    os::unix::process::CommandExt,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn wait(child: &mut Child, timeout: Duration) -> io::Result<Option<i32>> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status.code().unwrap_or(1)));
        }
        if start.elapsed() >= timeout {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(20));
    }
}
pub fn open_settings(executable: &std::ffi::OsStr) -> io::Result<i32> {
    settings_with_timeout(executable, Duration::from_secs(2), Duration::from_secs(8))
}
pub fn settings_with_timeout(
    executable: &std::ffi::OsStr,
    initial: Duration,
    delivery: Duration,
) -> io::Result<i32> {
    let spawn = || {
        Command::new(executable)
            .arg("wakatime://settings/")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
    };
    let mut first = spawn()?;
    if let Some(code) = wait(&mut first, initial)? {
        return Ok(code);
    }
    // Desktop handles cold-start deep links in its second-instance listener.
    let mut second = spawn()?;
    match wait(&mut second, delivery)? {
        Some(code) => Ok(code),
        None => {
            let _ = second.kill();
            let _ = second.wait();
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Desktop settings timed out",
            ))
        }
    }
}
pub fn dashboard() -> io::Result<i32> {
    Ok(Command::new("xdg-open")
        .arg("https://wakatime.com/dashboard")
        .status()?
        .code()
        .unwrap_or(1))
}
pub fn notify_waybar() -> io::Result<()> {
    let status = Command::new("pkill")
        .args([
            format!("-RTMIN+{SIGNAL}"),
            "-u".into(),
            rustix::process::getuid().as_raw().to_string(),
            "-x".into(),
            "waybar".into(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    match status.code() {
        Some(0 | 1) => Ok(()),
        _ => Err(io::Error::other("Could not refresh Waybar")),
    }
}
