//! "HE / EN" sign when the keyboard language changes, and Caps Lock after a press (1.5.0, OFF by
//! default, can be switched off by the administrator). Only while switched on: a slow timer (500 ms,
//! coalescable), plus Windows' language and foreground notices; no extra key is read (the
//! Caps Lock count comes from the hook that already exists for F9). In doubt, nothing is shown.

use crate::app;
use crate::platform::{input, target};
use klf_core::indicator::{tag, Sign, State};
use std::cell::{Cell, RefCell};
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const TIMER_TICK: usize = 5;
pub const TIMER_SOON: usize = 6;
pub const TIMER_CAPS: usize = 7;

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static ON: Cell<bool> = const { Cell::new(false) };
    static SHELL_MSG: Cell<u32> = const { Cell::new(0) };
}

fn now_ms() -> u64 {
    // SAFETY: no arguments.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}

fn main_window() -> HWND {
    app::STATE.with(|s| s.hwnd.get())
}

/// Starts or stops the indicator from the settings in force (user + administrator).
pub fn configure() {
    let want = app::settings().indicator;
    let hwnd = main_window();
    if hwnd.is_null() || want == ON.with(|o| o.get()) {
        return;
    }
    // SAFETY: our window, on its own thread.
    unsafe {
        if want {
            SetCoalescableTimer(hwnd, TIMER_TICK, 500, None, 200);
            if SHELL_MSG.with(|m| m.get()) == 0 {
                let name = crate::util::wide("SHELLHOOK");
                SHELL_MSG.with(|m| m.set(RegisterWindowMessageW(name.as_ptr())));
            }
            RegisterShellHookWindow(hwnd);
            STATE.with(|s| *s.borrow_mut() = State::default());
            crate::util::log("Indicator on");
        } else {
            KillTimer(hwnd, TIMER_TICK);
            KillTimer(hwnd, TIMER_SOON);
            KillTimer(hwnd, TIMER_CAPS);
            DeregisterShellHookWindow(hwnd);
            crate::util::log("Indicator off");
        }
    }
    ON.with(|o| o.set(want));
}

/// A reading a moment later (the layout of the new window is not always set at once).
pub fn soon() {
    if ON.with(|o| o.get()) {
        // SAFETY: our window.
        unsafe { SetTimer(main_window(), TIMER_SOON, 60, None) };
    }
}

/// After a conversion: its bubble has priority (and the language it may have switched to).
pub fn after_conversion() {
    STATE.with(|s| s.borrow_mut().quiet_for(now_ms(), 2500));
}

/// The SHELLHOOK message (HSHELL_LANGUAGE: a window changed its keyboard language).
pub fn shell_message(msg: u32, wparam: usize) -> bool {
    let m = SHELL_MSG.with(|m| m.get());
    if m == 0 || msg != m {
        return false;
    }
    if (wparam as u32 & 0x7FFF) == HSHELL_LANGUAGE {
        soon();
    }
    true
}

/// The window whose keyboard counts: for a Store app, its CoreWindow.
fn keyboard_window(fg: HWND) -> HWND {
    if crate::platform::sys::class_name(fg) == "ApplicationFrameWindow" {
        let class = crate::util::wide("Windows.UI.Core.CoreWindow");
        // SAFETY: valid class name.
        let core = unsafe { FindWindowExW(fg, null_mut(), class.as_ptr(), std::ptr::null()) };
        if !core.is_null() {
            return core;
        }
    }
    fg
}

fn our_window(hwnd: HWND) -> bool {
    let mut pid = 0u32;
    // SAFETY: pid written.
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    // SAFETY: no arguments.
    pid == unsafe { windows_sys::Win32::System::Threading::GetCurrentProcessId() }
}

pub fn on_timer(id: usize) -> bool {
    let hwnd = main_window();
    match id {
        TIMER_TICK => check(),
        TIMER_SOON => {
            // SAFETY: our window.
            unsafe { KillTimer(hwnd, TIMER_SOON) };
            check();
        }
        TIMER_CAPS => {
            // SAFETY: our window.
            unsafe { KillTimer(hwnd, TIMER_CAPS) };
            let fg = target::foreground();
            let on = if fg.is_null() { None } else { input::caps_lock_on_now(fg) };
            if let Some(sign) = STATE.with(|s| s.borrow_mut().caps_read(on, now_ms())) {
                show(sign, fg);
            }
        }
        _ => return false,
    }
    true
}

fn check() {
    if !ON.with(|o| o.get()) {
        return;
    }
    let fg = target::foreground();
    if fg.is_null() || our_window(fg) {
        return;
    }
    let layout = target::current_layout(keyboard_window(fg)).map(|h| h as u32);
    let (sign, caps_pressed) = STATE.with(|s| s.borrow_mut().reading(layout, input::caps_presses(), now_ms()));
    if caps_pressed {
        // the state is read a moment after the press
        // SAFETY: our window.
        unsafe { SetTimer(main_window(), TIMER_CAPS, 60, None) };
    }
    if let Some(sign) = sign {
        show(sign, fg);
    }
}

/// Full screen, presentation, quiet time, locked session...: nothing (only "accepts
/// notifications", and Store apps in a window).
fn notifications_allowed() -> bool {
    use windows_sys::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_ACCEPTS_NOTIFICATIONS, QUNS_APP};
    let mut st = 0;
    // SAFETY: output written.
    let hr = unsafe { SHQueryUserNotificationState(&mut st) };
    hr >= 0 && (st == QUNS_ACCEPTS_NOTIFICATIONS || st == QUNS_APP)
}

fn iso_tag(hkl: u32) -> Option<String> {
    let mut buf = [0u16; 85];
    // SAFETY: buffer size passed.
    let n = unsafe { windows_sys::Win32::Globalization::LCIDToLocaleName(hkl & 0xFFFF, buf.as_mut_ptr(), buf.len() as i32, 0) };
    if n <= 1 {
        return None;
    }
    let name = crate::util::from_wide(&buf);
    name.split('-').next().filter(|x| !x.is_empty()).map(|x| x.to_uppercase())
}

fn show(sign: Sign, fg: HWND) {
    if !notifications_allowed() {
        return;
    }
    let l = app::lang();
    let text = match sign {
        Sign::Layout(h) => match tag(h).map(String::from).or_else(|| iso_tag(h)) {
            Some(t) => t,
            None => return, // unknown language: nothing rather than something wrong
        },
        Sign::CapsLock(on) => klf_core::i18n::text(l, if on { klf_core::i18n::Msg::IndicatorCapsOn } else { klf_core::i18n::Msg::IndicatorCapsOff }).to_string(),
    };
    let caret = target::caret_point(fg);
    crate::bubble::show_near_caret(&text, l.is_rtl(), caret, fg as usize);
}
