mod config;
mod display_layout;
mod display_pause;
mod display_scope;
mod gui;
mod gui_position;
mod layout;
mod platform;
mod presets;
mod restore;
mod service;
mod service_identity;
mod standard_layout;
mod update;
mod window_binding;
mod window_reconnect;
mod workspace;

use anyhow::{Result, bail};
use v_concat::{v_concat_eprintln, v_concat_println};

const HELP: &str = r#"v_tileworkizer — native tiling workspace organizer

  v_tileworkizer                 Open the desktop GUI and start the session service
  v_tileworkizer --gui           Open the desktop GUI explicitly
  v_tileworkizer --service       Run the background worker in the current session
  v_tileworkizer --supervise     Restart the worker if it exits unexpectedly
  v_tileworkizer --stop-service  Stop this session's worker and supervisor
  v_tileworkizer --tile          Request one arrangement (also works while paused)
  v_tileworkizer --pause         Stop enforcement and restore all arranged displays
  v_tileworkizer --resume        Enable automatic arrangement and start the worker
  v_tileworkizer --inspect       Print a fresh read-only desktop inventory as JSON
  v_tileworkizer --status        Show worker health and configuration location
  v_tileworkizer --check-update  Check juanchoraf/v_tileworkizer releases
  v_tileworkizer --update        Download, verify and open the native installer
  v_tileworkizer --version       Show the application version
  v_tileworkizer --help          Show this help

Automatic tiling starts not enforced. Choose a layout and Apply & Enforce in the GUI.
Not Enforce beside Arrange Once stops and restores only the selected display.
Preset studio lets you save custom tiles, assign windows and set overlapping layers.
Assignments track live window IDs and reconnect by application identity after a restart.
For multiple windows from one app, set a stable title fragment under Reconnect options.
Ambiguous matches wait for your selection; changing a live window title keeps its tile.
The background worker continues when the GUI closes. Each user has independent
configuration. Installer registration starts the service at desktop login.
"#;

fn main() {
    if let Err(error) = run() {
        v_concat_eprintln!("v_tileworkizer: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(windows)]
    if platform::windows::powershell(&args)? {
        return Ok(());
    }
    if args.len() > 1 {
        bail!("Expected one option; use --help");
    }
    match args.first().map(String::as_str) {
        Some("--help" | "-h") => v_concat_println!("{}", HELP),
        Some("--version" | "-V") => {
            v_concat_println!("v_tileworkizer {}", env!("CARGO_PKG_VERSION"))
        }
        Some("--check-update") => update::run(false)?,
        Some("--update") => update::run(true)?,
        Some("--service") => service::run()?,
        Some("--supervise") => service::supervise()?,
        Some("--stop-service") => service::stop()?,
        Some("--tile") => service::request_tile()?,
        Some("--pause") => service::disengage()?,
        Some("--resume") => {
            service::engage()?;
        }
        Some("--inspect") => v_concat_println!(
            "{}",
            serde_json::to_string_pretty(&platform::connect()?.snapshot()?)?
        ),
        Some("--status") => v_concat_println!("{}", service::status_text()?),
        None | Some("--gui") => {
            service::start()?;
            gui::run()?;
        }
        Some(other) => bail!("Unknown option {other}; use --help"),
    }
    Ok(())
}
