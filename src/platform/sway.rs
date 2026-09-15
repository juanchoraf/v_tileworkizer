use super::{Backend, Snapshot, Window};
use crate::layout::Rect;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

pub struct Sway {
    socket: UnixStream,
}

impl Sway {
    pub fn connect() -> Result<Self> {
        let path = std::env::var_os("SWAYSOCK").context("SWAYSOCK is missing")?;
        let socket = UnixStream::connect(path)?;
        socket.set_read_timeout(Some(Duration::from_secs(2)))?;
        socket.set_write_timeout(Some(Duration::from_secs(2)))?;
        Ok(Self { socket })
    }

    fn request(&mut self, kind: u32, payload: &str) -> Result<Value> {
        self.socket.write_all(b"i3-ipc")?;
        self.socket
            .write_all(&(payload.len() as u32).to_ne_bytes())?;
        self.socket.write_all(&kind.to_ne_bytes())?;
        self.socket.write_all(payload.as_bytes())?;
        let mut header = [0u8; 14];
        self.socket.read_exact(&mut header)?;
        ensure!(&header[..6] == b"i3-ipc", "Invalid Sway IPC header");
        let length = u32::from_ne_bytes(header[6..10].try_into()?);
        ensure!(length <= 16 * 1024 * 1024, "Sway response too large");
        ensure!(
            u32::from_ne_bytes(header[10..14].try_into()?) == kind,
            "Unexpected Sway response"
        );
        let mut bytes = vec![0; length as usize];
        self.socket.read_exact(&mut bytes)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

fn walk(
    node: &Value,
    windows: &mut Vec<Window>,
    monitors: &mut Vec<Rect>,
    visible: &[String],
    active: bool,
    fullscreen: bool,
) {
    let active = if node["type"] == "workspace" {
        let visible = node["name"]
            .as_str()
            .is_some_and(|name| visible.iter().any(|n| n == name));
        if visible && let Ok(rect) = serde_json::from_value(node["rect"].clone()) {
            monitors.push(rect);
        }
        visible
    } else {
        active
    };
    let fullscreen = fullscreen || node["fullscreen_mode"].as_u64().unwrap_or(0) != 0;
    if node["pid"].is_number()
        && let (Some(id), Ok(rect)) = (
            node["id"].as_u64(),
            serde_json::from_value(node["rect"].clone()),
        )
    {
        windows.push(Window {
            state: super::WindowState {
                fullscreen,
                floating: node["type"].as_str().map(|kind| kind == "floating_con"),
                hidden: !active || !node["visible"].as_bool().unwrap_or(false),
                protected: !node["window_properties"]["transient_for"].is_null(),
                ..Default::default()
            },
            session: String::new(),
            reconnects: Vec::new(),
            app_id: node["app_id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .map(|id| v_concat::v_concat!("wayland:{id}"))
                .or_else(|| {
                    node["window_properties"]["class"]
                        .as_str()
                        .filter(|id| !id.is_empty())
                        .map(|id| v_concat::v_concat!("x11:{id}"))
                })
                .unwrap_or_default(),
            id,
            title: node["name"].as_str().unwrap_or("").into(),
            rect,
        });
    }
    for field in ["nodes", "floating_nodes"] {
        if let Some(children) = node[field].as_array() {
            for child in children {
                walk(child, windows, monitors, visible, active, fullscreen);
            }
        }
    }
}

impl Backend for Sway {
    fn restore_window(&mut self, window: &Window) -> Result<()> {
        if window.state.floating == Some(false) {
            let id = window.id;
            let result = self.request(0, &v_concat::v_concat!("[con_id={id}] floating disable"))?;
            ensure!(
                result
                    .as_array()
                    .is_some_and(|a| !a.is_empty() && a.iter().all(|v| v["success"] == true)),
                "Sway refused restoration: {result}"
            );
            Ok(())
        } else {
            self.place(window.id, window.rect)
        }
    }
    fn stacking_order(&mut self) -> Result<Vec<u64>> {
        let tree = self.request(4, "")?;
        let mut order = Vec::new();
        stack_order(&tree, &mut order);
        Ok(order)
    }
    fn raise(&mut self, id: u64) -> Result<()> {
        // Sway raises floating containers on focus. The highest layer receives focus.
        let result = self.request(0, &v_concat::v_concat!("[con_id={id}] focus"))?;
        ensure!(
            result
                .as_array()
                .is_some_and(|a| !a.is_empty() && a.iter().all(|v| v["success"] == true)),
            "Sway refused stacking: {result}"
        );
        Ok(())
    }
    fn snapshot(&mut self) -> Result<Snapshot> {
        let workspaces = self.request(1, "")?;
        let visible: Vec<String> = workspaces
            .as_array()
            .context("Invalid Sway workspace list")?
            .iter()
            .filter(|w| w["visible"] == true)
            .filter_map(|w| w["name"].as_str().map(str::to_owned))
            .collect();
        let tree = self.request(4, "")?;
        let mut windows = Vec::new();
        let mut monitors = Vec::new();
        walk(&tree, &mut windows, &mut monitors, &visible, false, false);
        // Outputs are authoritative even when no visible workspace has windows.
        let outputs = self.request(3, "")?;
        let monitor_brands = outputs
            .as_array()
            .context("Invalid Sway output list")?
            .iter()
            .filter(|o| o["active"] == true)
            .map(|o| {
                o["make"]
                    .as_str()
                    .filter(|s| !s.is_empty() && *s != "Unknown")
                    .unwrap_or("Unknown brand")
                    .to_owned()
            })
            .collect();
        let work_areas = monitors;
        let monitors = outputs
            .as_array()
            .context("Invalid Sway output list")?
            .iter()
            .filter(|o| o["active"] == true)
            .filter_map(|o| serde_json::from_value::<Rect>(o["rect"].clone()).ok())
            .map(|output| {
                work_areas
                    .iter()
                    .find(|area| output.contains(area.x, area.y))
                    .copied()
                    .unwrap_or(output)
            })
            .collect();
        Ok(Snapshot {
            monitor_brands,
            windows,
            monitors,
            workspace: 0,
        })
    }

    fn place(&mut self, id: u64, rect: Rect) -> Result<()> {
        // Explicit geometry requires floating containers. IDs and geometry are numeric
        // so application titles can never inject compositor commands.
        let command = v_concat::v_concat!(
            "[con_id={id}] floating enable, resize set width {} px height {} px, move absolute position {} px {} px",
            rect.width,
            rect.height,
            rect.x,
            rect.y
        );
        let result = self.request(0, &command)?;
        let replies = result.as_array().context("Invalid Sway command response")?;
        ensure!(
            !replies.is_empty() && replies.iter().all(|r| r["success"] == true),
            "Sway refused geometry: {result}"
        );
        Ok(())
    }
}

// Sway focus lists are most-recent first; reverse them to raise back to front.
fn stack_order(node: &Value, order: &mut Vec<u64>) {
    if node["pid"].is_number()
        && let Some(id) = node["id"].as_u64()
    {
        order.push(id);
    }
    let mut children: Vec<_> = ["nodes", "floating_nodes"]
        .iter()
        .filter_map(|key| node[*key].as_array())
        .flatten()
        .collect();
    if let Some(focus) = node["focus"].as_array() {
        children.sort_by_key(|child| {
            std::cmp::Reverse(
                focus
                    .iter()
                    .position(|id| *id == child["id"])
                    .unwrap_or(usize::MAX),
            )
        });
    }
    for child in children {
        stack_order(child, order);
    }
}
