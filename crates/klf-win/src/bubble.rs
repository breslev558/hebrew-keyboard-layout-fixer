//! Small bubble after F10: shown ~1.8-3.5 s (by length), then fades out. Position (1.5.2): centre top of the work area of the target's screen by default (always the same place:
//! the caret is unknown in Chrome/Word, a bubble "near the caret" would jump around), moved to the
//! bottom when the caret is right under it; or centre bottom; or near the caret (setting).
//! It NEVER takes the focus (WS_EX_NOACTIVATE + SW_SHOWNOACTIVATE), so the user's text and
//! selection are not disturbed, and clicks go through it.

use crate::app::instance;
use crate::util::wide;
use std::cell::{Cell, RefCell};
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const TIMER_FADE: usize = 1;
const SHOW_MIN_MS: u32 = 1800;
const SHOW_MAX_MS: u32 = 3500;
const FADE_MS: u32 = 300;
const STEP_MS: u32 = 30;

thread_local! {
    static HWND_: Cell<HWND> = const { Cell::new(null_mut()) };
    static TEXT: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
    static RTL: Cell<bool> = const { Cell::new(false) };
    static OK: Cell<bool> = const { Cell::new(true) };
    static ELAPSED: Cell<u32> = const { Cell::new(0) };
    static FONT: Cell<HFONT> = const { Cell::new(null_mut()) };
    static SHOW_MS: Cell<u32> = const { Cell::new(SHOW_MIN_MS) };
}

/// Display time: 60 ms per character, between 1.8 and 3.5 s.
fn show_ms(message: &str) -> u32 {
    (message.chars().count() as u32 * 60).clamp(SHOW_MIN_MS, SHOW_MAX_MS)
}

/// Screen of reference: the caret's, else the target window's (not minimised), else the mouse's
/// (target 0 at start, window closed or minimised at -32000).
fn monitor_for(caret: Option<(i32, i32)>, target: usize) -> HMONITOR {
    // SAFETY: plain calls; the target may be gone (checked with IsWindow).
    unsafe {
        if let Some((x, y)) = caret {
            return MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        }
        let t = target as HWND;
        if !t.is_null() && IsWindow(t) != 0 && IsIconic(t) == 0 {
            // MonitorFromWindow: right even for a maximised window that overflows by 7-8 px
            return MonitorFromWindow(t, MONITOR_DEFAULTTONEAREST);
        }
        let mut p = POINT { x: 0, y: 0 };
        GetCursorPos(&mut p);
        MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST)
    }
}

fn dpi_of(mon: HMONITOR) -> u32 {
    // SAFETY: plain calls, outputs written.
    unsafe {
        let (mut dx, mut dy) = (96u32, 96u32);
        if GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) < 0 {
            return 96;
        }
        dx
    }
}

fn work_area_of(mon: HMONITOR) -> RECT {
    // SAFETY: structure written.
    unsafe {
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(mon, &mut mi);
        mi.rcWork
    }
}

fn make_font(dpi: u32) -> HFONT {
    let face = wide("Segoe UI Semibold");
    // SAFETY: zeroed LOGFONTW is valid; the face name fits (< 32 units).
    unsafe {
        let mut lf: LOGFONTW = std::mem::zeroed();
        lf.lfHeight = -((11 * dpi as i32 + 36) / 72);
        lf.lfWeight = 600;
        lf.lfQuality = CLEARTYPE_QUALITY;
        lf.lfCharSet = DEFAULT_CHARSET;
        for (d, c) in lf.lfFaceName.iter_mut().zip(face.iter().take(31)) {
            *d = *c;
        }
        CreateFontIndirectW(&lf)
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            // SAFETY: standard painting between BeginPaint/EndPaint; objects freed.
            unsafe {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let dc = BeginPaint(hwnd, &mut ps);
                let mut rc: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rc);
                let ok = OK.with(|c| c.get());
                let hc = crate::platform::sys::high_contrast();
                // high contrast: the system colours of a tooltip (never colour alone)
                let bg = if hc { GetSysColorBrush(COLOR_INFOBK) } else { CreateSolidBrush(if ok { rgb(0x1E, 0x6F, 0x3E) } else { rgb(0x3A, 0x3A, 0x3A) }) };
                FillRect(dc, &rc, bg);
                if !hc {
                    DeleteObject(bg); // a system colour brush is never deleted
                }
                SetBkMode(dc, TRANSPARENT as i32);
                SetTextColor(dc, if hc { GetSysColor(COLOR_INFOTEXT) } else { rgb(255, 255, 255) });
                let old = SelectObject(dc, FONT.with(|f| f.get()));
                let mut flags = DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX;
                if RTL.with(|r| r.get()) {
                    flags |= DT_RTLREADING;
                }
                TEXT.with(|t| {
                    let t = t.borrow();
                    DrawTextW(dc, t.as_ptr(), (t.len() as i32) - 1, &mut rc, flags);
                });
                SelectObject(dc, old);
                EndPaint(hwnd, &ps);
            }
            0
        }
        WM_TIMER if wparam == TIMER_FADE => {
            let e = ELAPSED.with(|c| {
                let v = c.get() + STEP_MS;
                c.set(v);
                v
            });
            // SAFETY: our window.
            unsafe {
                let show = SHOW_MS.with(|c| c.get());
                if e >= show + FADE_MS {
                    KillTimer(hwnd, TIMER_FADE);
                    ShowWindow(hwnd, SW_HIDE);
                } else if e > show {
                    let a = 235 - ((e - show) * 235 / FADE_MS).min(235);
                    SetLayeredWindowAttributes(hwnd, 0, a as u8, LWA_ALPHA);
                }
            }
            0
        }
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_NCHITTEST => HTTRANSPARENT as LRESULT,
        // SAFETY: default handling.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}

