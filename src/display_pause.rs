//! Persistent per-display cancellation, including arrangements requested only once.
use crate::{
    config::{self, Config},
    layout::Rect,
    service,
};
use anyhow::{Context, Result, ensure};

pub fn paused() -> Result<Vec<Rect>> {
    match std::fs::read(service::state_dir()?.join("paused-displays.json")) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

fn write(areas: &[Rect]) -> Result<()> {
    config::atomic_write(
        &service::state_dir()?.join("paused-displays.json"),
        &serde_json::to_vec(areas)?,
    )
}

pub fn pause(area: Rect, connected: &[Rect], expected: &Config) -> Result<Config> {
    let _command = service::lock("display-command")?.context("Display command is busy; retry")?;
    let _config = service::lock("config")?.context("Configuration is busy; retry")?;
    let mut config = Config::load()?;
    ensure!(
        &config == expected,
        "Settings changed elsewhere. Reopen the app before saving."
    );
    ensure!(
        connected.contains(&area),
        "Select a connected display first"
    );
    config.set_display_engaged(area, false, connected);
    let before = paused()?;
    let mut areas = before.clone();
    if !areas.contains(&area) {
        areas.push(area);
    }
    // Stop an in-flight placement before committing the new enforcement scope.
    write(&areas)?;
    if let Err(error) = config.save() {
        write(&before).context("Restore previous display cancellation state")?;
        return Err(error);
    }
    Ok(config)
}

pub fn resume(area: Option<Rect>) -> Result<()> {
    let mut areas = paused()?;
    areas.retain(|paused| area.is_some_and(|area| area != *paused));
    write(&areas)
}
