//! First-run welcome window (1.6.0, public per-user install only): F10, F9, where the icon is,
//! "Try it now" opens the settings window on the workshop (a box to try the conversion) and
//! "Do not show again" (ticked by default). Never on a PC managed by an administrator (school).

use crate::app;
use crate::util::wide;
use klf_core::i18n::{text, text_with_key, Msg};
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Controls::*;

const ID_TRY: i32 = 100;
const ID_CLOSE: i32 = 101;
/// never two welcome boxes (the message could be posted twice)
static OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn maybe_show(owner: HWND) {
    let s = app::settings();
    if s.welcome_done || !crate::update::allowed(true) {
        return;
    }
    let l = app::lang();
    let title = wide(text(l, Msg::WindowTitle));
    let main = wide(text(l, Msg::WelcomeTitle));
    let body = wide(&text_with_key(l, Msg::WelcomeText, &s.hotkey_description()));
    let (t_try, t_close, t_no) = (wide(text(l, Msg::WelcomeTry)), wide(text(l, Msg::WelcomeClose)), wide(text(l, Msg::WelcomeNoMore)));
    let buttons = [TASKDIALOG_BUTTON { nButtonID: ID_TRY, pszButtonText: t_try.as_ptr() }, TASKDIALOG_BUTTON { nButtonID: ID_CLOSE, pszButtonText: t_close.as_ptr() }];
    // no owner window: our hidden window may close (update, exit) while the box is open, and the
    // box is centred on the screen with its own taskbar button
    let _ = owner;
    if OPEN.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let mut flags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_VERIFICATION_FLAG_CHECKED;
    if l.is_rtl() {
        flags |= TDF_RTL_LAYOUT;
    }
    let icon = app::app_icon(32);
    if !icon.is_null() {
        flags |= TDF_USE_HICON_MAIN;
    }
    let mut cfg: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
    cfg.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
    cfg.hwndParent = std::ptr::null_mut();
    cfg.dwFlags = flags;
    cfg.pszWindowTitle = title.as_ptr();
    cfg.Anonymous1.hMainIcon = icon;
    cfg.pszMainInstruction = main.as_ptr();
    cfg.pszContent = body.as_ptr();
    cfg.cButtons = buttons.len() as u32;
    cfg.pButtons = buttons.as_ptr();
    cfg.nDefaultButton = ID_TRY;
    cfg.pszVerificationText = t_no.as_ptr();
    let (mut pressed, mut checked) = (0i32, 0i32);
    // SAFETY: every pointer in cfg lives until TaskDialogIndirect returns.
    let hr = unsafe { TaskDialogIndirect(&cfg, &mut pressed, null_mut(), &mut checked) };
    OPEN.store(false, std::sync::atomic::Ordering::SeqCst);
    if hr < 0 {
        return; // shown again at the next start
    }
    if checked != 0 {
        let mut u = app::user_settings();
        u.welcome_done = true;
        if app::save_settings(&u) {
            app::apply_settings(u);
        }
    }
    if pressed == ID_TRY {
        crate::settings_ui::open();
    }
}
