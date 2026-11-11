use cairo::{RectangleInt, Region};
use gdk4_x11::{X11Display, X11Surface};
use gtk4::glib::translate::ToGlibPtr;
use gtk4::prelude::*;
use std::ffi::{c_int, c_long, c_uint, c_ulong, c_void};

type Display = c_void;
type Window = c_ulong;
type Time = c_ulong;

const CW_OVERRIDE_REDIRECT: c_ulong = 1 << 9;
const REVERT_TO_POINTER_ROOT: c_int = 1;
const CURRENT_TIME: Time = 0;

#[repr(C)]
struct XSetWindowAttributes {
    background_pixmap: c_ulong,
    background_pixel: c_ulong,
    border_pixmap: c_ulong,
    border_pixel: c_ulong,
    bit_gravity: c_int,
    win_gravity: c_int,
    backing_store: c_int,
    backing_planes: c_ulong,
    backing_pixel: c_ulong,
    save_under: c_int,
    event_mask: c_long,
    do_not_propagate_mask: c_long,
    override_redirect: c_int,
    colormap: c_ulong,
    cursor: c_ulong,
}

#[repr(C)]
struct XWindowAttributes {
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    border_width: c_int,
    depth: c_int,
    visual: *mut c_void,
    root: Window,
    class: c_int,
    bit_gravity: c_int,
    win_gravity: c_int,
    backing_store: c_int,
    backing_planes: c_ulong,
    backing_pixel: c_ulong,
    save_under: c_int,
    colormap: c_ulong,
    map_installed: c_int,
    map_state: c_int,
    all_event_masks: c_long,
    your_event_mask: c_long,
    do_not_propagate_mask: c_long,
    override_redirect: c_int,
    screen: *mut c_void,
}

#[link(name = "X11")]
extern "C" {
    fn XMoveWindow(display: *mut Display, w: Window, x: c_int, y: c_int) -> c_int;
    fn XMoveResizeWindow(
        display: *mut Display,
        w: Window,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
    ) -> c_int;
    fn XRaiseWindow(display: *mut Display, w: Window) -> c_int;
    fn XChangeWindowAttributes(
        display: *mut Display,
        w: Window,
        valuemask: c_ulong,
        attributes: *mut XSetWindowAttributes,
    ) -> c_int;
    fn XGetWindowAttributes(
        display: *mut Display,
        w: Window,
        window_attributes_return: *mut XWindowAttributes,
    ) -> c_int;
    fn XSetInputFocus(display: *mut Display, focus: Window, revert_to: c_int, time: Time) -> c_int;
    fn XFlush(display: *mut Display) -> c_int;
    fn XDefaultRootWindow(display: *mut Display) -> Window;
    fn XTranslateCoordinates(
        display: *mut Display,
        src_w: Window,
        dest_w: Window,
        src_x: c_int,
        src_y: c_int,
        dest_x_return: *mut c_int,
        dest_y_return: *mut c_int,
        child_return: *mut Window,
    ) -> c_int;
}

extern "C" {
    fn gdk_x11_display_get_xdisplay(display: *mut c_void) -> *mut Display;
}

pub fn get_primary_monitor_geometry() -> (i32, i32, i32, i32) {
    if let Some(display) = gtk4::gdk::Display::default() {
        let monitors = display.monitors();
        if let Some(obj) = monitors.item(0) {
            if let Ok(monitor) = obj.downcast::<gtk4::gdk::Monitor>() {
                let geom = monitor.geometry();
                return (geom.x(), geom.y(), geom.width(), geom.height());
            }
        }
    }
    (0, 0, 1920, 1080)
}

fn get_x11_handles(window: &gtk4::ApplicationWindow) -> Option<(*mut Display, Window)> {
    let surface = window.surface()?;
    let x11_surface = surface.downcast::<X11Surface>().ok()?;
    let x11_display = x11_surface.display().downcast::<X11Display>().ok()?;
    let xid: Window = x11_surface.xid() as Window;
    let disp_ptr: *mut gdk4_x11::ffi::GdkX11Display = x11_display.to_glib_none().0;
    let xdisplay: *mut Display = unsafe { gdk_x11_display_get_xdisplay(disp_ptr as *mut c_void) };
    if xdisplay.is_null() || xid == 0 {
        None
    } else {
        Some((xdisplay, xid))
    }
}

pub fn configure_pre_map_overlay(
    window: &gtk4::ApplicationWindow,
    canvas_width: i32,
    canvas_height: i32,
    y_offset: i32,
) {
    let Some((xdisplay, xid)) = get_x11_handles(window) else {
        return;
    };

    let (mon_x, mon_y, mon_w, _mon_h) = get_primary_monitor_geometry();
    let target_x = mon_x + (mon_w - canvas_width) / 2;
    let target_y = mon_y + y_offset;

    unsafe {
        let mut attrs: XSetWindowAttributes = std::mem::zeroed();
        attrs.override_redirect = 1;
        XChangeWindowAttributes(xdisplay, xid, CW_OVERRIDE_REDIRECT, &mut attrs);
        XMoveResizeWindow(
            xdisplay,
            xid,
            target_x as c_int,
            target_y as c_int,
            canvas_width as c_uint,
            canvas_height as c_uint,
        );
        XFlush(xdisplay);
    }
}

pub fn setup_top_center_overlay(
    window: &gtk4::ApplicationWindow,
    canvas_width: i32,
    _canvas_height: i32,
    y_offset: i32,
) {
    let Some((xdisplay, xid)) = get_x11_handles(window) else {
        return;
    };

    let (mon_x, mon_y, mon_w, _mon_h) = get_primary_monitor_geometry();
    let target_x = mon_x + (mon_w - canvas_width) / 2;
    let target_y = mon_y + y_offset;

    unsafe {
        XMoveWindow(xdisplay, xid, target_x as c_int, target_y as c_int);
        XRaiseWindow(xdisplay, xid);
        XFlush(xdisplay);

        let root = XDefaultRootWindow(xdisplay);
        let mut rx: c_int = 0;
        let mut ry: c_int = 0;
        let mut child: Window = 0;
        let mut wattrs: XWindowAttributes = std::mem::zeroed();
        XGetWindowAttributes(xdisplay, xid, &mut wattrs);
        XTranslateCoordinates(xdisplay, xid, root, 0, 0, &mut rx, &mut ry, &mut child);
        eprintln!(
            "[Arwiland X11] xid=0x{:x} depth={} pos=({}, {})",
            xid, wattrs.depth, rx, ry
        );
    }
}

/// Sets the clickable/hoverable input region of the 32-bit RGBA transparent window.
/// Unlike SHAPE_BOUNDING, this does NOT clip GTK4's anti-aliased OpenGL rounded corners or shadows,
/// and is lightweight so the VSync animation runs at a locked 60/120 FPS!
pub fn update_pill_input_region(
    window: &gtk4::ApplicationWindow,
    canvas_width: i32,
    pill_width: i32,
    pill_height: i32,
) {
    let Some(surface) = window.surface() else {
        return;
    };
    let px = ((canvas_width - pill_width) / 2).max(0);
    let rect = RectangleInt::new(px, 0, pill_width, pill_height + 4);
    let region = Region::create_rectangle(&rect);
    surface.set_input_region(&region);
}

pub fn grab_keyboard_focus(window: &gtk4::ApplicationWindow) {
    if let Some((xdisplay, xid)) = get_x11_handles(window) {
        unsafe {
            XRaiseWindow(xdisplay, xid);
            XSetInputFocus(xdisplay, xid, REVERT_TO_POINTER_ROOT, CURRENT_TIME);
            XFlush(xdisplay);
        }
    }
}
