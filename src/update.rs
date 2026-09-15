mod installer;
use crate::config;
use anyhow::{Context, Result, bail, ensure};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    time::{Duration, Instant},
};

const API: &str = "https://api.github.com/repos/juanchoraf/v_tileworkizer/releases/latest";
const RELEASES: &str = "https://github.com/juanchoraf/v_tileworkizer/releases/download/";
const LIMIT: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub enum Progress {
    Checking,
    Downloading { received: u64, total: u64 },
    Verifying,
    Installing,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

pub fn run(install: bool) -> Result<()> {
    v_concat::v_concat_println!("{}", execute(install)?);
    Ok(())
}

fn package_kind() -> Result<&'static str> {
    match std::env::consts::OS {
        "windows" => Ok("msi"),
        "macos" | "freebsd" => Ok("pkg"),
        "linux" => {
            let release = fs::read_to_string("/etc/os-release")
                .context("Cannot identify Linux package format")?;
            let families: Vec<_> = release
                .lines()
                .filter(|l| l.starts_with("ID=") || l.starts_with("ID_LIKE="))
                .filter_map(|l| l.split_once('=').map(|(_, v)| v.trim_matches('"')))
                .collect();
            if families
                .iter()
                .flat_map(|s| s.split_whitespace())
                .any(|s| ["debian", "ubuntu"].contains(&s))
            {
                Ok("deb")
            } else if families
                .iter()
                .flat_map(|s| s.split_whitespace())
                .any(|s| ["fedora", "rhel", "centos", "suse", "opensuse"].contains(&s))
            {
                Ok("rpm")
            } else {
                bail!("No supported native installer for this Linux distribution")
            }
        }
        os => bail!("No published native installer format configured for {os}"),
    }
}

pub fn execute(install: bool) -> Result<String> {
    execute_with_progress(install, |_| {})
}

pub fn execute_with_progress(install: bool, mut report: impl FnMut(Progress)) -> Result<String> {
    report(Progress::Checking);
    let _lock = crate::service::lock("update")?.context("Another update is in progress")?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .build()
        .into();
    let release: Release = agent
        .get(API)
        .header("User-Agent", "v_tileworkizer")
        .header("Accept", "application/vnd.github+json")
        .call()
        .context("Cannot read GitHub release; a first release may not have been published yet")?
        .body_mut()
        .read_json()?;
    ensure!(
        !release.draft && !release.prerelease,
        "Refusing a draft or prerelease update"
    );
    let version = Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    )?;
    let current = Version::parse(env!("CARGO_PKG_VERSION"))?;
    if version <= current {
        return Ok(v_concat::v_concat!(
            "No updates available. You are running the latest version ({current})."
        ));
    }
    if !install {
        return Ok(v_concat::v_concat!(
            "Version {version} is available. Use --update or the sidebar Update button to install."
        ));
    }
    let kind = package_kind()?;
    let name = v_concat::v_concat!(
        "v_tileworkizer-{version}-{}-{}.{kind}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == name)
        .context("This release has no installer for your OS/architecture")?;
    validate_asset(asset, &release.tag_name)?;
    let expected = asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
        .context("GitHub did not provide a SHA-256 digest; refusing an unverified installer")?;
    ensure!(
        expected.len() == 64 && expected.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid release digest"
    );
    let directory = config::directory()?.join("downloads");
    fs::create_dir_all(&directory)?;
    let mut file = tempfile::NamedTempFile::new_in(&directory)?;
    let mut response = agent
        .get(&asset.browser_download_url)
        .header("User-Agent", "v_tileworkizer")
        .call()?;
    copy_verified(
        response.body_mut().as_reader(),
        &mut file,
        asset.size,
        expected,
        &mut report,
    )?;
    file.as_file().sync_all()?;
    let path = directory.join(&name);
    file.persist(&path).map_err(|e| e.error)?;
    report(Progress::Installing);
    let reboot = installer::install(&path, kind).with_context(|| {
        v_concat::v_concat!(
            "Installer saved at {}. Installation did not complete",
            path.display()
        )
    })?;
    if reboot {
        return Ok(v_concat::v_concat!(
            "Version {version} installed. Restart your computer to finish applying the update."
        ));
    }
    Ok(v_concat::v_concat!(
        "Version {version} installed. Close and reopen the app to use the update. Log out and back in to restart all desktop services."
    ))
}

