use std::cell::RefCell;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr;

use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, COLORREF, HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EnumDisplaySettingsW,
    GetMonitorInfoW, MonitorFromPoint, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DEVMODEW, DIB_RGB_COLORS, ENUM_CURRENT_SETTINGS,
    HBITMAP, HDC, HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    CreateWaitableTimerExW, GetCurrentProcess, SetProcessWorkingSetSize, SetWaitableTimer,
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, INFINITE, TIMER_ALL_ACCESS,
};
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_ALLOWMULTISELECT, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR,
    OPENFILENAMEW,
};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VK_ESCAPE,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetWindowLongPtrW,
    LoadCursorW, MsgWaitForMultipleObjectsEx, PeekMessageW, RegisterClassW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow,
    UpdateLayeredWindowIndirect, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HWND_TOPMOST, IDC_ARROW,
    MB_ICONASTERISK, MSG, MWMO_INPUTAVAILABLE, PM_REMOVE, QS_ALLINPUT, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE, ULW_ALPHA, UPDATELAYEREDWINDOWINFO, WM_CHAR,
    WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT,
    WM_RBUTTONDOWN, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP, WS_VISIBLE,
};

#[derive(Debug, Clone, Copy)]
pub enum WindowEvent {
    Expose,
    #[allow(dead_code)]
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

struct WndState {
    events: RefCell<Vec<WindowEvent>>,
    tracking_mouse: RefCell<bool>,
}

pub struct Win32OverlayWindow {
    hwnd: HWND,
    hdc_mem: HDC,
    hbmp: HBITMAP,
    old_obj: HGDIOBJ,
    dib_bits: *mut u8,
    state_ptr: *mut WndState,
    frame_timer: HANDLE,
    pub width: u16,
    pub height: u16,
    win_x: i32,
    win_y: i32,
    is_bottom: bool,
    y_offset: i32,
    prev_blit_h: u16,
    full_present_needed: bool,
    refresh_hz: u32,
}

fn get_x_lparam(lp: LPARAM) -> i16 {
    (lp.0 & 0xFFFF) as u16 as i16
}

fn get_y_lparam(lp: LPARAM) -> i16 {
    ((lp.0 >> 16) & 0xFFFF) as u16 as i16
}

fn primary_monitor_rect() -> (i32, i32, i32, i32) {
    unsafe {
        let hmon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(hmon, &mut info).as_bool() {
            let r = info.rcMonitor;
            return (r.left, r.top, r.right - r.left, r.bottom - r.top);
        }
    }
    (0, 0, 1920, 1080)
}

fn primary_monitor_refresh_hz() -> u32 {
    unsafe {
        let mut dm: DEVMODEW = std::mem::zeroed();
        dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        if EnumDisplaySettingsW(PCWSTR::null(), ENUM_CURRENT_SETTINGS, &mut dm).as_bool()
            && dm.dmDisplayFrequency >= 30
        {
            return dm.dmDisplayFrequency.clamp(60, 120);
        }
    }
    60
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WndState;
    if !ptr.is_null() {
        let st = &*ptr;
        match msg {
            WM_PAINT => {
                st.events.borrow_mut().push(WindowEvent::Expose);
            }
            WM_MOUSEMOVE => {
                let x = get_x_lparam(lparam);
                let y = get_y_lparam(lparam);
                let mut tracking = st.tracking_mouse.borrow_mut();
                if !*tracking {
                    *tracking = true;
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                    st.events.borrow_mut().push(WindowEvent::PointerEnter(x, y));
                }
                st.events.borrow_mut().push(WindowEvent::PointerMove(x, y));
                return LRESULT(0);
            }
            WM_MOUSELEAVE => {
                *st.tracking_mouse.borrow_mut() = false;
                st.events.borrow_mut().push(WindowEvent::PointerLeave);
                return LRESULT(0);
            }
            WM_LBUTTONDOWN => {
                let x = get_x_lparam(lparam);
                let y = get_y_lparam(lparam);
                SetCapture(hwnd);
                st.events
                    .borrow_mut()
                    .push(WindowEvent::ButtonDown(1, x, y));
                return LRESULT(0);
            }
            WM_LBUTTONUP => {
                let _ = ReleaseCapture();
                st.events.borrow_mut().push(WindowEvent::ButtonUp(1));
                return LRESULT(0);
            }
            WM_RBUTTONDOWN => {
                let x = get_x_lparam(lparam);
                let y = get_y_lparam(lparam);
                st.events
                    .borrow_mut()
                    .push(WindowEvent::ButtonDown(3, x, y));
                return LRESULT(0);
            }
            WM_MOUSEWHEEL => {
                let delta = ((wparam.0 >> 16) & 0xFFFF) as u16 as i16;
                let btn = if delta > 0 { 4 } else { 5 };
                st.events
                    .borrow_mut()
                    .push(WindowEvent::ButtonDown(btn, 0, 0));
                return LRESULT(0);
            }
            WM_KEYDOWN => {
                if wparam.0 as u16 == VK_ESCAPE.0 {
                    st.events.borrow_mut().push(WindowEvent::KeyEscape);
                    return LRESULT(0);
                }
            }
            WM_CHAR => {
                let code = wparam.0 as u32;
                match code {
                    0x08 => st.events.borrow_mut().push(WindowEvent::KeyBackspace),
                    0x0D => st.events.borrow_mut().push(WindowEvent::KeyEnter),
                    0x1B => st.events.borrow_mut().push(WindowEvent::KeyEscape),
                    0x20..=0x7E => {
                        if let Some(ch) = char::from_u32(code) {
                            st.events.borrow_mut().push(WindowEvent::KeyChar(ch));
                        }
                    }
                    _ => {}
                }
                return LRESULT(0);
            }
            _ => {}
        }
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

impl Win32OverlayWindow {
    pub fn new(width: u16, height: u16, is_bottom: bool, y_offset: i32) -> Result<Self, String> {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

            let hinstance = GetModuleHandleW(None)
                .map_err(|e| format!("GetModuleHandleW failed: {e}"))?;
            let class_name = w!("ArwilandOverlayClass");

            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassW(&wc);

            let (mon_x, mon_y, mon_w, mon_h) = primary_monitor_rect();
            let y_off = y_offset.clamp(0, 64);
            let win_x = mon_x + ((mon_w - width as i32) / 2).max(0);
            let win_y = if is_bottom {
                mon_y + mon_h - height as i32 - y_off
            } else {
                mon_y + y_off
            };

            let ex_style =
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
            let style = WS_POPUP | WS_VISIBLE;

            let hwnd = CreateWindowExW(
                ex_style,
                class_name,
                w!("Arwiland"),
                style,
                win_x,
                win_y,
                width as i32,
                height as i32,
                None,
                None,
                hinstance,
                None,
            )
            .map_err(|e| format!("CreateWindowExW failed: {e}"))?;

            let state_box = Box::new(WndState {
                events: RefCell::new(Vec::with_capacity(32)),
                tracking_mouse: RefCell::new(false),
            });
            let state_ptr = Box::into_raw(state_box);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);

            // Allocate persistent 32-bit top-down BGRA DIBSection for zero-alloc UpdateLayeredWindow blits
            let hdc_mem = CreateCompatibleDC(HDC::default());
            if hdc_mem.is_invalid() {
                let _ = DestroyWindow(hwnd);
                drop(Box::from_raw(state_ptr));
                return Err("CreateCompatibleDC failed".to_string());
            }

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32), // top-down DIB matching tiny_skia row order
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut bits: *mut std::ffi::c_void = ptr::null_mut();
            let hbmp = CreateDIBSection(hdc_mem, &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
                .map_err(|e| {
                    let _ = DeleteDC(hdc_mem);
                    let _ = DestroyWindow(hwnd);
                    drop(Box::from_raw(state_ptr));
                    format!("CreateDIBSection failed: {e}")
                })?;

            let old_obj = SelectObject(hdc_mem, hbmp);
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

            let frame_timer = CreateWaitableTimerExW(
                None,
                PCWSTR::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS.0,
            )
            .or_else(|_| CreateWaitableTimerExW(None, PCWSTR::null(), 0, TIMER_ALL_ACCESS.0))
            .unwrap_or_default();

            let refresh_hz = primary_monitor_refresh_hz();

            Ok(Self {
                hwnd,
                hdc_mem,
                hbmp,
                old_obj,
                dib_bits: bits as *mut u8,
                state_ptr,
                frame_timer,
                width,
                height,
                win_x,
                win_y,
                is_bottom,
                y_offset: y_off,
                prev_blit_h: height,
                full_present_needed: true,
                refresh_hz,
            })
        }
    }

    pub fn refresh_rate_hz(&self) -> u32 {
        self.refresh_hz
    }

    #[allow(dead_code)]
    pub fn sync_monitor_and_raise(&mut self) {
        self.set_position(self.is_bottom, self.y_offset);
    }

    pub fn set_position(&mut self, is_bottom: bool, y_offset: i32) {
        let y_off = y_offset.clamp(0, 64);
        self.is_bottom = is_bottom;
        self.y_offset = y_off;
        self.prev_blit_h = self.height;
        self.full_present_needed = true;
        let (mon_x, mon_y, mon_w, mon_h) = primary_monitor_rect();
        self.win_x = mon_x + ((mon_w - self.width as i32) / 2).max(0);
        self.win_y = if is_bottom {
            mon_y + mon_h - self.height as i32 - y_off
        } else {
            mon_y + y_off
        };
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                self.win_x,
                self.win_y,
                self.width as i32,
                self.height as i32,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    /// On Windows `WS_EX_LAYERED` + `ULW_ALPHA`, every pixel with alpha=0 is natively click-through.
    pub fn set_input_region(&self, _pill_width: i32, _pill_height: i32) {}

    pub fn raise(&self) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    pub fn grab_keyboard_focus(&self) {
        unsafe {
            let _ = SetForegroundWindow(self.hwnd);
            let _ = SetFocus(self.hwnd);
        }
    }

    /// Copies premultiplied RGBA -> premultiplied BGRA only for dirty rows and uploads via UpdateLayeredWindowIndirect.
    pub fn present(&mut self, pixmap: &mut tiny_skia::Pixmap, active_h: u16) {
        let src = pixmap.data();
        let row_bytes = (self.width as usize) * 4;
        let total = row_bytes * (self.height as usize);
        if src.len() < total || self.dib_bits.is_null() {
            return;
        }

        let clamped_active = active_h.min(self.height).max(1);
        let blit_h = if self.full_present_needed {
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
        let byte_start = y_start as usize * row_bytes;
        let byte_end = y_end as usize * row_bytes;

        unsafe {
            let dst = std::slice::from_raw_parts_mut(self.dib_bits, total);
            for (s, d) in src[byte_start..byte_end]
                .chunks_exact(4)
                .zip(dst[byte_start..byte_end].chunks_exact_mut(4))
            {
                let px = u32::from_ne_bytes([s[0], s[1], s[2], s[3]]);
                let bgra = (px & 0xFF00_FF00)
                    | ((px & 0x00FF_0000) >> 16)
                    | ((px & 0x0000_00FF) << 16);
                d.copy_from_slice(&bgra.to_ne_bytes());
            }

            let pt_dst = POINT {
                x: self.win_x,
                y: self.win_y,
            };
            let size = SIZE {
                cx: self.width as i32,
                cy: self.height as i32,
            };
            let pt_src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let dirty = RECT {
                left: 0,
                top: y_start as i32,
                right: self.width as i32,
                bottom: y_end as i32,
            };
            let info = UPDATELAYEREDWINDOWINFO {
                cbSize: std::mem::size_of::<UPDATELAYEREDWINDOWINFO>() as u32,
                hdcDst: HDC::default(),
                pptDst: &pt_dst,
                psize: &size,
                hdcSrc: self.hdc_mem,
                pptSrc: &pt_src,
                crKey: COLORREF(0),
                pblend: &blend,
                dwFlags: ULW_ALPHA,
                prcDirty: if self.full_present_needed {
                    ptr::null()
                } else {
                    &dirty
                },
            };
            if !UpdateLayeredWindowIndirect(self.hwnd, &info).as_bool() {
                let _ = UpdateLayeredWindow(
                    self.hwnd,
                    HDC::default(),
                    Some(&pt_dst),
                    Some(&size),
                    self.hdc_mem,
                    Some(&pt_src),
                    COLORREF(0),
                    Some(&blend),
                    ULW_ALPHA,
                );
            }
            self.full_present_needed = false;
        }
    }

    pub fn wait_events_us(&self, timeout_us: u64) {
        if timeout_us == 0 {
            return;
        }
        unsafe {
            if !self.frame_timer.is_invalid() {
                let due_time: i64 = -((timeout_us as i64).saturating_mul(10));
                if SetWaitableTimer(self.frame_timer, &due_time, 0, None, None, false).is_ok() {
                    let handles = [self.frame_timer];
                    let _ = MsgWaitForMultipleObjectsEx(
                        Some(&handles),
                        INFINITE,
                        QS_ALLINPUT,
                        MWMO_INPUTAVAILABLE,
                    );
                    return;
                }
            }
            let timeout_ms = ((timeout_us + 999) / 1000).min(u32::MAX as u64) as u32;
            let _ = MsgWaitForMultipleObjectsEx(
                None,
                timeout_ms,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            );
        }
    }

    #[allow(dead_code)]
    pub fn wait_events(&self, timeout_ms: i32) {
        self.wait_events_us((timeout_ms.max(0) as u64) * 1000);
    }

    pub fn drain_events(&self, out: &mut Vec<WindowEvent>) {
        out.clear();
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if !self.state_ptr.is_null() {
                let st = &*self.state_ptr;
                out.extend(st.events.borrow_mut().drain(..));
            }
        }
    }
}

impl Drop for Win32OverlayWindow {
    fn drop(&mut self) {
        unsafe {
            if !self.frame_timer.is_invalid() {
                let _ = CloseHandle(self.frame_timer);
            }
            if !self.hdc_mem.is_invalid() {
                SelectObject(self.hdc_mem, self.old_obj);
                let _ = DeleteObject(self.hbmp);
                let _ = DeleteDC(self.hdc_mem);
            }
            if !self.hwnd.is_invalid() {
                SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
                let _ = DestroyWindow(self.hwnd);
            }
            if !self.state_ptr.is_null() {
                drop(Box::from_raw(self.state_ptr));
                self.state_ptr = ptr::null_mut();
            }
        }
    }
}

pub fn trim_working_set() {
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

pub fn play_alert_beep() {
    unsafe {
        let _ = MessageBeep(MB_ICONASTERISK);
    }
}

pub fn open_file_default(path: &str) {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        ShellExecuteW(
            HWND::default(),
            w!("open"),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
}

pub fn pick_files_dialog() -> Vec<PathBuf> {
    let mut buf = vec![0u16; 32768];
    let filter: Vec<u16> = "Semua File\0*.*\0\0".encode_utf16().collect();
    let title: Vec<u16> = "Pilih File untuk Rak Arwiland\0".encode_utf16().collect();

    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(buf.as_mut_ptr()),
        nMaxFile: buf.len() as u32,
        lpstrTitle: PCWSTR(title.as_ptr()),
        Flags: OFN_EXPLORER | OFN_ALLOWMULTISELECT | OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };

    let ok = unsafe { GetOpenFileNameW(&mut ofn).as_bool() };
    if !ok {
        return Vec::new();
    }

    // Parse null-separated result: single file => [full_path], multi => [dir, file1, file2, ...]
    let mut parts: Vec<OsString> = Vec::new();
    let mut start = 0usize;
    while start < buf.len() {
        let mut end = start;
        while end < buf.len() && buf[end] != 0 {
            end += 1;
        }
        if end == start {
            break;
        }
        parts.push(OsString::from_wide(&buf[start..end]));
        start = end + 1;
    }

    if parts.is_empty() {
        Vec::new()
    } else if parts.len() == 1 {
        vec![PathBuf::from(&parts[0])]
    } else {
        let dir = PathBuf::from(&parts[0]);
        parts[1..].iter().map(|f| dir.join(f)).collect()
    }
}
