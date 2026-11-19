use std::os::unix::io::AsRawFd;
use std::thread;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::randr;
use x11rb::protocol::shape::{self, SK, SO};
use x11rb::protocol::xproto::{
    self, AtomEnum, ClipOrdering, ColormapAlloc, ConfigureWindowAux, ConnectionExt,
    CreateGCAux, CreateWindowAux, EventMask, ImageFormat, InputFocus, PropMode, Rectangle,
    StackMode, VisualClass, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

#[derive(Debug, Clone, Copy)]
pub enum WindowEvent {
    Expose,
    MonitorChanged,
    PointerEnter(i16, i16),
    PointerLeave,
    PointerMove(i16, i16),
    ButtonDown(u8, i16, i16), // 1=left, 3=right, 4=scroll_up, 5=scroll_down
    ButtonUp(u8),
    KeyChar(char),
    KeyBackspace,
    KeyEnter,
    KeyEscape,
}

pub struct X11OverlayWindow {
    conn: RustConnection,
    screen_num: usize,
    win: u32,
    gc: u32,
    pub width: u16,
    pub height: u16,
    is_bottom: bool,
    y_offset: i32,
    prev_blit_h: u16,
    min_keycode: u8,
    keysyms_per_keycode: u8,
    keysyms: Vec<u32>,
}

fn connect_x11_with_retry() -> Result<(RustConnection, usize), String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match RustConnection::connect(None) {
            Ok(pair) => return Ok(pair),
            Err(e) => {
                // On some Wayland/autostart sessions DISPLAY may not be exported yet; try :0 / :1
                for fallback in [":0", ":1"] {
                    if let Ok(pair) = RustConnection::connect(Some(fallback)) {
                        return Ok(pair);
                    }
                }
                if Instant::now() >= deadline {
                    return Err(format!("X11 connect failed: {e}"));
                }
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn is_compositor_and_wm_ready(conn: &RustConnection, screen_num: usize, root: u32) -> bool {
    let cm_name = format!("_NET_WM_CM_S{}", screen_num);
    let has_cm = intern(conn, cm_name.as_bytes())
        .and_then(|atom| conn.get_selection_owner(atom).ok())
        .and_then(|c| c.reply().ok())
        .map(|r| r.owner != 0)
        .unwrap_or(false);

    let has_wm = intern(conn, b"_NET_SUPPORTING_WM_CHECK")
        .and_then(|atom| {
            conn.get_property(false, root, atom, AtomEnum::WINDOW, 0, 1)
                .ok()
        })
        .and_then(|c| c.reply().ok())
        .map(|r| !r.value.is_empty())
        .unwrap_or(false);

    has_cm && has_wm
}

fn wait_for_desktop_ready(conn: &RustConnection, screen_num: usize, root: u32) {
    if is_compositor_and_wm_ready(conn, screen_num, root) {
        return;
    }
    // We caught the session mid-login (e.g. systemd-xdg-autostart before Mutter/XWayland or picom is ready).
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(200));
        if is_compositor_and_wm_ready(conn, screen_num, root) {
            // Brief settle time so GNOME Shell / panel finishes its initial mapping before we map on top
            thread::sleep(Duration::from_millis(600));
            return;
        }
    }
}

impl X11OverlayWindow {
    pub fn new(width: u16, height: u16, is_bottom: bool, y_offset: i32) -> Result<Self, String> {
        let (conn, screen_num) = connect_x11_with_retry()?;
        let setup = conn.setup();
        let min_keycode = setup.min_keycode;
        let max_keycode = setup.max_keycode;
        let screen = &setup.roots[screen_num];
        let root = screen.root;

        // Wait for WM + X11/XWayland compositor on early login startup so the 32-bit ARGB
        // override_redirect overlay isn't dropped or hidden behind the desktop shell.
        wait_for_desktop_ready(&conn, screen_num, root);

        let argb_visual = screen
            .allowed_depths
            .iter()
            .find(|d| d.depth == 32)
            .and_then(|d| {
                d.visuals
                    .iter()
                    .find(|v| v.class == VisualClass::TRUE_COLOR)
                    .map(|v| v.visual_id)
            })
            .ok_or_else(|| "No 32-bit ARGB TrueColor X11 visual found".to_string())?;

        let cmap = conn
            .generate_id()
            .map_err(|e| format!("generate_id cmap failed: {e}"))?;
        conn.create_colormap(ColormapAlloc::NONE, cmap, root, argb_visual)
            .map_err(|e| format!("create_colormap failed: {e}"))?;

        let (mon_x, mon_y, mon_w, mon_h) = detect_primary_monitor(&conn, root, screen);
        let y_off = y_offset.clamp(0, 64);
        let win_x = mon_x + ((mon_w - width as i32) / 2).max(0);
        let win_y = if is_bottom {
            mon_y + mon_h - height as i32 - y_off
        } else {
            mon_y + y_off
        };

        let win = conn
            .generate_id()
            .map_err(|e| format!("generate_id win failed: {e}"))?;

        let event_mask = EventMask::EXPOSURE
            | EventMask::POINTER_MOTION
            | EventMask::BUTTON_PRESS
            | EventMask::BUTTON_RELEASE
            | EventMask::ENTER_WINDOW
            | EventMask::LEAVE_WINDOW
            | EventMask::KEY_PRESS
            | EventMask::STRUCTURE_NOTIFY;

        let aux = CreateWindowAux::new()
            .background_pixel(0)
            .border_pixel(0)
            .colormap(cmap)
            .override_redirect(1)
            .event_mask(event_mask);

        conn.create_window(
            32,
            win,
            root,
            win_x as i16,
            win_y as i16,
            width,
            height,
            0,
            WindowClass::INPUT_OUTPUT,
            argb_visual,
            &aux,
        )
        .map_err(|e| format!("create_window failed: {e}"))?;

        // Subscribe to RandR monitor layout/scaling changes (common right after GNOME/KDE login)
        let _ = randr::select_input(
            &conn,
            root,
            randr::NotifyMask::SCREEN_CHANGE
                | randr::NotifyMask::OUTPUT_CHANGE
                | randr::NotifyMask::CRTC_CHANGE,
        );

        set_ewmh_overlay_hints(&conn, win);

        let gc = conn
            .generate_id()
            .map_err(|e| format!("generate_id gc failed: {e}"))?;
        conn.create_gc(gc, win, &CreateGCAux::new())
            .map_err(|e| format!("create_gc failed: {e}"))?;

        // Cache X11 keyboard mapping once for lightweight pure-Rust text input
        let count = max_keycode.saturating_sub(min_keycode).saturating_add(1);
        let (keysyms_per_keycode, keysyms) = conn
            .get_keyboard_mapping(min_keycode, count)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| (r.keysyms_per_keycode, r.keysyms))
            .unwrap_or((0, Vec::new()));

        conn.map_window(win)
            .map_err(|e| format!("map_window failed: {e}"))?;
        let _ = conn.configure_window(
            win,
            &ConfigureWindowAux::new()
                .x(win_x)
                .y(win_y)
                .stack_mode(StackMode::ABOVE),
        );
        conn.flush().map_err(|e| format!("flush failed: {e}"))?;

        Ok(Self {
            conn,
            screen_num,
            win,
            gc,
            width,
            height,
            is_bottom,
            y_offset: y_off,
            prev_blit_h: height,
            min_keycode,
            keysyms_per_keycode,
            keysyms,
        })
    }

    pub fn set_position(&mut self, is_bottom: bool, y_offset: i32) {
        let y_off = y_offset.clamp(0, 64);
        self.is_bottom = is_bottom;
        self.y_offset = y_off;
        self.prev_blit_h = self.height;

        let screen = &self.conn.setup().roots[self.screen_num];
        let root = screen.root;
        let (mon_x, mon_y, mon_w, mon_h) = detect_primary_monitor(&self.conn, root, screen);
        let win_x = mon_x + ((mon_w - self.width as i32) / 2).max(0);
        let win_y = if is_bottom {
            mon_y + mon_h - self.height as i32 - y_off
        } else {
            mon_y + y_off
        };
        let _ = self.conn.configure_window(
            self.win,
            &ConfigureWindowAux::new()
                .x(win_x)
                .y(win_y)
                .stack_mode(StackMode::ABOVE),
        );
        let _ = self.conn.flush();
    }

    /// Re-syncs primary monitor geometry, ensures the overlay window is mapped & stacked ABOVE,
    /// and forces a full repaint on the next present (rescues early-login XWayland/WM races).
    pub fn sync_monitor_and_raise(&mut self) {
        self.prev_blit_h = self.height;
        let screen = &self.conn.setup().roots[self.screen_num];
        let root = screen.root;
        let (mon_x, mon_y, mon_w, mon_h) = detect_primary_monitor(&self.conn, root, screen);
        let win_x = mon_x + ((mon_w - self.width as i32) / 2).max(0);
        let win_y = if self.is_bottom {
            mon_y + mon_h - self.height as i32 - self.y_offset
        } else {
            mon_y + self.y_offset
        };
        set_ewmh_overlay_hints(&self.conn, self.win);
        let _ = self.conn.map_window(self.win);
        let _ = self.conn.configure_window(
            self.win,
            &ConfigureWindowAux::new()
                .x(win_x)
                .y(win_y)
                .stack_mode(StackMode::ABOVE),
        );
        let _ = self.conn.flush();
    }

    pub fn set_input_region(&self, pill_width: i32, pill_height: i32) {
        let pw = pill_width.clamp(16, self.width as i32);
        let ph = (pill_height + 6).clamp(16, self.height as i32);
        let px = ((self.width as i32 - pw) / 2).max(0);
        let py = if self.is_bottom {
            (self.height as i32 - ph).max(0)
        } else {
            0
        };
        let rect = Rectangle {
            x: px as i16,
            y: py as i16,
            width: pw as u16,
            height: ph as u16,
        };
        let _ = shape::rectangles(
            &self.conn,
            SO::SET,
            SK::INPUT,
            ClipOrdering::UNSORTED,
            self.win,
            0,
            0,
            &[rect],
        );
        let _ = self.conn.flush();
    }

    pub fn raise(&self) {
        let _ = self
            .conn
            .configure_window(self.win, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE));
        let _ = self.conn.flush();
    }

    pub fn grab_keyboard_focus(&self) {
        let _ = self.conn.set_input_focus(
            InputFocus::POINTER_ROOT,
            self.win,
            x11rb::CURRENT_TIME,
        );
        let _ = self.conn.flush();
    }

    /// Swizzles RGBA -> BGRA in-place and blits the active region (plus full clear when shrinking).
    pub fn present(&mut self, pixmap: &mut tiny_skia::Pixmap, active_h: u16) {
        let clamped_active = active_h.min(self.height).max(1);
        let blit_h = if clamped_active < self.prev_blit_h {
            // When collapsing/shrinking, always blit full height so zero stale/shadow pixels ever linger
            self.height
        } else {
            clamped_active.max(self.prev_blit_h).min(self.height)
        };
        self.prev_blit_h = clamped_active;

        let (y_start, y_end) = if self.is_bottom {
            (self.height.saturating_sub(blit_h), self.height)
        } else {
            (0, blit_h)
        };

        let w = self.width as usize;
        let row_bytes = w * 4;
        let data = pixmap.data_mut();
        let swizzle_start = y_start as usize * row_bytes;
        let swizzle_end = y_end as usize * row_bytes;

        for px in data[swizzle_start..swizzle_end].chunks_exact_mut(4) {
            px.swap(0, 2);
        }

        const STRIP_ROWS: u16 = 64;
        let mut y = y_start;
        while y < y_end {
            let h = STRIP_ROWS.min(y_end - y);
            let start = y as usize * row_bytes;
            let end = start + h as usize * row_bytes;
            let _ = self.conn.put_image(
                ImageFormat::Z_PIXMAP,
                self.win,
                self.gc,
                self.width,
                h,
                0,
                y as i16,
                0,
                32,
                &data[start..end],
            );
            y += h;
        }
        let _ = self.conn.flush();
    }

    pub fn refresh_rate_hz(&self) -> u32 {
        60
    }

    pub fn wait_events_us(&self, timeout_us: u64) {
        if timeout_us == 0 {
            return;
        }
        let timeout_ms = ((timeout_us + 500) / 1000).max(1).min(i32::MAX as u64) as i32;
        self.wait_events(timeout_ms);
    }

    pub fn wait_events(&self, timeout_ms: i32) {
        let fd = self.conn.stream().as_raw_fd();
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        unsafe {
            libc::poll(&mut pfd, 1, timeout_ms.max(0));
        }
    }

    fn translate_key(&self, keycode: u8, state_bits: u16) -> Option<WindowEvent> {
        if self.keysyms_per_keycode == 0 || keycode < self.min_keycode {
            return None;
        }
        let idx_base = (keycode - self.min_keycode) as usize * self.keysyms_per_keycode as usize;
        if idx_base >= self.keysyms.len() {
            return None;
        }
        let shift = (state_bits & 1) != 0;
        let caps = (state_bits & 2) != 0;
        let base_sym = self.keysyms[idx_base];
        let shift_sym = if self.keysyms_per_keycode > 1 && idx_base + 1 < self.keysyms.len() {
            self.keysyms[idx_base + 1]
        } else {
            0
        };

        let sym = if shift && shift_sym != 0 {
            shift_sym
        } else {
            base_sym
        };

        match sym {
            0xFF08 => Some(WindowEvent::KeyBackspace),
            0xFF0D | 0xFF8D => Some(WindowEvent::KeyEnter),
            0xFF1B => Some(WindowEvent::KeyEscape),
            0x0020..=0x007E => {
                let mut ch = char::from_u32(sym)?;
                if shift && shift_sym == 0 && ch.is_ascii_lowercase() {
                    ch = ch.to_ascii_uppercase();
                }
                if caps && ch.is_ascii_alphabetic() {
                    ch = if shift {
                        ch.to_ascii_lowercase()
                    } else {
                        ch.to_ascii_uppercase()
                    };
                }
                Some(WindowEvent::KeyChar(ch))
            }
            _ => None,
        }
    }

    pub fn drain_events(&self, out: &mut Vec<WindowEvent>) {
        out.clear();
        while let Ok(Some(ev)) = self.conn.poll_for_event() {
            match ev {
                Event::Expose(_) | Event::MapNotify(_) => out.push(WindowEvent::Expose),
                Event::RandrScreenChangeNotify(_) | Event::RandrNotify(_) => {
                    out.push(WindowEvent::MonitorChanged);
                }
                Event::EnterNotify(e) => out.push(WindowEvent::PointerEnter(e.event_x, e.event_y)),
                Event::LeaveNotify(_) => out.push(WindowEvent::PointerLeave),
                Event::MotionNotify(e) => out.push(WindowEvent::PointerMove(e.event_x, e.event_y)),
                Event::ButtonPress(e) => {
                    out.push(WindowEvent::ButtonDown(e.detail, e.event_x, e.event_y))
                }
                Event::ButtonRelease(e) => {
                    out.push(WindowEvent::ButtonUp(e.detail))
                }
                Event::KeyPress(e) => {
                    if let Some(kev) = self.translate_key(e.detail, u16::from(e.state)) {
                        out.push(kev);
                    }
                }
                _ => {}
            }
        }
    }
}