fn copy_verified(
    reader: impl Read,
    mut writer: impl Write,
    size: u64,
    expected: &str,
    report: &mut impl FnMut(Progress),
) -> Result<()> {
    ensure!(size > 0 && size <= LIMIT, "Invalid installer size");
    let mut reader = reader.take(LIMIT + 1);
    let mut digest = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 65536];
    let mut last_report = Instant::now();
    report(Progress::Downloading {
        received: 0,
        total: size,
    });
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        ensure!(
            total <= LIMIT && total <= size,
            "Installer exceeded advertised size"
        );
        digest.update(&buffer[..n]);
        writer.write_all(&buffer[..n])?;
        if total == size || last_report.elapsed() >= Duration::from_millis(100) {
            report(Progress::Downloading {
                received: total,
                total: size,
            });
            last_report = Instant::now();
        }
    }
    ensure!(total == size, "Incomplete installer download");
    report(Progress::Verifying);
    let actual: String = digest
        .finalize()
        .iter()
        .map(|byte| v_concat::v_concat!("{byte:02x}"))
        .collect();
    ensure!(
        actual.eq_ignore_ascii_case(expected),
        "Installer SHA-256 mismatch; download rejected"
    );
    Ok(())
}

fn validate_asset(asset: &Asset, tag: &str) -> Result<()> {
    ensure!(
        asset.size > 0 && asset.size <= LIMIT,
        "Invalid installer size"
    );
    ensure!(
        tag.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".-+".contains(&c)),
        "Invalid release tag"
    );
    let expected = v_concat::v_concat!("{RELEASES}{tag}/{}", asset.name);
    ensure!(
        asset.browser_download_url == expected,
        "Installer must come from juanchoraf/v_tileworkizer"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const ABC_DIGEST: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn download_progress_reports_bytes_before_verification() {
        let mut file = Vec::new();
        let mut events = Vec::new();
        copy_verified(&b"abc"[..], &mut file, 3, ABC_DIGEST, &mut |p| {
            events.push(p)
        })
        .unwrap();
        assert_eq!(file, b"abc");
        assert!(matches!(
            events[0],
            Progress::Downloading {
                received: 0,
                total: 3
            }
        ));
        assert!(matches!(
            events[1],
            Progress::Downloading {
                received: 3,
                total: 3
            }
        ));
        assert!(matches!(events[2], Progress::Verifying));
    }

    #[test]
    fn rejects_truncated_oversized_or_corrupted_downloads() {
        for (data, size, message) in [
            (&b"ab"[..], 3, "Incomplete"),
            (&b"abcd"[..], 3, "exceeded"),
            (&b"abd"[..], 3, "SHA-256 mismatch"),
        ] {
            let error = copy_verified(data, Vec::new(), size, ABC_DIGEST, &mut |_| {}).unwrap_err();
            assert!(error.to_string().contains(message));
        }
    }

    #[test]
    fn rejects_wrong_repository_and_oversized_downloads() {
        let mut asset = Asset {
            name: "app.msi".into(),
            browser_download_url: "https://github.com/attacker/app/releases/download/v1/app.msi"
                .into(),
            size: 12,
            digest: None,
        };
        assert!(validate_asset(&asset, "v1").is_err());
        asset.browser_download_url = v_concat::v_concat!("{RELEASES}v1/app.msi");
        assert!(validate_asset(&asset, "v1").is_ok());
        asset.size = LIMIT + 1;
        assert!(validate_asset(&asset, "v1").is_err());
    }
}
