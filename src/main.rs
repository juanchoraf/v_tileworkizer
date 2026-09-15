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
        Some("--help" | "-h") => v_concat_println!("{}", include_str!("../docs/help.txt")),
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
