use super::{Backend, Snapshot, Window};
use crate::layout::Rect;
use anyhow::{Context, Result, ensure};
use x11rb::{
    connection::Connection,
    protocol::{randr::ConnectionExt as _, xproto::*},
    rust_connection::RustConnection,
};

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        _NET_CLIENT_LIST, _NET_CLIENT_LIST_STACKING, _NET_CURRENT_DESKTOP, _NET_WM_DESKTOP, _NET_WORKAREA,
        _NET_WM_NAME, UTF8_STRING, _NET_WM_WINDOW_TYPE, _NET_WM_WINDOW_TYPE_NORMAL,
        _NET_WM_STATE, _NET_WM_STATE_HIDDEN, _NET_WM_STATE_FULLSCREEN,
        _NET_WM_STATE_MAXIMIZED_VERT, _NET_WM_STATE_MAXIMIZED_HORZ,
        _NET_MOVERESIZE_WINDOW, _NET_FRAME_EXTENTS, _GTK_FRAME_EXTENTS, _NET_SUPPORTING_WM_CHECK,
    }
}

pub struct X11 {
    conn: RustConnection,
    root: u32,
    atoms: Atoms,
    fallback: Rect,
}

impl X11 {
    pub fn connect() -> Result<Self> {
        ensure!(
            std::env::var_os("WAYLAND_DISPLAY").is_none(),
            "Wayland does not expose a universal window-control API. Use an X11 desktop session; XWayland cannot manage native Wayland windows."
        );
        let (conn, index) =
            x11rb::connect(None).context("Connect to X11 in a graphical login session")?;
        let screen = &conn.setup().roots[index];
        let root = screen.root;
        let fallback = Rect {
            x: 0,
            y: 0,
            width: screen.width_in_pixels.into(),
            height: screen.height_in_pixels.into(),
        };
        let atoms = Atoms::new(&conn)?.reply()?;
        let backend = Self {
            conn,
            root,
            atoms,
            fallback,
        };
        ensure!(
            !backend
                .property(root, backend.atoms._NET_SUPPORTING_WM_CHECK)?
                .is_empty(),
            "An EWMH-compatible X11 window manager is required"
        );
        Ok(backend)
    }

    fn output_brand(&self, output: u32) -> Result<String> {
        let edid = self.conn.intern_atom(true, b"EDID")?.reply()?.atom;
        if edid == 0 {
            return Ok("Unknown brand".into());
        }
        let reply = self
            .conn
            .randr_get_output_property(output, edid, AtomEnum::ANY, 0, 32, false, false)?
            .reply()?;
        Ok(super::display_brand::edid(&reply.data))
    }

    fn property(&self, window: u32, atom: u32) -> Result<Vec<u32>> {
        Ok(self
            .conn
            .get_property(false, window, atom, AtomEnum::ANY, 0, 4096)?
            .reply()?
            .value32()
            .map(|v| v.collect())
            .unwrap_or_default())
    }

    fn message(&self, window: u32, atom: u32, values: [u32; 5]) -> Result<()> {
        self.conn
            .send_event(
                false,
                self.root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                ClientMessageEvent::new(32, window, atom, values),
            )?
            .check()?;
        Ok(())
    }

    fn window(&self, id: u32, workspace: u32) -> Result<Option<Window>> {
        let attrs = self.conn.get_window_attributes(id)?.reply()?;
        if attrs.override_redirect {
            return Ok(None);
        }
        let desktop = self.property(id, self.atoms._NET_WM_DESKTOP)?;
        let hidden = desktop
            .first()
            .is_some_and(|d| *d != workspace && *d != u32::MAX);
        let types = self.property(id, self.atoms._NET_WM_WINDOW_TYPE)?;
        let protected =
            !types.is_empty() && !types.contains(&self.atoms._NET_WM_WINDOW_TYPE_NORMAL);
        let states = self.property(id, self.atoms._NET_WM_STATE)?;
        let name = self
            .conn
            .get_property(
                false,
                id,
                self.atoms._NET_WM_NAME,
                self.atoms.UTF8_STRING,
                0,
                1024,
            )?
            .reply()?;
        let title = if name.value.is_empty() {
            String::from_utf8_lossy(
                &self
                    .conn
                    .get_property(false, id, AtomEnum::WM_NAME, AtomEnum::ANY, 0, 1024)?
                    .reply()?
                    .value,
            )
            .into_owned()
        } else {
            String::from_utf8_lossy(&name.value).into_owned()
        };
        let geo = self.conn.get_geometry(id)?.reply()?;
        let coords = self
            .conn
            .translate_coordinates(id, self.root, 0, 0)?
            .reply()?;
        let app_id = self
            .conn
            .get_property(false, id, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 1024)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| {
                reply
                    .value
                    .split(|byte| *byte == 0)
                    .nth(1)
                    .filter(|class| !class.is_empty())
                    .map(|class| v_concat::v_concat!("x11:{}", String::from_utf8_lossy(class)))
            })
            .unwrap_or_default();
        Ok(Some(Window {
            state: super::WindowState {
                minimized: states.contains(&self.atoms._NET_WM_STATE_HIDDEN),
                maximized: states.contains(&self.atoms._NET_WM_STATE_MAXIMIZED_VERT)
                    || states.contains(&self.atoms._NET_WM_STATE_MAXIMIZED_HORZ),
                fullscreen: states.contains(&self.atoms._NET_WM_STATE_FULLSCREEN),
                hidden: hidden || attrs.map_state != MapState::VIEWABLE,
                protected,
                floating: None,
            },
            session: String::new(),
            reconnects: Vec::new(),
            app_id,
            id: id.into(),
            title,
            rect: Rect {
                x: coords.dst_x.into(),
                y: coords.dst_y.into(),
                width: geo.width.into(),
                height: geo.height.into(),
            },
        }))
    }
}

