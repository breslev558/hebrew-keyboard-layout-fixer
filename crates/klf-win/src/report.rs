//! Anonymous problem reports (1.6.1). PUBLIC per-user install only: never on a PC with an
//! administrator policy (school) nor on another install (same rule as the updates).
//! - after a real error (clipboard not put back, paste failed) or a crash of the previous
//!   session: a Windows notification asks "Send an anonymous report?"; one click sends it;
//! - tray menu "Report a problem…": a window with an optional comment and EXACTLY what is sent;
//! - "Send reports without asking" (off by default) and "Never offer a report".
//!
//! What is sent is built by klf-core/report.rs (fixed technical fields, never the text).

use crate::app;
use crate::util::{log, wide};
use klf_core::i18n::{text, Msg};
use klf_core::report as r;
use std::cell::{Cell, RefCell};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow, SystemParametersInfoForDpi};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// WPARAM of WM_APP_REPORT: 0 sent, 1 refused by the receiver, 2 network problem (kept)
pub const WM_APP_REPORT: u32 = WM_APP + 7;
/// the report waiting for the user's click (the last problem only)
static PENDING: Mutex<Option<String>> = Mutex::new(None);
/// reports the user agreed to send, sent one after the other by one worker thread
static QUEUE: Mutex<Vec<(String, bool)>> = Mutex::new(Vec::new());
static SENDING: AtomicBool = AtomicBool::new(false);
/// "always send": at most 5 reports per session (never a flood)
static AUTO_SENT: AtomicU32 = AtomicU32::new(0);
/// the notification asks at most once every 30 minutes
static LAST_ASK: AtomicU64 = AtomicU64::new(0);
const ASK_EVERY_MS: u64 = 30 * 60 * 1000;

/// Public per-user install, no administrator policy: update::allowed(true) checks the policy key
/// and the install folder, whatever the "Check for updates" setting.
pub fn enabled() -> bool {
    crate::update::allowed(true)
}

pub fn pending() -> bool {
    PENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_some()
}

fn windows() -> String {
    format!("10.0.{}", crate::platform::sys::windows_build())
}

fn tick() -> u64 {
    // SAFETY: no arguments.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}

/// After each conversion (UI thread; the attempt was recorded before WM_APP_DONE was posted).
pub fn after_conversion(hwnd: HWND, msg: Option<Msg>) {
    if !r::reportable(msg) || !enabled() {
        return;
    }
    let a = crate::diagnostic::last_attempt();
    offer(hwnd, r::build("erreur", klf_core::VERSION, &windows(), a.as_ref(), ""));
}

fn offer(hwnd: HWND, body: String) {
    match app::settings().report_mode {
        r::NEVER => {}
        r::ALWAYS => {
            if AUTO_SENT.fetch_add(1, Ordering::SeqCst) < 5 {
                send(hwnd, body, false);
            }
        }
        _ => {
            *PENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(body);
            let (now, last) = (tick(), LAST_ASK.load(Ordering::SeqCst));
            if (last == 0 || now.saturating_sub(last) >= ASK_EVERY_MS) && app::show_report_notification() {
                LAST_ASK.store(now.max(1), Ordering::SeqCst);
            }
        }
    }
}

/// The panic hook notes only where it stopped in our code (never the message).
pub fn note_crash(location: Option<&std::panic::Location>) {
    if let (Some(dir), Some(l)) = (crate::util::app_dir(), location) {
        let _ = std::fs::write(dir.join("crash.txt"), format!("{}:{}", l.file(), l.line()));
    }
}

fn today() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0)
}