fn detect_primary_monitor(
    conn: &RustConnection,
    root: u32,
    screen: &xproto::Screen,
) -> (i32, i32, i32, i32) {
    if let Some(primary_reply) = randr::get_output_primary(conn, root)
        .ok()
        .and_then(|c| c.reply().ok())
    {
        let output = primary_reply.output;
        if output != 0 {
            if let Some(out_info) = randr::get_output_info(conn, output, 0)
                .ok()
                .and_then(|c| c.reply().ok())
            {
                if out_info.crtc != 0 {
                    if let Some(crtc) = randr::get_crtc_info(conn, out_info.crtc, 0)
                        .ok()
                        .and_then(|c| c.reply().ok())
                    {
                        if crtc.width > 0 && crtc.height > 0 {
                            return (
                                crtc.x as i32,
                                crtc.y as i32,
                                crtc.width as i32,
                                crtc.height as i32,
                            );
                        }
                    }
                }
            }
        }
    }

    if let Some(res) = randr::get_screen_resources_current(conn, root)
        .ok()
        .and_then(|c| c.reply().ok())
    {
        for &crtc_id in &res.crtcs {
            if let Some(crtc) = randr::get_crtc_info(conn, crtc_id, res.config_timestamp)
                .ok()
                .and_then(|c| c.reply().ok())
            {
                if crtc.width > 0 && crtc.height > 0 {
                    return (
                        crtc.x as i32,
                        crtc.y as i32,
                        crtc.width as i32,
                        crtc.height as i32,
                    );
                }
            }
        }
    }

    (
        0,
        0,
        screen.width_in_pixels as i32,
        screen.height_in_pixels as i32,
    )
}