impl Backend for X11 {
    fn stacking_order(&mut self) -> Result<Vec<u64>> {
        Ok(self
            .property(self.root, self.atoms._NET_CLIENT_LIST_STACKING)?
            .into_iter()
            .map(u64::from)
            .collect())
    }
    fn restore_window(&mut self, window: &Window) -> Result<()> {
        let id = u32::try_from(window.id)?;
        self.message(
            id,
            self.atoms._NET_WM_STATE,
            [
                0,
                self.atoms._NET_WM_STATE_MAXIMIZED_VERT,
                self.atoms._NET_WM_STATE_MAXIMIZED_HORZ,
                2,
                0,
            ],
        )?;
        // Snapshot geometry is the client rectangle. Static gravity restores that
        // origin directly, without shrinking it a second time for frame extents.
        self.message(
            id,
            self.atoms._NET_MOVERESIZE_WINDOW,
            [
                10 | (15 << 8) | (2 << 12),
                window.rect.x as u32,
                window.rect.y as u32,
                window.rect.width.max(1) as u32,
                window.rect.height.max(1) as u32,
            ],
        )?;
        if window.state.maximized {
            self.message(
                id,
                self.atoms._NET_WM_STATE,
                [
                    1,
                    self.atoms._NET_WM_STATE_MAXIMIZED_VERT,
                    self.atoms._NET_WM_STATE_MAXIMIZED_HORZ,
                    2,
                    0,
                ],
            )?;
        }
        self.conn.flush()?;
        Ok(())
    }
    fn raise(&mut self, id: u64) -> Result<()> {
        self.conn
            .configure_window(
                u32::try_from(id)?,
                &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
            )?
            .check()?;
        self.conn.flush()?;
        Ok(())
    }
    fn snapshot(&mut self) -> Result<Snapshot> {
        let workspace = self
            .property(self.root, self.atoms._NET_CURRENT_DESKTOP)?
            .first()
            .copied()
            .unwrap_or(0);
        let ids = self.property(self.root, self.atoms._NET_CLIENT_LIST)?;
        let mut windows = Vec::new();
        for id in ids {
            // Windows can disappear between discovery and inspection.
            if let Ok(Some(window)) = self.window(id, workspace) {
                windows.push(window);
            }
        }
        let mut monitor_brands = Vec::new();
        let mut monitors = match self.conn.randr_get_monitors(self.root, true) {
            Ok(cookie) => cookie
                .reply()
                .ok()
                .map(|reply| {
                    reply
                        .monitors
                        .into_iter()
                        .map(|m| {
                            monitor_brands.push(
                                m.outputs
                                    .first()
                                    .and_then(|output| self.output_brand(*output).ok())
                                    .unwrap_or_else(|| "Unknown brand".into()),
                            );
                            Rect {
                                x: m.x.into(),
                                y: m.y.into(),
                                width: m.width.into(),
                                height: m.height.into(),
                            }
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        if monitors.is_empty()
            && let Ok(cookie) = self.conn.randr_get_screen_resources_current(self.root)
            && let Ok(resources) = cookie.reply()
        {
            for crtc in resources.crtcs {
                if let Ok(cookie) = self
                    .conn
                    .randr_get_crtc_info(crtc, resources.config_timestamp)
                    && let Ok(info) = cookie.reply()
                    && info.width > 0
                    && info.height > 0
                {
                    monitor_brands.push(
                        info.outputs
                            .first()
                            .and_then(|output| self.output_brand(*output).ok())
                            .unwrap_or_else(|| "Unknown brand".into()),
                    );
                    monitors.push(Rect {
                        x: info.x.into(),
                        y: info.y.into(),
                        width: info.width.into(),
                        height: info.height.into(),
                    });
                }
            }
        }
        if monitors.is_empty() {
            monitors.push(self.fallback);
            monitor_brands.push("Unknown brand".into());
        }
        let work = self.property(self.root, self.atoms._NET_WORKAREA)?;
        let global = super::x11_workarea::areas(&work)
            .get(workspace as usize)
            .copied();
        let name = v_concat::v_concat!("_GTK_WORKAREAS_D{workspace}");
        let atom = self.conn.intern_atom(true, name.as_bytes())?.reply()?.atom;
        let local = if atom == 0 {
            Vec::new()
        } else {
            super::x11_workarea::areas(&self.property(self.root, atom)?)
        };
        super::x11_workarea::apply(&mut monitors, &local, global);
        Ok(Snapshot {
            monitor_brands,
            windows,
            monitors,
            workspace,
        })
    }

    fn place(&mut self, id: u64, rect: Rect) -> Result<()> {
        let id = u32::try_from(id)?;
        self.message(
            id,
            self.atoms._NET_WM_STATE,
            [
                0,
                self.atoms._NET_WM_STATE_MAXIMIZED_VERT,
                self.atoms._NET_WM_STATE_MAXIMIZED_HORZ,
                2,
                0,
            ],
        )?;
        let extents = self.property(id, self.atoms._NET_FRAME_EXTENTS)?;
        let shadows = self.property(id, self.atoms._GTK_FRAME_EXTENTS)?;
        let client = super::x11_geometry::client_rect(rect, &extents, &shadows);
        // Static gravity positions the client origin explicitly. Visible window
        // borders match the tile; client-side shadow margins extend outside it.
        self.message(
            id,
            self.atoms._NET_MOVERESIZE_WINDOW,
            [
                10 | (15 << 8) | (2 << 12),
                client.x as u32,
                client.y as u32,
                client.width as u32,
                client.height as u32,
            ],
        )?;
        self.conn.flush()?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "x11_live.rs"]
mod live_tests;