/// At start: the previous session crashed -> offer a report, at most once a day (a program that
/// crashes at every start must not ask or send at every start). The marker is always removed.
pub fn at_start(hwnd: HWND) {
    let Some(dir) = crate::util::app_dir() else { return };
    let Ok(place) = std::fs::read_to_string(dir.join("crash.txt")) else { return };
    let _ = std::fs::remove_file(dir.join("crash.txt"));
    if !enabled() {
        return;
    }
    let mark = dir.join("crash-report-day.txt");
    let last = std::fs::read_to_string(&mark).ok().and_then(|t| t.trim().parse().ok());
    if !r::crash_offer_due(last, today()) {
        return;
    }
    let _ = std::fs::write(&mark, today().to_string());
    let place: String = place.chars().take(200).collect();
    offer(hwnd, r::build_crash(klf_core::VERSION, &windows(), place.trim()));
}

/// Notification or menu click: sends the waiting report.
pub fn send_pending(hwnd: HWND) {
    let body = PENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
    if let Some(body) = body {
        send(hwnd, body, true);
    }
}

/// Queues a report the user agreed to send; one worker thread sends the queue (a click during
/// another sending is never lost). `tell`: a bubble says how it went (the user asked for it).
fn send(hwnd: HWND, body: String, tell: bool) {
    QUEUE.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((body, tell));
    if SENDING.swap(true, Ordering::SeqCst) {
        return; // the running thread takes it
    }
    let owner = hwnd as usize;
    let _ = std::thread::Builder::new().name("report".into()).spawn(move || loop {
        let next = {
            let mut q = QUEUE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if q.is_empty() {
                SENDING.store(false, Ordering::SeqCst);
                None
            } else {
                Some(q.remove(0))
            }
        };
        let Some((body, tell)) = next else { break };
        let status = crate::update::post_json(r::REPORT_URL, &body);
        let code = match r::sent(status) {
            r::Sent::Ok => {
                log("Anonymous report sent");
                0
            }
            r::Sent::Refused => {
                log(&format!("Anonymous report refused by the receiver ({})", status.unwrap_or(0)));
                1
            }
            r::Sent::Retry => {
                log("Anonymous report NOT sent (network), kept for the menu");
                if tell {
                    PENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get_or_insert(body);
                }
                2
            }
        };
        if tell {
            // SAFETY: posting a message without pointers.
            unsafe { PostMessageW(owner as HWND, WM_APP_REPORT, code, 0) };
        }
    });
}

pub fn on_result(code: WPARAM) {
    let l = app::lang();
    crate::bubble::show(text(l, if code == 0 { Msg::ReportSent } else { Msg::ReportFailed }), l.is_rtl(), None, 0, code == 0);
}

/// Menu "Always send" / "Never offer": ticking one unticks the other.
pub fn set_mode(mode: u32) {
    let mut s = app::user_settings();
    s.report_mode = if s.report_mode == mode { r::ASK } else { mode };
    if s.report_mode == r::NEVER {
        PENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
    }
    if app::save_settings(&s) {
        app::apply_settings(s);
    }
}

// ---------------------------------------------------------------------------------------------
// "Report a problem…" window: optional comment + exactly what will be sent

// as in settings_ui.rs (windows-sys gives these with other types or not at all)
const BS_PUSHBUTTON: u32 = 0x0000;
const ES_LEFT: u32 = 0x0000;
const ES_RIGHT: u32 = 0x0002;
const ES_AUTOVSCROLL: u32 = 0x0040;
const ES_MULTILINE: u32 = 0x0004;
const ES_READONLY: u32 = 0x0800;
const ES_WANTRETURN: u32 = 0x1000;
const EN_CHANGE: u32 = 0x0300;
const EM_SETLIMITTEXT: u32 = 0x00C5;
const SS_LEFT: u32 = 0;
const SS_RIGHT: u32 = 2;

const ID_COMMENT: i32 = 10;
const ID_PREVIEW: i32 = 11;
const ID_SEND: i32 = 12;
const ID_CANCEL: i32 = 13;
const ID_LBL1: i32 = 14;
const ID_LBL2: i32 = 15;