fn ensure_window() -> HWND {
    let h = HWND_.with(|c| c.get());
    if !h.is_null() {
        return h;
    }
    let class = wide("KBLayoutFixerBubble");
    // SAFETY: class registered once; window owned by this (UI) thread.
    unsafe {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: instance(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassExW(&wc);
        let h = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            class.as_ptr(),
            null(),
            WS_POPUP,
            0,
            0,
            10,
            10,
            null_mut(),
            null_mut(),
            instance(),
            null(),
        );
        if !h.is_null() {
            // Windows 11: rounded corners (ignored on Windows 10)
            let pref: u32 = 2; // DWMWCP_ROUND
            DwmSetWindowAttribute(h, 33, &pref as *const u32 as *const _, 4);
            // left out of screen captures and shares (Teams, recordings), Windows 10 2004 and
            // later only: older versions would show a black box instead
            // 1.9.0: KLF_TEST_CAPTURE=1 (visual test on GitHub's Windows only) lets the bubble be
            // photographed; otherwise it is never in a screenshot (it can show the fixed text)
            if crate::platform::sys::windows_build() >= 19041 && std::env::var("KLF_TEST_CAPTURE").as_deref() != Ok("1") {
                SetWindowDisplayAffinity(h, WDA_EXCLUDEFROMCAPTURE);
            }
        }
        HWND_.with(|c| c.set(h));
        h
    }
}

/// Shows the bubble. `caret`: screen point under the text caret, if known.
pub fn show(message: &str, rtl: bool, caret: Option<(i32, i32)>, target: usize, ok: bool) {
    show_at(crate::app::settings().bubble_position, message, rtl, caret, target, ok);
}

/// The language / Caps Lock sign keeps its place near the caret (as before 1.5.2).
pub fn show_near_caret(message: &str, rtl: bool, caret: Option<(i32, i32)>, target: usize) {
    show_at(klf_core::settings::BUBBLE_CARET, message, rtl, caret, target, false);
}

fn show_at(pos: u32, message: &str, rtl: bool, caret: Option<(i32, i32)>, target: usize, ok: bool) {
    let hwnd = ensure_window();
    if hwnd.is_null() {
        return;
    }
    let mon = monitor_for(caret, target);
    let dpi = dpi_of(mon);
    let old = FONT.with(|f| f.replace(make_font(dpi)));
    if !old.is_null() {
        // SAFETY: previous font, no longer selected anywhere.
        unsafe { DeleteObject(old) };
    }
    TEXT.with(|t| *t.borrow_mut() = wide(message));
    RTL.with(|r| r.set(rtl));
    OK.with(|c| c.set(ok));
    ELAPSED.with(|c| c.set(0));
    SHOW_MS.with(|c| c.set(show_ms(message)));

    // size of the text
    let (tw, th) = TEXT.with(|t| {
        let t = t.borrow();
        // SAFETY: memory DC only for measuring; released.
        unsafe {
            let dc = GetDC(null_mut());
            let oldf = SelectObject(dc, FONT.with(|f| f.get()));
            let mut rc = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            DrawTextW(dc, t.as_ptr(), (t.len() as i32) - 1, &mut rc, DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX);
            SelectObject(dc, oldf);
            ReleaseDC(null_mut(), dc);
            (rc.right - rc.left, rc.bottom - rc.top)
        }
    });
    let sc = |v: i32| v * dpi as i32 / 96;
    let (w, h) = (tw + 2 * sc(16), th + 2 * sc(8));
    let wa = work_area_of(mon);
    let (x, y) = klf_core::bubble::place(pos, caret, (w, h), (wa.left, wa.top, wa.right, wa.bottom), sc(12), sc(6), sc(24));
    // SAFETY: our window; no activation anywhere.
    unsafe {
        SetLayeredWindowAttributes(hwnd, 0, 235, LWA_ALPHA);
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        InvalidateRect(hwnd, null(), 1);
        KillTimer(hwnd, TIMER_FADE);
        SetTimer(hwnd, TIMER_FADE, STEP_MS, None);
    }
}

