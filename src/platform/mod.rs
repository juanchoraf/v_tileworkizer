use crate::layout::Rect;
use anyhow::Result;
mod app_identity;
mod display_brand;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod macos_screens;
#[cfg(all(unix, not(target_os = "macos")))]
mod sway;
#[cfg(windows)]
pub mod windows;
#[cfg(all(unix, not(target_os = "macos")))]
mod x11;
#[cfg(all(unix, not(target_os = "macos")))]
mod x11_geometry;
#[cfg(all(unix, not(target_os = "macos")))]
mod x11_workarea;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Window {
    pub id: u64,
    pub title: String,
    pub rect: Rect,
    #[serde(default)]
    pub state: WindowState,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub app_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reconnects: Vec<crate::presets::WindowBinding>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub windows: Vec<Window>,
    pub monitors: Vec<Rect>,
    #[serde(default)]
    pub monitor_brands: Vec<String>,
    pub workspace: u32,
}

pub trait Backend {
    fn snapshot(&mut self) -> Result<Snapshot>;
    fn place(&mut self, id: u64, rect: Rect) -> Result<()>;
    /// Native bottom-to-top stacking order, or empty when unavailable.
    fn stacking_order(&mut self) -> Result<Vec<u64>> {
        Ok(Vec::new())
    }
    fn restore_window(&mut self, window: &Window) -> Result<()> {
        self.place(window.id, window.rect)
    }
    /// Raise within the normal window stack; never set an always-on-top flag.
    fn raise(&mut self, id: u64) -> Result<()>;
}

pub fn connect() -> Result<Box<dyn Backend>> {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if std::env::var_os("SWAYSOCK").is_some() {
            return Ok(Box::new(sway::Sway::connect()?));
        }
        Ok(Box::new(x11::X11::connect()?))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacOS::connect()?))
    }
    #[cfg(windows)]
    {
        Ok(Box::new(windows::Windows::new()))
    }
}

/// Inventory includes protected windows; only eligible windows enter a layout.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct WindowState {
    pub minimized: bool,
    pub maximized: bool,
    pub fullscreen: bool,
    pub hidden: bool,
    pub protected: bool,
    #[serde(default)]
    pub floating: Option<bool>,
}
impl Window {
    pub fn eligible(&self) -> bool {
        !self.state.minimized
            && !self.state.fullscreen
            && !self.state.hidden
            && !self.state.protected
    }
    pub fn status(&self) -> String {
        let mut labels = Vec::new();
        if self.state.minimized {
            labels.push("Minimized");
        }
        if self.state.maximized {
            labels.push("Maximized");
        }
        if self.state.fullscreen {
            labels.push("Fullscreen");
        }
        if self.state.hidden {
            labels.push("Hidden / other workspace");
        }
        if self.state.protected {
            labels.push("Not movable");
        }
        if labels.is_empty() {
            labels.push("Normal");
        }
        labels.join(" · ")
    }
}

impl Snapshot {
    pub fn display_name(&self, index: usize) -> String {
        let brand = self
            .monitor_brands
            .get(index)
            .map(String::as_str)
            .filter(|b| !b.trim().is_empty())
            .unwrap_or("Unknown brand");
        v_concat::v_concat!("Display {} ({brand})", index + 1)
    }
}