thread_local! {
    static WIN: Cell<HWND> = const { Cell::new(null_mut()) };
    static FONT: Cell<HFONT> = const { Cell::new(null_mut()) };
    /// the report shown in the preview: exactly this one is sent
    static SHOWN: RefCell<String> = const { RefCell::new(String::new()) };
}
static REGISTERED: AtomicBool = AtomicBool::new(false);

pub fn window() -> HWND {
    WIN.with(|w| w.get())
}

fn item(id: i32) -> HWND {
    // SAFETY: child of our window (null if closed).
    unsafe { GetDlgItem(window(), id) }
}

fn comment() -> String {
    let h = item(ID_COMMENT);
    let mut buf = vec![0u16; 2048];
    // SAFETY: buffer of the given size.
    let n = unsafe { GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n.min(buf.len())]).replace("\r\n", "\n")
}

fn refresh_preview() {
    let body = r::build("signalement", klf_core::VERSION, &windows(), crate::diagnostic::last_attempt().as_ref(), &comment());
    let t = wide(&r::pretty(&body));
    SHOWN.with(|s| *s.borrow_mut() = body);
    // SAFETY: child of our window; string outlives the call.
    unsafe { SetWindowTextW(item(ID_PREVIEW), t.as_ptr()) };
}

pub fn open() {
    if !enabled() {
        return;
    }
    let existing = window();
    if !existing.is_null() {
        // SAFETY: our window.
        unsafe {
            ShowWindow(existing, SW_RESTORE);
            SetForegroundWindow(existing);
        }
        return;
    }
    let l = app::lang();
    let rtl = l.is_rtl();
    let class = wide("KLFReportWindow");
    let title = wide(&format!("{} – {}", text(l, Msg::MenuReportProblem).trim_end_matches('…'), text(l, Msg::WindowTitle)));
    // SAFETY: class registered once; strings outlive the calls; children of our window.
    unsafe {
        if !REGISTERED.load(Ordering::SeqCst) {
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                hInstance: app::instance(),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: (COLOR_BTNFACE + 1) as usize as HBRUSH,
                hIcon: app::app_icon(GetSystemMetrics(SM_CXICON)),
                hIconSm: app::app_icon(GetSystemMetrics(SM_CXSMICON)),
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            if RegisterClassExW(&wc) == 0 {
                log("Report window class could not be registered");
                return;
            }
            REGISTERED.store(true, Ordering::SeqCst);
        }
        // not mirrored (the JSON preview must stay left-to-right): Hebrew texts are right-aligned
        // and read right-to-left control by control instead
        let ex = WS_EX_CONTROLPARENT | WS_EX_APPWINDOW | if rtl { WS_EX_RTLREADING } else { 0 };
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
        let hwnd = CreateWindowExW(ex, class.as_ptr(), title.as_ptr(), style, CW_USEDEFAULT, CW_USEDEFAULT, 100, 100, null_mut(), null_mut(), app::instance(), null());
        if hwnd.is_null() {
            log("Report window could not be created");
            return;
        }
        WIN.with(|w| w.set(hwnd));
        let dpi = GetDpiForWindow(hwnd).max(96);
        let sc = |v: i32| v * dpi as i32 / 96;
        let mut ncm: NONCLIENTMETRICSW = std::mem::zeroed();
        ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
        SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS, ncm.cbSize, &mut ncm as *mut _ as *mut _, 0, dpi);
        let font = CreateFontIndirectW(&ncm.lfMessageFont);
        FONT.with(|f| f.set(font));
        let (w, m) = (sc(520), sc(12));
        let inner = w - 2 * m;
        let (rd_ex, st_align, ed_align) = if rtl { (WS_EX_RTLREADING | WS_EX_RIGHT, SS_RIGHT, ES_RIGHT) } else { (0, SS_LEFT, ES_LEFT) };
        // creation order = Tab order
        let mk = |class: &str, t: &str, style: u32, exs: u32, id: i32, x: i32, y: i32, cw: i32, ch: i32| {
            let (c, t) = (wide(class), wide(t));
            let h = CreateWindowExW(exs, c.as_ptr(), t.as_ptr(), WS_CHILD | WS_VISIBLE | style, x, y, cw, ch, hwnd, id as usize as HMENU, app::instance(), null());
            SendMessageW(h, WM_SETFONT, font as usize, 1);
            h
        };
        let mut y = m;
        mk("STATIC", text(l, Msg::ReportDialogText), st_align, rd_ex, ID_LBL1, m, y, inner, sc(20));
        y += sc(24);
        let c = mk("EDIT", "", ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN | WS_VSCROLL | WS_TABSTOP | ed_align, WS_EX_CLIENTEDGE | rd_ex, ID_COMMENT, m, y, inner, sc(70));
        SendMessageW(c, EM_SETLIMITTEXT, r::MAX_COMMENT, 0);
        y += sc(70) + sc(10);
        mk("STATIC", text(l, Msg::ReportPreviewLabel), st_align, rd_ex, ID_LBL2, m, y, inner, sc(20));
        y += sc(24);
        // the preview stays left-to-right (technical text), even in Hebrew
        mk("EDIT", "", ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY | WS_VSCROLL | WS_TABSTOP | ES_LEFT, WS_EX_CLIENTEDGE, ID_PREVIEW, m, y, inner, sc(220));
        y += sc(220) + sc(12);
        // no default button: Enter in the preview must never send
        let (bw, bh) = (sc(100), sc(28));
        let (x_send, x_cancel) = if rtl { (m, m + bw + sc(8)) } else { (w - m - 2 * bw - sc(8), w - m - bw) };
        mk("BUTTON", text(l, Msg::ReportSend), BS_PUSHBUTTON | WS_TABSTOP, 0, ID_SEND, x_send, y, bw, bh);
        mk("BUTTON", text(l, Msg::ReportCancel), BS_PUSHBUTTON | WS_TABSTOP, 0, ID_CANCEL, x_cancel, y, bw, bh);
        y += bh + m;
        let mut rc = RECT { left: 0, top: 0, right: w, bottom: y };
        AdjustWindowRectExForDpi(&mut rc, style, 0, ex, dpi);
        let (ww, wh) = (rc.right - rc.left, rc.bottom - rc.top);
        // centred on the screen of the mouse
        let mut p = POINT { x: 0, y: 0 };
        GetCursorPos(&mut p);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST), &mut mi);
        let wa = mi.rcWork;
        SetWindowPos(hwnd, null_mut(), wa.left + (wa.right - wa.left - ww) / 2, wa.top + (wa.bottom - wa.top - wh) / 2, ww, wh, SWP_NOZORDER);
        refresh_preview();
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(c);
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let (id, code) = ((wparam & 0xFFFF) as i32, ((wparam >> 16) & 0xFFFF) as u32);
            match (id, code) {
                (ID_COMMENT, EN_CHANGE) => refresh_preview(),
                (ID_SEND, _) => {
                    let body = SHOWN.with(|s| std::mem::take(&mut *s.borrow_mut()));
                    if !body.is_empty() {
                        send(app::main_window(), body, true);
                    }
                    // SAFETY: our window.
                    unsafe { DestroyWindow(hwnd) };
                }
                // Esc (IDCANCEL from IsDialogMessage) or Cancel
                (ID_CANCEL, _) | (2, _) => {
                    // SAFETY: our window.
                    unsafe { DestroyWindow(hwnd) };
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            // SAFETY: our window.
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            WIN.with(|w| w.set(null_mut()));
            SHOWN.with(|s| s.borrow_mut().clear());
            let f = FONT.with(|f| f.replace(null_mut()));
            if !f.is_null() {
                // SAFETY: font created by open().
                unsafe { DeleteObject(f) };
            }
            0
        }
        // SAFETY: default handling.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
