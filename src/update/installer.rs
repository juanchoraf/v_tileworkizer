use anyhow::{Result, bail, ensure};
use std::{
    path::Path,
    process::{Command, Stdio},
};

pub(super) fn install(path: &Path, kind: &str) -> Result<bool> {
    let os = std::env::consts::OS;
    let mut command = installer_command(os, path, kind)?;
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()?;
    // Windows Installer also reports success with reboot-required/reboot-started codes.
    let reboot = os == "windows" && matches!(output.status.code(), Some(3010 | 1641));
    let success = output.status.success() || reboot;
    let error = String::from_utf8_lossy(&output.stderr);
    let detail: String = error.chars().take(2000).collect();
    ensure!(
        success,
        "System installer cancelled or failed ({}). {}",
        output.status,
        detail.trim()
    );
    Ok(reboot)
}

fn installer_command(os: &str, path: &Path, kind: &str) -> Result<Command> {
    let command = match os {
        "windows" => {
            let mut c = Command::new("msiexec.exe");
            c.arg("/i").arg(path).args(["/passive", "/norestart"]);
            c
        }
        "macos" => {
            let mut c = Command::new("/usr/bin/osascript");
            // Pass the path as data. AppleScript quotes it for the privileged shell.
            c.args(["-e", "on run argv", "-e",
                "do shell script \"/usr/sbin/installer -pkg \" & quoted form of (item 1 of argv) & \" -target /\" with administrator privileges",
                "-e", "end run"]).arg(path);
            c
        }
        "linux" => {
            let mut c = Command::new("pkexec");
            if kind == "deb" {
                c.args(["/usr/bin/apt-get", "--yes", "install", "--"]);
            } else if Path::new("/usr/bin/zypper").exists() {
                c.args(["/usr/bin/zypper", "--non-interactive", "install", "--"]);
            } else {
                c.args(["/usr/bin/dnf", "--assumeyes", "install", "--"]);
            }
            c.arg(path);
            c
        }
        "freebsd" => {
            let mut c = if Path::new("/usr/local/bin/pkexec").exists() {
                Command::new("/usr/local/bin/pkexec")
            } else {
                // A cached/admin sudo session is usable without an invisible prompt.
                let mut c = Command::new("sudo");
                c.arg("-n");
                c
            };
            c.args(["/usr/local/sbin/pkg", "add"]).arg(path);
            c
        }
        _ => bail!("Unsupported package manager"),
    };
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_passes_special_characters_as_an_argument_not_script_source() {
        let path = Path::new("/tmp/a 'quoted' $(name).pkg");
        let command = installer_command("macos", path, "pkg").unwrap();
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args.last().unwrap(), &path.as_os_str());
        assert!(
            args[..args.len() - 1]
                .iter()
                .all(|arg| !arg.to_string_lossy().contains("$(name)"))
        );
    }
}
