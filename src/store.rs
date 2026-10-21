use crate::{
    INTERVAL,
    model::{Data, ErrorCode, FetchError, Output, State, instant, render},
};
use rustix::fs::{FlockOperation, flock};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
};

pub fn load(path: &Path) -> State {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<State>(&bytes) {
            Ok(state)
                if state.attempt_at.is_finite()
                    && state.next_request_at.is_finite()
                    && state.data.as_ref().is_none_or(|d| d.fetched_at.is_finite()) =>
            {
                state
            }
            _ => State::local_error(),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => State::default(),
        Err(_) => State::local_error(),
    }
}
fn save(path: &Path, state: &State) -> io::Result<()> {
    let mut file = tempfile::NamedTempFile::new_in(path.parent().expect("cache parent"))?;
    serde_json::to_writer(&mut file, state)?;
    file.write_all(b"\n")?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}
pub fn status_with<F>(
    home: &Path,
    cache: &Path,
    now: f64,
    force: bool,
    fetcher: F,
) -> io::Result<Output>
where
    F: FnOnce(&Path, f64) -> Result<Data, FetchError>,
{
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(cache)?;
    let path = cache.join("status.json");
    let lock: File = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(cache.join("fetch.lock"))?;
    match flock(&lock, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => {}
        Err(rustix::io::Errno::WOULDBLOCK) => return Ok(render(&load(&path), now, true)),
        Err(error) => return Err(error.into()),
    }
    let mut state = load(&path);
    let last = state.attempt_at;
    let mut due = state.next_request_at <= now;
    if force && state.error != Some(ErrorCode::RateLimit) {
        due = now >= last + 5.0;
    }
    if !state.data.as_ref().is_some_and(|d| d.valid_today(now)) && now >= last + 5.0 {
        due = due || state.error.is_none();
    }
    if due {
        state.attempt_at = now;
        state.next_request_at = now + INTERVAL;
        match fetcher(home, now) {
            Ok(data) => {
                let regresses = state
                    .data
                    .as_ref()
                    .filter(|d| d.valid_today(now))
                    .and_then(|previous| {
                        instant(previous.cached_at.as_deref()?)
                            .zip(instant(data.cached_at.as_deref()?))
                    })
                    .is_some_and(|(previous, current)| current < previous);
                if regresses {
                    state.error = Some(ErrorCode::Pending);
                } else {
                    state.data = Some(data);
                    state.error = None;
                }
            }
            Err(error) => {
                state.error = Some(error.code);
                state.next_request_at = (now + INTERVAL).max(error.retry_at);
            }
        }
        if save(&path, &state).is_err() {
            state.error = Some(ErrorCode::Local);
        }
    }
    Ok(render(&state, now, false))
}
