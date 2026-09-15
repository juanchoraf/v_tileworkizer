use crate::layout::Layout;
use anyhow::{Context, Result, ensure};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub layout: Layout,
    pub gap: i32,
    pub master_ratio: f32,
    pub poll_ms: u64,
    pub excluded_titles: Vec<String>,
    pub presets: Vec<crate::presets::Preset>,
    pub active_preset: Option<String>,
    pub display_layouts: Vec<crate::display_layout::DisplayLayout>,
    pub standard_assignments: Vec<crate::standard_layout::Assignments>,
    pub engaged_displays: Option<Vec<crate::layout::Rect>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            layout: Layout::Master,
            gap: 12,
            master_ratio: 0.6,
            poll_ms: 750,
            excluded_titles: vec!["v_tileworkizer".into()],
            presets: Vec::new(),
            active_preset: None,
            display_layouts: Vec::new(),
            standard_assignments: Vec::new(),
            engaged_displays: None,
            extra: BTreeMap::new(),
        }
    }
}

pub fn directory() -> Result<PathBuf> {
    let dir = ProjectDirs::from("com", "TheVelasquez", "v_tileworkizer")
        .context("Cannot determine the current user's configuration directory")?
        .config_dir()
        .to_path_buf();
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(dir)
}

pub fn path() -> Result<PathBuf> {
    Ok(directory()?.join("config.json"))
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        self.validate_display_layouts()?;
        self.validate_standard_assignments()?;
        self.validate_display_scope()?;
        ensure!(
            self.presets.len() <= 64,
            "At most 64 custom presets are supported"
        );
        let mut names = std::collections::BTreeSet::new();
        for preset in &self.presets {
            preset.validate()?;
            ensure!(
                names.insert(preset.name.to_lowercase()),
                "Preset names must be unique"
            );
        }
        if let Some(name) = &self.active_preset {
            ensure!(
                self.presets.iter().any(|p| &p.name == name),
                "Selected preset does not exist"
            );
        }
        ensure!(
            (0..=100).contains(&self.gap),
            "gap must be between 0 and 100"
        );
        ensure!(
            self.master_ratio.is_finite() && (0.2..=0.8).contains(&self.master_ratio),
            "master_ratio must be between 0.2 and 0.8"
        );
        ensure!(
            (250..=30_000).contains(&self.poll_ms),
            "poll_ms must be between 250 and 30000"
        );
        ensure!(
            self.excluded_titles.len() <= 256,
            "At most 256 title exclusions are supported"
        );
        Ok(())
    }

    pub fn load() -> Result<Self> {
        let path = path()?;
        match fs::read(&path) {
            Ok(bytes) => {
                ensure!(bytes.len() <= 1_048_576, "Configuration exceeds 1 MiB");
                let config: Self = serde_json::from_slice(&bytes)
                    .context("Invalid config.json; it has not been overwritten")?;
                config.validate()?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self) -> Result<()> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self)?;
        ensure!(
            bytes.len() <= 1_048_576,
            "Configuration exceeds 1 MiB; shorten title rules or remove unused presets"
        );
        atomic_write(&path()?, &bytes)
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = tempfile::NamedTempFile::new_in(path.parent().context("Missing parent")?)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let _lock = crate::service::lock("config")?.context("Configuration is being edited; retry")?;
    let mut config = Config::load()?;
    config.enabled = enabled;
    config.save()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_configuration_keeps_settings_and_unknown_fields() {
        let config: Config =
            serde_json::from_str(r#"{"enabled":true,"gap":8,"future_option":42}"#).unwrap();
        config.validate().unwrap();
        assert!(config.enabled);
        assert_eq!(config.gap, 8);
        assert!(config.presets.is_empty());
        assert_eq!(serde_json::to_value(config).unwrap()["future_option"], 42);
    }
    #[test]
    fn rejects_duplicate_preset_names_and_missing_selection() {
        let preset = crate::presets::Preset::from_layout("Work".into(), Layout::Grid, 2, 0.6);
        let mut config = Config {
            presets: vec![preset.clone(), preset],
            ..Config::default()
        };
        assert!(config.validate().is_err());
        config.presets.pop();
        config.active_preset = Some("Missing".into());
        assert!(config.validate().is_err());
    }
}
