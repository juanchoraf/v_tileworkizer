use crate::{
    config::{self, Config},
    platform, workspace,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn state_dir() -> Result<PathBuf> {
    // Separate X servers / Windows sessions must never compete for a worker lock.
    let session = std::env::var("WAYLAND_DISPLAY")
        .or_else(|_| std::env::var("DISPLAY"))
        .or_else(|_| std::env::var("SESSIONNAME"))
        .unwrap_or_else(|_| "desktop".into());
    let session: String = session
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let path = config::directory()?.join(session);
    fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn lock(name: &str) -> Result<Option<File>> {
    let dir = if name == "config" || name == "update" {
        config::directory()?
    } else {
        state_dir()?
    };
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(v_concat::v_concat!("{name}.lock")))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(e.into()),
    }
}

pub fn start() -> Result<()> {
    let _startup = lock("startup")?.context("Service startup already in progress; retry")?;
    if lock("supervisor")?.is_none() || lock("worker")?.is_none() {
        // A supervisor may be starting its child. Give it time to publish identity.
        for _ in 0..10 {
            if let Ok(bytes) = fs::read(state_dir()?.join("status.json"))
                && let Ok(status) = serde_json::from_slice::<Status>(&bytes)
            {
                if status.build_id == crate::service_identity::current()? {
                    return Ok(());
                }
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        stop().context("Replace the older desktop service")?;
    }
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--supervise")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe extern "C" {
            fn setsid() -> i32;
        }
        // SAFETY: setsid is async-signal-safe; the closure allocates nothing before exec.
        unsafe {
            command.pre_exec(|| {
                if setsid() < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000 | 0x00000200); // No console, new process group.
    }
    command.spawn().context("Start the session supervisor")?;
    Ok(())
}

pub fn supervise() -> Result<()> {
    let Some(_lock) = lock("supervisor")? else {
        return Ok(());
    };
    let executable = std::env::current_exe()?;
    while executable.exists() {
        if take_stop()? {
            break;
        }
        let mut child = Command::new(&executable);
        child
            .arg("--service")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            child.creation_flags(0x08000000);
        }
        match child.status() {
            Ok(status) if status.success() => break,
            Ok(_) => {}
            Err(e) => {
                write_status(0, &v_concat::v_concat!("Worker failed: {e}"))?;
            }
        }
        thread::sleep(Duration::from_secs(5));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct Status {
    pub pid: u32,
    #[serde(default)]
    pub build_id: String,
    pub timestamp: u64,
    pub windows: usize,
    pub message: String,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn write_status(windows: usize, message: &str) -> Result<()> {
    let value = Status {
        pid: std::process::id(),
        build_id: crate::service_identity::current()?.to_owned(),
        timestamp: now(),
        windows,
        message: message.into(),
    };
    config::atomic_write(
        &state_dir()?.join("status.json"),
        &serde_json::to_vec(&value)?,
    )
}

pub fn status_text() -> Result<String> {
    if lock("worker")?.is_some() {
        return Ok(v_concat::v_concat!(
            "Worker stopped\nConfig: {}",
            config::path()?.display()
        ));
    }
    let value = match fs::read(state_dir()?.join("status.json")) {
        Ok(bytes) => {
            let status: Status = serde_json::from_slice(&bytes)?;
            if now().saturating_sub(status.timestamp) > 65 {
                "Worker is not responding".into()
            } else {
                v_concat::v_concat!(
                    "{} · {} windows · PID {}",
                    status.message,
                    status.windows,
                    status.pid
                )
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "Worker has not started".into(),
        Err(e) => return Err(e.into()),
    };
    Ok(v_concat::v_concat!(
        "{value}\nConfig: {}",
        config::path()?.display()
    ))
}

pub fn request_tile() -> Result<()> {
    request_display_tile(None)
}

pub fn request_display_tile(display: Option<crate::layout::Rect>) -> Result<()> {
    start()?;
    let _command = lock("display-command")?.context("Display command is busy; retry")?;
    crate::display_pause::resume(display)?;
    take_request("disengage.request")?;
    config::atomic_write(
        &state_dir()?.join("tile.request"),
        &serde_json::to_vec(&crate::display_scope::Request { display })?,
    )
}

fn take_tile_request() -> Result<Option<crate::display_scope::Request>> {
    let dir = state_dir()?;
    let processing = dir.join("tile.processing");
    match fs::rename(dir.join("tile.request"), &processing) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let bytes = fs::read(&processing)?;
    fs::remove_file(processing)?;
    // Older clients wrote an eight-byte timestamp for a whole-desktop request.
    if bytes.len() == 8 {
        return Ok(Some(crate::display_scope::Request { display: None }));
    }
    Ok(Some(serde_json::from_slice(&bytes)?))
}

pub fn disengage() -> Result<()> {
    let _command = lock("display-command")?.context("Display command is busy; retry")?;
    // Latch cancellation before editing configuration, so an in-flight plan stops too.
    config::atomic_write(&state_dir()?.join("disengage.request"), b"disengage")?;
    config::set_enabled(false)?;
    take_request("tile.request")?;
    Ok(())
}

pub fn engage() -> Result<()> {
    config::set_enabled(true)?;
    request_tile()
}

fn take_request(name: &str) -> Result<bool> {
    match fs::remove_file(state_dir()?.join(name)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn responsive_wait(duration: Duration) -> Result<()> {
    let dir = state_dir()?;
    let path = config::path()?;
    let modified = fs::metadata(&path).and_then(|m| m.modified()).ok();
    let disengaged = workspace::disengaged()?;
    let deadline = std::time::Instant::now() + duration;
    while std::time::Instant::now() < deadline {
        if ["stop.request", "tile.request", "inventory.request"]
            .iter()
            .any(|name| dir.join(name).exists())
            || fs::metadata(&path).and_then(|m| m.modified()).ok() != modified
            || workspace::disengaged()? != disengaged
        {
            break;
        }
        thread::sleep(
            Duration::from_millis(100)
                .min(deadline.saturating_duration_since(std::time::Instant::now())),
        );
    }
    Ok(())
}

fn take_stop() -> Result<bool> {
    match fs::remove_file(state_dir()?.join("stop.request")) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub fn stop() -> Result<()> {
    if lock("worker")?.is_some() && lock("supervisor")?.is_some() {
        return Ok(());
    }
    config::atomic_write(&state_dir()?.join("stop.request"), b"stop")?;
    for _ in 0..160 {
        if lock("worker")?.is_some() && lock("supervisor")?.is_some() {
            let _ = take_stop()?;
            return Ok(());
        }
        thread::sleep(Duration::from_millis(250));
    }
    anyhow::bail!(
        "Worker did not stop within 40 seconds; check the desktop session before uninstalling"
    )
}

pub fn run() -> Result<()> {
    let Some(_lock) = lock("worker")? else {
        return Ok(());
    };
    write_status(0, "Discovering desktop…")?;
    let executable = std::env::current_exe()?;
    let mut backend = None;
    let mut inventory_session = String::new();
    let mut window_sessions: std::collections::BTreeMap<u64, String> =
        std::collections::BTreeMap::new();
    let mut next_window_session = 0_u64;
    let mut reconnect = crate::window_reconnect::Reconnector::default();
    let mut original = crate::restore::OriginalLayout::default();
    let mut restore_status = String::new();
    let mut previous = std::collections::BTreeMap::new();
    let mut heartbeat = 0;
    let mut last_message = String::new();
    while executable.exists() {
        if take_stop()? {
            break;
        }
        let request = take_tile_request()?;
        let force = request.is_some();
        let config = match Config::load() {
            Ok(config) => config,
            Err(e) => {
                write_status(0, &v_concat::v_concat!("Configuration error: {e:#}"))?;
                thread::sleep(Duration::from_secs(5));
                continue;
            }
        };
        take_request("inventory.request")?;
        let mut snapshot_failed = false;
        let result: Result<(usize, String)> = (|| {
            if backend.is_none() {
                backend = Some(platform::connect()?);
                window_sessions.clear();
                inventory_session = v_concat::v_concat!(
                    "{}-{}",
                    std::process::id(),
                    SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
                );
            }
            let backend = backend.as_mut().context("Missing desktop connection")?;
            let mut snapshot = backend.snapshot().inspect_err(|_| snapshot_failed = true)?;
            window_sessions.retain(|id, _| snapshot.windows.iter().any(|w| w.id == *id));
            for window in &mut snapshot.windows {
                window.session = window_sessions
                    .entry(window.id)
                    .or_insert_with(|| {
                        next_window_session += 1;
                        v_concat::v_concat!("{inventory_session}-{next_window_session}")
                    })
                    .clone();
                window.state.protected |= window.title.to_lowercase().contains("v_tileworkizer")
                    || config.excluded_titles.iter().any(|t| {
                        !t.is_empty() && window.title.to_lowercase().contains(&t.to_lowercase())
                    });
            }
            snapshot.windows.sort_by_key(|w| w.id);
            reconnect.update(&config, &mut snapshot);
            workspace::publish(&snapshot)?;
            if workspace::disengaged()? {
                previous.clear();
                if original.active() {
                    restore_status = original.restore(backend.as_mut(), &snapshot);
                }
                return Ok((
                    snapshot.windows.len(),
                    if restore_status.is_empty() {
                        "Not Enforced · no saved arrangement to restore".into()
                    } else {
                        restore_status.clone()
                    },
                ));
            }
            let paused = crate::display_pause::paused()?;
            if original.has_displays(&paused) {
                restore_status =
                    original.restore_displays(backend.as_mut(), &snapshot, Some(&paused));
            }
            if !config.enabled && !force {
                previous.clear();
                return Ok((
                    snapshot.windows.len(),
                    if restore_status.is_empty() {
                        "Not Enforced · move windows freely".into()
                    } else {
                        restore_status.clone()
                    },
                ));
            }
            let requests = match request {
                Some(request) => vec![Some(request)],
                None if config.engaged_displays.is_some() => snapshot
                    .monitors
                    .iter()
                    .filter(|area| config.display_engaged(**area))
                    .map(|area| {
                        Some(crate::display_scope::Request {
                            display: Some(*area),
                        })
                    })
                    .collect(),
                None => vec![None],
            };
            for request in requests {
                let (scoped, placements, layered) =
                    crate::display_scope::plan(&config, &snapshot, request)?;
                let scope = v_concat::v_concat!("{:?}", scoped.monitors);
                if scoped.monitors.iter().any(|area| paused.contains(area)) {
                    previous.remove(&scope);
                    continue;
                }
                let key =
                    v_concat::v_concat!("{:?}:{}:{}", placements, layered, snapshot.workspace);
                if force || previous.get(&scope) != Some(&key) {
                    if paused.is_empty() {
                        restore_status.clear();
                    }
                    if !workspace::apply(
                        backend.as_mut(),
                        &mut original,
                        &scoped,
                        &placements,
                        layered,
                    )? {
                        previous.clear();
                        return Ok((
                            snapshot.windows.len(),
                            "Not Enforced · move windows freely".into(),
                        ));
                    }
                    previous.insert(scope, key);
                }
            }
            Ok((
                snapshot.windows.len(),
                if !restore_status.is_empty() && !paused.is_empty() {
                    v_concat::v_concat!("Active on other displays · {restore_status}")
                } else if config.enabled {
                    "Active".into()
                } else {
                    "Arranged once · free movement".into()
                },
            ))
        })();
        let (count, message) = match result {
            Ok(value) => value,
            Err(e) => {
                // A refused move/raise is not a lost desktop connection. Reconnecting
                // here changes every window's session and invalidates the undo baseline.
                if snapshot_failed {
                    backend = None;
                }
                let _ = fs::remove_file(state_dir()?.join("windows.json"));
                previous.clear();
                (0, v_concat::v_concat!("Desktop error: {e:#}"))
            }
        };
        if now().saturating_sub(heartbeat) >= 5 || last_message != message || force {
            write_status(count, &message)?;
            heartbeat = now();
            last_message = message.clone();
        }
        responsive_wait(Duration::from_millis(
            if message.starts_with("Desktop error") {
                5000
            } else {
                config.poll_ms
            },
        ))?;
    }
    Ok(())
}