fn intern(conn: &RustConnection, name: &[u8]) -> Option<u32> {
    conn.intern_atom(false, name)
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.atom)
}

fn set_ewmh_overlay_hints(conn: &RustConnection, win: u32) {
    let _ = conn.change_property8(
        PropMode::REPLACE,
        win,
        AtomEnum::WM_CLASS,
        AtomEnum::STRING,
        b"arwiland\0Arwiland\0",
    );
    let _ = conn.change_property8(
        PropMode::REPLACE,
        win,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        b"Arwiland",
    );

    if let (Some(wm_type), Some(dock)) = (
        intern(conn, b"_NET_WM_WINDOW_TYPE"),
        intern(conn, b"_NET_WM_WINDOW_TYPE_DOCK"),
    ) {
        let _ = conn.change_property32(PropMode::REPLACE, win, wm_type, AtomEnum::ATOM, &[dock]);
    }

    if let Some(extents) = intern(conn, b"_GTK_FRAME_EXTENTS") {
        let _ = conn.change_property32(
            PropMode::REPLACE,
            win,
            extents,
            AtomEnum::CARDINAL,
            &[0, 0, 0, 0],
        );
    }

    if let Some(motif) = intern(conn, b"_MOTIF_WM_HINTS") {
        // flags=2 (decorations), functions=0, decorations=0 (completely borderless / no compositor shadow)
        let _ = conn.change_property32(
            PropMode::REPLACE,
            win,
            motif,
            motif,
            &[2, 0, 0, 0, 0],
        );
    }

    if let Some(wm_state) = intern(conn, b"_NET_WM_STATE") {
        let mut states = Vec::new();
        for name in [
            b"_NET_WM_STATE_ABOVE".as_slice(),
            b"_NET_WM_STATE_STICKY".as_slice(),
            b"_NET_WM_STATE_SKIP_TASKBAR".as_slice(),
            b"_NET_WM_STATE_SKIP_PAGER".as_slice(),
        ] {
            if let Some(a) = intern(conn, name) {
                states.push(a);
            }
        }
        if !states.is_empty() {
            let _ =
                conn.change_property32(PropMode::REPLACE, win, wm_state, AtomEnum::ATOM, &states);
        }
    }
}
