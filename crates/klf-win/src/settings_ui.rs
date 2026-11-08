//! The settings window: native Windows controls (Common Controls v6), system font at the
//! right DPI, mirrored right-to-left in Hebrew (WS_EX_LAYOUTRTL set at creation).
//! Three tabs (1.4.0): Simple (keys, converter, language), Advanced (options of settings.json),
//! Diagnostic (last attempts, copy the information). The controls of every tab are children of
//! the window, shown or hidden with the tab; Ctrl+Tab changes the tab.

use crate::app::{self, HotkeyState};
use crate::platform::sys;
use crate::util::{log, wide};
use klf_core::convert::{convert_with, swap_case};
use klf_core::i18n::{text, text_with_key, Lang, Msg};
use klf_core::layouts::{KeyMap, LatinKind};
use klf_core::settings::{
    hotkey_allowed, Action, KEY_NONE, key_to_vk, vk_to_key, AppSettings, MixedTextMode, UiLanguage, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN,
};
use std::cell::Cell;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::{NMHDR, TCITEMW};
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, SystemParametersInfoForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

// common
const ID_STATUS: i32 = 201;
const ID_PAUSE: i32 = 202;
const ID_TAB: i32 = 240;
const ID_ABOUT: i32 = 213;
const ID_SAVE: i32 = 214;
const ID_CLOSE: i32 = 2; // IDCANCEL: Esc closes the window
// Simple
const ID_LBL_HOTKEY: i32 = 230;
const ID_HOTKEY: i32 = 203;
const ID_LBL_HOTKEY_CASE: i32 = 241;
const ID_HOTKEY_CASE: i32 = 236;
const ID_LBL_WORK: i32 = 242;
const ID_WORK_MODE: i32 = 243;
const ID_COPY_RESULT: i32 = 244;
const ID_LBL_TRY: i32 = 232;
const ID_TRY: i32 = 205;
const ID_LBL_RESULT: i32 = 233;
const ID_RESULT: i32 = 206;
const ID_LBL_LANG: i32 = 234;
const ID_LANG: i32 = 212;
const ID_AUTOSTART: i32 = 211;
// Advanced
const ID_LBL_DIRECTION: i32 = 231;
const ID_DIRECTION: i32 = 204;
const ID_SWITCH: i32 = 207;
const ID_BUBBLE: i32 = 208;
const ID_SOUND: i32 = 209;
const ID_CAREFUL: i32 = 245;
const ID_CAPSOFF: i32 = 246;
const ID_CONSOLE: i32 = 210;
const ID_LASTWORD: i32 = 258;
const ID_LBL_LIMIT: i32 = 247;
const ID_LIMIT: i32 = 248;
const ID_LBL_DELAY: i32 = 249;
const ID_DELAY: i32 = 250;
const ID_LBL_EXCLUDED: i32 = 251;
const ID_EXCLUDED: i32 = 252;
const ID_LAYOUTS: i32 = 253;
// Diagnostic
const ID_DIAG_LAST: i32 = 254;
const ID_DIAG_TEXT: i32 = 255;
const ID_DIAG_COPY: i32 = 256;
const ID_DIAG_REFRESH: i32 = 257;
const ID_INDICATOR: i32 = 259;
const ID_EXPORT: i32 = 260;
const ID_IMPORT: i32 = 261;
const ID_UNDO_IMPORT: i32 = 262;
const ID_BUBBLE_POS: i32 = 263;

const PAGE_SIMPLE: &[i32] = &[ID_LBL_HOTKEY, ID_HOTKEY, ID_LBL_HOTKEY_CASE, ID_HOTKEY_CASE, ID_LBL_WORK, ID_WORK_MODE, ID_COPY_RESULT, ID_LBL_TRY, ID_TRY, ID_LBL_RESULT, ID_RESULT, ID_LBL_LANG, ID_LANG, ID_AUTOSTART, ID_INDICATOR];
const PAGE_ADVANCED: &[i32] = &[
    ID_LBL_DIRECTION, ID_DIRECTION, ID_SWITCH, ID_BUBBLE, ID_BUBBLE_POS, ID_SOUND, ID_CAREFUL, ID_LASTWORD, ID_CAPSOFF, ID_CONSOLE, ID_LBL_LIMIT, ID_LIMIT, ID_LBL_DELAY, ID_DELAY, ID_LBL_EXCLUDED, ID_EXCLUDED,
];
const PAGE_DIAG: &[i32] = &[ID_DIAG_LAST, ID_DIAG_TEXT, ID_LAYOUTS, ID_DIAG_COPY, ID_DIAG_REFRESH, ID_EXPORT, ID_IMPORT, ID_UNDO_IMPORT];

const HKM_SETHOTKEY: u32 = WM_USER + 1;
const HKM_GETHOTKEY: u32 = WM_USER + 2;
const HKM_SETRULES: u32 = WM_USER + 3;
const HOTKEYF_SHIFT: u32 = 1;
const HOTKEYF_CONTROL: u32 = 2;
const HOTKEYF_ALT: u32 = 4;
const CB_ADDSTRING: u32 = 0x0143;
const CB_GETCURSEL: u32 = 0x0147;
const CB_SETCURSEL: u32 = 0x014E;
const CB_RESETCONTENT: u32 = 0x014B;
const CBS_DROPDOWNLIST: u32 = 0x0003;
const BM_GETCHECK: u32 = 0x00F0;
const BM_SETCHECK: u32 = 0x00F1;
const BS_AUTOCHECKBOX: u32 = 0x0003;
const BS_PUSHBUTTON: u32 = 0x0000;
const BS_DEFPUSHBUTTON: u32 = 0x0001;
const ES_AUTOHSCROLL: u32 = 0x0080;
const ES_AUTOVSCROLL: u32 = 0x0040;
const ES_MULTILINE: u32 = 0x0004;
const ES_NUMBER: u32 = 0x2000;
const ES_READONLY: u32 = 0x0800;
const ES_WANTRETURN: u32 = 0x1000;
const EN_CHANGE: u32 = 0x0300;
const EM_GETSEL: u32 = 0x00B0;
const EM_SETSEL: u32 = 0x00B1;
const EM_REPLACESEL: u32 = 0x00C2;
const CBN_SELCHANGE: u32 = 1;
const SS_LEFT: u32 = 0;
const SS_CENTERIMAGE: u32 = 0x0200;
const SS_ENDELLIPSIS: u32 = 0x4000;
const TCM_INSERTITEMW: u32 = 0x1300 + 62;
const TCM_GETCURSEL: u32 = 0x1300 + 11;
const TCM_SETCURSEL: u32 = 0x1300 + 12;
const TCIF_TEXT: u32 = 1;
const TCN_SELCHANGE: u32 = (-551i32) as u32;

/// Client size in 96-DPI units. When the screen is too small (200 % on 768 px), up to SHRINK_MAX
/// units are taken from the elastic parts only: text boxes, report, spacing of the check boxes
///.
const WIDTH: i32 = 480;
const FULL_HEIGHT: i32 = 362;
const SHRINK_MAX: i32 = 28;

fn height() -> i32 {
    FULL_HEIGHT - SHRINK.with(|s| s.get())
}

thread_local! {
    static HWND_: Cell<HWND> = const { Cell::new(null_mut()) };
    static DPI: Cell<u32> = const { Cell::new(96) };
    static FONT: Cell<HFONT> = const { Cell::new(null_mut()) };
    static BRUSH: Cell<HBRUSH> = const { Cell::new(null_mut()) };
    static MANAGED: Cell<bool> = const { Cell::new(false) };
    static PAGE: Cell<usize> = const { Cell::new(0) };
    static SHRINK: Cell<i32> = const { Cell::new(0) };
    static PENDING_REFRESH: Cell<bool> = const { Cell::new(false) };
}

pub fn window() -> HWND {
    HWND_.with(|c| c.get())
}

fn item(id: i32) -> HWND {
    // SAFETY: our window (or null -> null result).
    unsafe { GetDlgItem(window(), id) }
}

fn set_text(id: i32, s: &str) {
    let w = wide(s);
    // SAFETY: valid string.
    unsafe { SetWindowTextW(item(id), w.as_ptr()) };
}

fn get_text(id: i32) -> String {
    let h = item(id);
    // SAFETY: buffer size passed.
    unsafe {
        let n = GetWindowTextLengthW(h);
        let mut buf = vec![0u16; (n + 1).max(1) as usize];
        let got = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32);
        crate::util::prefix(&buf, got.max(0) as usize)
    }
}

fn send(id: i32, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: message to our own control.
    unsafe { SendMessageW(item(id), msg, w, l) }
}

fn checked(id: i32) -> bool {
    send(id, BM_GETCHECK, 0, 0) == 1
}

fn set_checked(id: i32, on: bool) {
    send(id, BM_SETCHECK, on as usize, 0);
}

fn scale(v: i32) -> i32 {
    v * DPI.with(|d| d.get()) as i32 / 96
}

fn make_fonts(dpi: u32) {
    // SAFETY: NONCLIENTMETRICSW written by Windows; fonts deleted when replaced.
    unsafe {
        let mut ncm: NONCLIENTMETRICSW = std::mem::zeroed();
        ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
        SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS, ncm.cbSize, &mut ncm as *mut _ as *mut _, 0, dpi);
        let lf = ncm.lfMessageFont;
        let normal = CreateFontIndirectW(&lf);
        let old = FONT.with(|c| c.replace(normal));
        if !old.is_null() {
            DeleteObject(old);
        }
    }
}

fn create(class: &str, txt: &str, style: u32, ex: u32, id: i32) {
    let c = wide(class);
    let t = wide(txt);
    // SAFETY: child of our window; strings outlive the call.
    unsafe {
        CreateWindowExW(ex, c.as_ptr(), t.as_ptr(), WS_CHILD | WS_VISIBLE | style, 0, 0, 10, 10, window(), id as usize as HMENU, app::instance(), null());
    }
}

/// (id, x, y, width, height) in 96-DPI units.
fn layout() -> Vec<(i32, i32, i32, i32, i32)> {
    let k = SHRINK.with(|s| s.get());
    #[allow(non_snake_case)]
    let HEIGHT = height();
    let m = 16;
    let x0 = m + 10; // inside the tab
    let lw = 158; // label column
    let cx = x0 + lw;
    let right = WIDTH - m - 10;
    let cw = right - cx;
    let mut v = vec![
        (ID_STATUS, m, 10, WIDTH - 2 * m - 120, 26),
        (ID_PAUSE, WIDTH - m - 110, 10, 110, 26),
        (ID_TAB, m, 42, WIDTH - 2 * m, HEIGHT - 42 - 46),
        (ID_ABOUT, m, HEIGHT - 38, 110, 28),
        (ID_SAVE, WIDTH - m - 230, HEIGHT - 38, 110, 28),
        (ID_CLOSE, WIDTH - m - 110, HEIGHT - 38, 110, 28),
    ];
    // Simple
    let mut y = 74;
    v.push((ID_LBL_HOTKEY, x0, y + 2, lw - 6, 22));
    v.push((ID_HOTKEY, cx, y, cw, 24));
    y += 28;
    v.push((ID_LBL_HOTKEY_CASE, x0, y + 2, lw - 6, 22));
    v.push((ID_HOTKEY_CASE, cx, y, cw, 24));
    y += 32;
    v.push((ID_LBL_WORK, x0, y + 2, 70, 22));
    v.push((ID_WORK_MODE, x0 + 72, y, right - x0 - 72 - 138, 200));
    v.push((ID_COPY_RESULT, right - 132, y, 132, 24));
    y += 28;
    let half = (right - x0 - 8) / 2;
    v.push((ID_LBL_TRY, x0, y, half, 18));
    v.push((ID_LBL_RESULT, x0 + half + 8, y, half, 18));
    y += 18;
    let tk = k * 3 / 4; // the text boxes give most of the room on a small screen
    v.push((ID_TRY, x0, y, half, 48 - tk));
    v.push((ID_RESULT, x0 + half + 8, y, half, 48 - tk));
    y += 52 - tk;
    v.push((ID_LBL_LANG, x0, y + 2, lw - 6, 22));
    v.push((ID_LANG, cx, y, cw, 200));
    y += 26 - (k > 0) as i32 * 2;
    let ch = 20 - (k > 0) as i32 * 2;
    v.push((ID_AUTOSTART, x0, y, right - x0, ch));
    y += ch;
    v.push((ID_INDICATOR, x0, y, right - x0, ch));
    // Advanced
    let mut y = 74;
    v.push((ID_LBL_DIRECTION, x0, y + 2, lw - 6, 22));
    v.push((ID_DIRECTION, cx, y, cw, 200));
    y += 28;
    let step = 20 - (k + 6) / 7; // 20 .. 16
    for id in [ID_SWITCH, ID_BUBBLE, ID_SOUND, ID_CAREFUL, ID_LASTWORD, ID_CAPSOFF, ID_CONSOLE] {
        if id == ID_BUBBLE {
            // bubble: check box + position list on the same line (1.5.2), a little more room
            v.push((id, x0, y + 1, lw + 56, step));
            v.push((ID_BUBBLE_POS, cx + 62, y - 1, cw - 62, 200));
            y += step + 4;
            continue;
        }
        v.push((id, x0, y, right - x0, step));
        y += step;
    }
    y += 4 - (k > 0) as i32 * 4;
    v.push((ID_LBL_LIMIT, x0, y + 2, 170, 22));
    v.push((ID_LIMIT, x0 + 172, y, 56, 22));
    v.push((ID_LBL_DELAY, x0 + 236, y + 2, 128, 22));
    v.push((ID_DELAY, right - 58, y, 58, 22));
    y += 26;
    v.push((ID_LBL_EXCLUDED, x0, y + 2, lw - 6, 22));
    v.push((ID_EXCLUDED, cx, y, cw, 22));
    // Diagnostic
    let y = 74;
    v.push((ID_DIAG_LAST, x0, y, right - x0, 56));
    v.push((ID_DIAG_TEXT, x0, y + 60, right - x0, 84 - k));
    v.push((ID_LAYOUTS, x0, y + 148 - k, right - x0, 20)); // installed keyboards, ✓ / ✗
    v.push((ID_DIAG_COPY, x0, y + 172 - k, 180, 26));
    v.push((ID_DIAG_REFRESH, x0 + 188, y + 172 - k, 110, 26));
    // export / import of the settings (1.5.0)
    let bw = (right - x0 - 12) / 3;
    v.push((ID_EXPORT, x0, y + 202 - k, bw, 26));
    v.push((ID_IMPORT, x0 + bw + 6, y + 202 - k, bw, 26));
    v.push((ID_UNDO_IMPORT, x0 + 2 * (bw + 6), y + 202 - k, bw, 26));
    v
}

/// Shows an error in a small Windows message box: the settings window never grows (audit 1.3.5,
/// N5). Empty = nothing to show.
fn set_error(msg: &str) {
    message(msg, MB_ICONWARNING);
}

fn message(msg: &str, icon: MESSAGEBOX_STYLE) {
    if msg.is_empty() {
        return;
    }
    let l = app::lang();
    let title = wide(text(l, Msg::WindowTitle));
    let body = wide(msg);
    let mut flags = MB_OK | icon;
    if l.is_rtl() {
        flags |= MB_RTLREADING | MB_RIGHT;
    }
    // SAFETY: valid strings; our window as owner (modal, comes back here).
    unsafe { MessageBoxW(window(), body.as_ptr(), title.as_ptr(), flags) };
}

fn apply_layout() {
    let font = FONT.with(|f| f.get());
    for (id, x, y, w, h) in layout() {
        let hw = item(id);
        // SAFETY: our child windows.
        unsafe {
            MoveWindow(hw, scale(x), scale(y), scale(w), scale(h), 1);
            SendMessageW(hw, WM_SETFONT, font as usize, 1);
        }
    }
}

/// Shows the controls of one tab, hides the others.
fn show_page(page: usize) {
    PAGE.with(|p| p.set(page));
    send(ID_TAB, TCM_SETCURSEL, page, 0);
    for (i, ids) in [PAGE_SIMPLE, PAGE_ADVANCED, PAGE_DIAG].iter().enumerate() {
        for id in *ids {
            // SAFETY: our child windows.
            unsafe { ShowWindow(item(*id), if i == page { SW_SHOW } else { SW_HIDE }) };
        }
    }
    if page == 2 {
        refresh_diagnostic();
    }
}

/// Ctrl+Tab / Ctrl+Shift+Tab in the window: next / previous tab (called by the message loop).
pub fn handle_ctrl_tab(msg: &MSG) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_SHIFT, VK_TAB};
    let w = window();
    // SAFETY: GetKeyState / IsChild have no pointers.
    unsafe {
        if w.is_null() || msg.message != WM_KEYDOWN || msg.wParam != VK_TAB as usize || GetKeyState(VK_CONTROL as i32) >= 0 {
            return false;
        }
        if msg.hwnd != w && IsChild(w, msg.hwnd) == 0 {
            return false;
        }
        let back = GetKeyState(VK_SHIFT as i32) < 0;
        let p = PAGE.with(|p| p.get());
        show_page(if back { (p + 2) % 3 } else { (p + 1) % 3 });
        SetFocus(item(ID_TAB));
    }
    true
}

fn mods_to_hotkeyf(m: u32) -> u32 {
    (if m & MOD_SHIFT != 0 { HOTKEYF_SHIFT } else { 0 })
        | (if m & MOD_CONTROL != 0 { HOTKEYF_CONTROL } else { 0 })
        | (if m & MOD_ALT != 0 { HOTKEYF_ALT } else { 0 })
}

fn hotkeyf_to_mods(f: u32) -> u32 {
    (if f & HOTKEYF_SHIFT != 0 { MOD_SHIFT } else { 0 })
        | (if f & HOTKEYF_CONTROL != 0 { MOD_CONTROL } else { 0 })
        | (if f & HOTKEYF_ALT != 0 { MOD_ALT } else { 0 })
}

fn direction() -> MixedTextMode {
    match send(ID_DIRECTION, CB_GETCURSEL, 0, 0) {
        1 => MixedTextMode::EnglishToHebrewOnly,
        2 => MixedTextMode::HebrewToEnglishOnly,
        _ => MixedTextMode::ToggleAll,
    }
}

/// Key map of the converter: the first supported Latin layout installed (US table otherwise).
fn workshop_map() -> KeyMap {
    match crate::platform::layouts::choose(None) {
        crate::platform::layouts::Choice::Map(m, _) => m,
        crate::platform::layouts::Choice::Unsupported(_) => klf_core::convert::us_map(),
    }
}

fn latin_name(l: Lang, k: LatinKind) -> &'static str {
    match (l, k) {
        (Lang::He, LatinKind::Us) => "אנגלית",
        (Lang::He, LatinKind::FrenchAzerty) => "צרפתית (AZERTY)",
        (Lang::Fr, LatinKind::Us) => "anglais",
        (Lang::Fr, LatinKind::FrenchAzerty) => "français (AZERTY)",
        (Lang::En, LatinKind::Us) => "English",
        (Lang::En, LatinKind::FrenchAzerty) => "French (AZERTY)",
    }
}

/// The converter (Simple tab): nothing is sent to another program, nothing is kept.
fn workshop(input: &str, case: bool) -> String {
    if case {
        return swap_case(input);
    }
    let s = app::settings();
    let mode = match send(ID_WORK_MODE, CB_GETCURSEL, 0, 0) {
        1 => MixedTextMode::HebrewToEnglishOnly,
        2 => MixedTextMode::EnglishToHebrewOnly,
        3 => return swap_case(input),
        _ => direction(),
    };
    convert_with(&workshop_map(), input, mode, s.replace_caps)
}

fn update_try() {
    let t = get_text(ID_TRY);
    set_text(ID_RESULT, &workshop(&t, false));
}

/// F10/F9 pressed while the converter text has the focus: the selection (or the whole text) is
/// converted IN the field, without the clipboard (our window would be both the target and the
/// clipboard owner). True if done.
pub fn convert_in_try_field(action: Action) -> bool {
    let w = window();
    // SAFETY: GetFocus / GetForegroundWindow for our own thread and window.
    let ours = unsafe { !w.is_null() && GetForegroundWindow() == w && GetFocus() == item(ID_TRY) };
    if !ours {
        return false;
    }
    let mut start: u32 = 0;
    let mut end: u32 = 0;
    send(ID_TRY, EM_GETSEL, &mut start as *mut u32 as usize, &mut end as *mut u32 as isize);
    let all: Vec<u16> = get_text(ID_TRY).encode_utf16().collect();
    let (a, b) = if start < end && (end as usize) <= all.len() { (start as usize, end as usize) } else { (0, all.len()) };
    let part = String::from_utf16_lossy(&all[a..b]);
    let s = app::settings();
    let out = match action {
        Action::Case => swap_case(&part),
        Action::Layout => convert_with(&workshop_map(), &part, s.mixed_text_mode, s.replace_caps),
    };
    send(ID_TRY, EM_SETSEL, a, b as isize);
    let wout = wide(&out);
    send(ID_TRY, EM_REPLACESEL, 1, wout.as_ptr() as isize);
    let n = out.encode_utf16().count();
    send(ID_TRY, EM_SETSEL, a, (a + n) as isize);
    true
}

fn refresh_diagnostic() {
    let l = app::lang();
    let last = crate::diagnostic::last();
    let line = match last {
        Some((when, msg, explanation)) => {
            let bubble = msg.map(|m| text_with_key(l, m, &app::settings().hotkey_description())).unwrap_or_default();
            format!("{} {}\n{}", text(l, Msg::DiagLast).replace("{time}", &when), bubble, explanation)
        }
        None => text(l, Msg::DiagNone).to_string(),
    };
    set_text(ID_DIAG_LAST, &line);
    set_text(ID_DIAG_TEXT, &crate::diagnostic::report().replace('\n', "\r\n"));
}

pub fn refresh_status() {
    if window().is_null() {
        return;
    }
    let s = app::settings();
    let l = app::lang_of(&s);
    let key = s.hotkey_description();
    let (msg, button) = match app::hotkey_state() {
        HotkeyState::Ready => (Msg::StatusReady, Msg::ButtonPause),
        HotkeyState::Paused => (Msg::StatusPaused, Msg::ButtonResume),
        HotkeyState::Unavailable => (Msg::StatusUnavailable, Msg::ButtonPause),
    };
    let mut status = format!("●  {}", text_with_key(l, msg, &key));
    // the Caps Lock hotkey taken by another program: said here too (audit 1.3.5, N4)
    if app::hotkey_state() != HotkeyState::Paused && !app::case_hotkey_ok() {
        status.push_str(" · ");
        status.push_str(&text_with_key(l, Msg::CaseKeyBusy, &s.case_hotkey_description()));
    }
    if app::locked().any() {
        status.push_str(" · ");
        status.push_str(text(l, Msg::SchoolManaged));
    }
    set_text(ID_STATUS, &status);
    set_text(ID_PAUSE, text(l, button));
    set_text(ID_LBL_EXCLUDED, &text(l, Msg::LabelExcluded).replace("{n}", &s.app_rules.len().to_string()));
    // SAFETY: repaint our control.
    unsafe { InvalidateRect(item(ID_STATUS), null(), 1) };
}

fn fill_work_modes(l: Lang) {
    let latin = latin_name(l, workshop_map().latin);
    send(ID_WORK_MODE, CB_RESETCONTENT, 0, 0);
    for t in [
        text(l, Msg::WorkFollow).to_string(),
        text(l, Msg::WorkToLatin).replace("{latin}", latin),
        text(l, Msg::WorkToHebrew).replace("{latin}", latin),
        text(l, Msg::WorkCase).to_string(),
    ] {
        let w = wide(&t);
        send(ID_WORK_MODE, CB_ADDSTRING, 0, w.as_ptr() as isize);
    }
    send(ID_WORK_MODE, CB_SETCURSEL, 0, 0);
}

fn layouts_line(l: Lang) -> String {
    let list: Vec<String> = crate::platform::layouts::installed()
        .iter()
        .map(|h| {
            let ok = !matches!(klf_core::layouts::classify_hkl(*h), klf_core::layouts::LayoutKind::Unsupported);
            format!("{}{}", klf_core::layouts::hkl_name(*h), if ok { " ✓" } else { " ✗" })
        })
        .collect();
    text(l, Msg::LayoutsLine).replace("{list}", &list.join(", "))
}

fn fill_controls() {
    let s = app::settings();
    let l = app::lang_of(&s);
    for (i, m) in [Msg::TabSimple, Msg::TabAdvanced, Msg::TabDiagnostic].iter().enumerate() {
        let mut t = wide(text(l, *m));
        // SAFETY: TCITEMW read during the call; text buffer alive.
        let mut it: TCITEMW = unsafe { std::mem::zeroed() };
        it.mask = TCIF_TEXT;
        it.pszText = t.as_mut_ptr();
        send(ID_TAB, TCM_INSERTITEMW, i, &it as *const TCITEMW as isize);
    }
    set_text(ID_LBL_HOTKEY, text(l, Msg::LabelHotkeyLayout));
    set_text(ID_LBL_HOTKEY_CASE, text(l, Msg::LabelHotkeyCase));
    set_text(ID_LBL_WORK, text(l, Msg::LabelWorkshop));
    set_text(ID_COPY_RESULT, text(l, Msg::ButtonCopyResult));
    set_text(ID_LBL_TRY, text(l, Msg::LabelText));
    set_text(ID_LBL_RESULT, text(l, Msg::LabelResult));
    set_text(ID_LBL_DIRECTION, text(l, Msg::LabelDirection));
    set_text(ID_LBL_LANG, text(l, Msg::LabelLanguage));
    set_text(ID_SWITCH, text(l, Msg::CheckSwitchLanguage));
    set_text(ID_BUBBLE, text(l, Msg::CheckBubble));
    set_text(ID_SOUND, text(l, Msg::CheckSound));
    set_text(ID_CAREFUL, text(l, Msg::CheckCareful));
    set_text(ID_LASTWORD, text(l, Msg::CheckLastWord));
    set_text(ID_CAPSOFF, text(l, Msg::CheckCapsOff));
    set_text(ID_CONSOLE, text(l, Msg::CheckConsole));
    set_text(ID_LBL_LIMIT, text(l, Msg::LabelLimit));
    set_text(ID_LBL_DELAY, text(l, Msg::LabelDelay));
    set_text(ID_DIAG_COPY, text(l, Msg::ButtonCopyDiag));
    set_text(ID_DIAG_REFRESH, text(l, Msg::ButtonRefresh));
    set_text(ID_INDICATOR, text(l, Msg::CheckIndicator));
    set_text(ID_EXPORT, text(l, Msg::ButtonExport));
    set_text(ID_IMPORT, text(l, Msg::ButtonImport));
    set_text(ID_UNDO_IMPORT, text(l, Msg::ButtonUndoImport));
    set_text(ID_ABOUT, text(l, Msg::ButtonAbout));
    set_text(ID_SAVE, text(l, Msg::ButtonSave));
    set_text(ID_CLOSE, text(l, Msg::ButtonClose));
    set_text(ID_LAYOUTS, &layouts_line(l));
    fill_work_modes(l);
    for m in [Msg::DirectionAuto, Msg::DirectionToHebrew, Msg::DirectionToEnglish] {
        let w = wide(text(l, m));
        send(ID_DIRECTION, CB_ADDSTRING, 0, w.as_ptr() as isize);
    }
    send(ID_DIRECTION, CB_SETCURSEL, s.mixed_text_mode.as_number() as usize, 0);
    for m in [Msg::BubbleTop, Msg::BubbleBottom, Msg::BubbleCaret] {
        let w = wide(text(l, m));
        send(ID_BUBBLE_POS, CB_ADDSTRING, 0, w.as_ptr() as isize);
    }
    send(ID_BUBBLE_POS, CB_SETCURSEL, s.bubble_position.min(klf_core::settings::BUBBLE_CARET) as usize, 0);
    for t in [text(l, Msg::LanguageAuto), "עברית", "Français", "English"] {
        let w = wide(t);
        send(ID_LANG, CB_ADDSTRING, 0, w.as_ptr() as isize);
    }
    let li = match s.ui_language {
        UiLanguage::Auto => 0,
        UiLanguage::He => 1,
        UiLanguage::Fr => 2,
        UiLanguage::En => 3,
    };
    send(ID_LANG, CB_SETCURSEL, li, 0);
    send(ID_HOTKEY, HKM_SETRULES, 0, 0); // F1-F11 alone allowed (F12 refused by validate)
    send(ID_HOTKEY_CASE, HKM_SETRULES, 0, 0);
    set_box(ID_HOTKEY, s.hotkey_modifiers, s.hotkey_key);
    set_box(ID_HOTKEY_CASE, s.case_hotkey_modifiers, s.case_hotkey_key);
    set_checked(ID_SWITCH, s.switch_language_after_convert);
    set_checked(ID_BUBBLE, s.show_bubble);
    set_checked(ID_SOUND, s.play_sound);
    set_checked(ID_CAREFUL, s.require_selection);
    set_checked(ID_LASTWORD, s.last_word_only);
    set_checked(ID_CAPSOFF, s.turn_off_caps_lock);
    set_checked(ID_CONSOLE, s.console_support);
    set_checked(ID_INDICATOR, s.indicator);
    set_text(ID_LIMIT, &s.max_character_limit.to_string());
    set_text(ID_DELAY, &s.paste_delay_ms.to_string());
    // only the user's own list (the school's programs are added on top, never saved here)
    set_text(ID_EXCLUDED, &app::user_settings().excluded_processes.join(", "));
    // settings imposed by the school: shown, greyed out
    let lk = app::locked();
    for (id, locked) in [
        (ID_HOTKEY, lk.hotkey),
        (ID_HOTKEY_CASE, lk.case_hotkey),
        (ID_DIRECTION, lk.mixed_text_mode),
        (ID_SWITCH, lk.switch_language_after_convert),
        (ID_BUBBLE, lk.show_bubble),
        (ID_BUBBLE_POS, lk.bubble_position || !s.show_bubble),
        (ID_SOUND, lk.play_sound),
        (ID_CAREFUL, lk.require_selection),
        (ID_LASTWORD, lk.last_word_only),
        (ID_CAPSOFF, lk.turn_off_caps_lock),
        (ID_CONSOLE, lk.console_support),
        (ID_LIMIT, lk.max_character_limit),
        (ID_DELAY, lk.paste_delay_ms),
        (ID_LANG, lk.ui_language),
        (ID_INDICATOR, lk.indicator),
    ] {
        if locked {
            // SAFETY: our control.
            unsafe { EnableWindow(item(id), 0) };
        }
    }
    let managed = sys::autostart_managed();
    MANAGED.with(|c| c.set(managed));
    if managed {
        set_text(ID_AUTOSTART, text(l, Msg::AutostartManaged));
        // SAFETY: our control.
        unsafe { EnableWindow(item(ID_AUTOSTART), 0) };
    } else {
        set_text(ID_AUTOSTART, text(l, Msg::CheckAutostart));
        set_checked(ID_AUTOSTART, sys::autostart_user());
    }
    refresh_status();
}

/// Reads the controls; None (with a message) if a value is not allowed.
/// Builds the USER settings to save: a setting imposed by the school keeps the user's own value
/// (the school's value is never written into settings.json).
fn read_controls() -> Option<AppSettings> {
    let old = app::user_settings();
    let lk = app::locked();
    let mut s = old.clone();
    let l = app::lang();
    let hk = send(ID_HOTKEY, HKM_GETHOTKEY, 0, 0) as u32;
    let vk = hk & 0xFF;
    let mut mods = hotkeyf_to_mods((hk >> 8) & 0xFF);
    if old.hotkey_modifiers & MOD_WIN != 0 && key_to_vk(old.hotkey_key) == vk && mods == old.hotkey_modifiers & !MOD_WIN {
        mods |= MOD_WIN; // the control cannot show Win: keep it if nothing else changed
    }
    match vk_to_key(vk) {
        _ if lk.hotkey => {}
        Some(k) if hotkey_allowed(mods, k) => {
            s.hotkey_key = k;
            s.hotkey_modifiers = mods;
        }
        _ => {
            set_error(text(l, Msg::HotkeyInvalid));
            return None;
        }
    }
    // Caps Lock box: empty = function switched off
    let hk = send(ID_HOTKEY_CASE, HKM_GETHOTKEY, 0, 0) as u32;
    let (vk, mods) = (hk & 0xFF, hotkeyf_to_mods((hk >> 8) & 0xFF));
    if lk.case_hotkey {
    } else if vk == 0 {
        s.case_hotkey_key = KEY_NONE;
        s.case_hotkey_modifiers = 0;
    } else {
        match vk_to_key(vk) {
            Some(k) if hotkey_allowed(mods, k) && (mods, k) != (s.hotkey_modifiers, s.hotkey_key) => {
                s.case_hotkey_key = k;
                s.case_hotkey_modifiers = mods;
            }
            _ => {
                set_error(text(l, Msg::HotkeyInvalid));
                return None;
            }
        }
    }
    let limit: Option<i64> = if lk.max_character_limit { Some(old.max_character_limit) } else { get_text(ID_LIMIT).trim().parse().ok().filter(|n| *n >= 0) };
    let delay: Option<i64> = if lk.paste_delay_ms { Some(old.paste_delay_ms) } else { get_text(ID_DELAY).trim().parse().ok().filter(|n| (100..=10_000).contains(n)) };
    let (Some(limit), Some(delay)) = (limit, delay) else {
        set_error(text(l, Msg::NumberInvalid));
        return None;
    };
    s.max_character_limit = limit;
    s.paste_delay_ms = delay;
    s.excluded_processes = get_text(ID_EXCLUDED).split([',', ';', '\n']).map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
    let keep = |locked: bool, mine: bool, id: i32| if locked { mine } else { checked(id) };
    if !lk.mixed_text_mode {
        s.mixed_text_mode = direction();
    }
    s.switch_language_after_convert = keep(lk.switch_language_after_convert, old.switch_language_after_convert, ID_SWITCH);
    s.show_bubble = keep(lk.show_bubble, old.show_bubble, ID_BUBBLE);
    if !lk.bubble_position {
        let i = send(ID_BUBBLE_POS, CB_GETCURSEL, 0, 0);
        if (0..=klf_core::settings::BUBBLE_CARET as isize).contains(&i) {
            s.bubble_position = i as u32;
        }
    }
    s.play_sound = keep(lk.play_sound, old.play_sound, ID_SOUND);
    s.require_selection = keep(lk.require_selection, old.require_selection, ID_CAREFUL);
    s.last_word_only = keep(lk.last_word_only, old.last_word_only, ID_LASTWORD);
    s.turn_off_caps_lock = keep(lk.turn_off_caps_lock, old.turn_off_caps_lock, ID_CAPSOFF);
    s.console_support = keep(lk.console_support, old.console_support, ID_CONSOLE);
    s.indicator = keep(lk.indicator, old.indicator, ID_INDICATOR);
    if !lk.ui_language {
        s.ui_language = match send(ID_LANG, CB_GETCURSEL, 0, 0) {
            1 => UiLanguage::He,
            2 => UiLanguage::Fr,
            3 => UiLanguage::En,
            _ => UiLanguage::Auto,
        };
    }
    s.validate();
    Some(s)
}

fn on_save() {
    let Some(new_user) = read_controls() else { return };
    let old = app::settings();
    // the keys and the language in force after saving (the school's settings stay on top)
    let new = klf_core::school::effective(&new_user, &app::policy()).0;
    // a new hotkey is tried BEFORE the old one is given up (audit 1.3.2)
    // a combination we already hold (F9 and F10 exchanged) is not "busy"
    let ours = |m: u32, k: u32| (m, k) == (old.hotkey_modifiers, old.hotkey_key) || (m, k) == (old.case_hotkey_modifiers, old.case_hotkey_key);
    let key_changed = (new.hotkey_modifiers, new.hotkey_key) != (old.hotkey_modifiers, old.hotkey_key);
    if key_changed && !ours(new.hotkey_modifiers, new.hotkey_key) && !app::hotkey_free(new.hotkey_modifiers, new.hotkey_key) {
        set_error(&text_with_key(app::lang(), Msg::HotkeyBusy, &new.hotkey_description()));
        return;
    }
    let case_changed = (new.case_hotkey_modifiers, new.case_hotkey_key) != (old.case_hotkey_modifiers, old.case_hotkey_key);
    if case_changed && new.case_hotkey_key != KEY_NONE && !ours(new.case_hotkey_modifiers, new.case_hotkey_key) && !app::hotkey_free(new.case_hotkey_modifiers, new.case_hotkey_key) {
        set_error(&text_with_key(app::lang(), Msg::HotkeyBusy, &new.case_hotkey_description()));
        return;
    }
    // settings.json first: if it cannot be written, NOTHING is changed (registry included)
    if !app::save_settings(&new_user) {
        set_error(text(app::lang(), Msg::SaveFailed));
        return;
    }
    let mut autostart_failed = false;
    if !MANAGED.with(|c| c.get()) {
        let want = checked(ID_AUTOSTART);
        if want != sys::autostart_user() && !sys::set_autostart_user(want, &sys::exe_path()) {
            log("Autostart (HKCU Run) could not be changed");
            autostart_failed = true;
        }
    }
    let relang = app::lang_of(&new) != app::lang_of(&old);
    app::apply_settings(new_user);
    log("Settings saved");
    if relang {
        // the layout direction (RTL) is chosen at creation: recreate the window
        let page = PAGE.with(|p| p.get());
        close();
        open();
        show_page(page);
        if autostart_failed {
            // kept across the new window (audit 1.3.2, R7)
            set_error(text(app::lang(), Msg::AutostartFailed));
        }
    } else {
        set_error(if autostart_failed { text(app::lang(), Msg::AutostartFailed) } else { "" });
        refresh_status();
    }
}

/// The school's settings changed while the window is open: shown again, same tab.
pub fn refresh_after_policy() {
    if window().is_null() {
        return;
    }
    // a dialog of ours is open (import, message): never destroy the window under it; done
    // when it closes
    // SAFETY: our window.
    if unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(window()) } == 0 {
        PENDING_REFRESH.with(|p| p.set(true));
        return;
    }
    PENDING_REFRESH.with(|p| p.set(false));
    let page = PAGE.with(|p| p.get());
    close();
    open();
    show_page(page);
}

/// Common file dialog (open or save) for a profile; None if cancelled.
fn file_dialog(save: bool) -> Option<std::path::PathBuf> {
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let filter: Vec<u16> = "Profil (*.json)\0*.json\0Tous les fichiers (*.*)\0*.*\0\0".encode_utf16().collect();
    let ext = wide("json");
    let mut file = vec![0u16; 4096];
    if save {
        for (d, c) in file.iter_mut().zip("keyboard-layout-fixer-profil.json".encode_utf16()) {
            *d = c;
        }
    }
    // SAFETY: zeroed OPENFILENAMEW is valid; buffers alive during the modal call (UI thread).
    unsafe {
        let mut ofn: OPENFILENAMEW = std::mem::zeroed();
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = window();
        ofn.lpstrFilter = filter.as_ptr();
        ofn.nFilterIndex = 1;
        ofn.lpstrFile = file.as_mut_ptr();
        ofn.nMaxFile = file.len() as u32;
        ofn.lpstrDefExt = ext.as_ptr();
        // never change the current folder of the program
        ofn.Flags = OFN_NOCHANGEDIR | OFN_EXPLORER | OFN_HIDEREADONLY | if save { OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST } else { OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST };
        let ok = if save { GetSaveFileNameW(&mut ofn) } else { GetOpenFileNameW(&mut ofn) };
        (ok != 0).then(|| std::path::PathBuf::from(crate::util::from_wide(&file)))
    }
}

fn ask(msg: &str) -> bool {
    let l = app::lang();
    let title = wide(text(l, Msg::WindowTitle));
    let body = wide(msg);
    let mut flags = MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2;
    if l.is_rtl() {
        flags |= MB_RTLREADING | MB_RIGHT;
    }
    // SAFETY: valid strings; our window as owner.
    unsafe { MessageBoxW(window(), body.as_ptr(), title.as_ptr(), flags) == IDYES }
}

fn backup_path() -> Option<std::path::PathBuf> {
    crate::util::settings_path().map(|p| p.with_extension("json.avant-import"))
}

fn on_export() {
    let Some(path) = file_dialog(true) else { return };
    let text_ = klf_core::profile::export(&app::user_settings(), klf_core::VERSION);
    if std::fs::write(&path, text_).is_ok() {
        log("Settings exported");
        message(text(app::lang(), Msg::ExportDone), MB_ICONINFORMATION);
    } else {
        set_error(text(app::lang(), Msg::SaveFailed));
    }
}

fn on_import() {
    let l = app::lang();
    let Some(path) = file_dialog(false) else { return };
    let read = std::fs::metadata(&path)
        .ok()
        .filter(|m| m.len() as usize <= klf_core::profile::MAX_BYTES)
        .and_then(|_| std::fs::read_to_string(&path).ok())
        .ok_or_else(|| "fichier illisible ou trop gros".to_string())
        .and_then(|t| klf_core::profile::import(&t));
    let new = match read {
        Ok(s) => s,
        Err(why) => {
            set_error(&format!("{} {why}", text(l, Msg::ImportFailed)));
            return;
        }
    };
    let old = app::user_settings();
    let diff = klf_core::profile::differences(&old, &new, &app::locked());
    if diff.is_empty() {
        message(text(l, Msg::ImportNothing), MB_ICONINFORMATION);
        return;
    }
    let shown: Vec<String> = diff.iter().take(20).cloned().collect();
    if !ask(&format!("{}

{}", text(l, Msg::ImportConfirm), shown.join("
"))) {
        return;
    }
    // copy of the current settings first: nothing changes if it cannot be made
    let Some(bak) = backup_path() else { return };
    if std::fs::write(&bak, old.to_json()).is_err() || !app::save_settings(&new) {
        set_error(text(l, Msg::SaveFailed));
        return;
    }
    log("Settings imported (previous ones in settings.json.avant-import)");
    app::apply_settings(new);
    refresh_after_policy();
    message(text(app::lang(), Msg::ImportDone), MB_ICONINFORMATION);
}

fn on_undo_import() {
    let l = app::lang();
    let prev = backup_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| AppSettings::from_json(&t));
    let Some((prev, true)) = prev else {
        message(text(l, Msg::UndoNone), MB_ICONINFORMATION);
        return;
    };
    if !ask(text(l, Msg::UndoConfirm)) {
        return;
    }
    if !app::save_settings(&prev) {
        set_error(text(l, Msg::SaveFailed));
        return;
    }
    log("Settings put back from settings.json.avant-import");
    app::apply_settings(prev);
    refresh_after_policy();
}

pub fn close() {
    let h = window();
    if !h.is_null() {
        // SAFETY: our window.
        unsafe { DestroyWindow(h) };
    }
}

fn set_box(id: i32, mods: u32, key: u32) {
    let vk = if key == KEY_NONE { 0 } else { key_to_vk(key) as usize };
    send(id, HKM_SETHOTKEY, vk | ((mods_to_hotkeyf(mods) as usize) << 8), 0);
}

/// One of our hotkeys pressed while a hotkey box has the focus: show it in that box instead of
/// converting (the box never sees a registered hotkey otherwise).
pub fn hotkey_box_has_focus() -> bool {
    let h = window();
    // SAFETY: GetFocus for our own thread.
    let f = unsafe { GetFocus() };
    !h.is_null() && (f == item(ID_HOTKEY) || f == item(ID_HOTKEY_CASE))
}

pub fn put_pressed_hotkey_in_box(action: Action) {
    let s = app::settings();
    let (mods, key) = if action == Action::Case { (s.case_hotkey_modifiers, s.case_hotkey_key) } else { (s.hotkey_modifiers, s.hotkey_key) };
    // SAFETY: GetFocus for our own thread.
    let id = if unsafe { GetFocus() } == item(ID_HOTKEY_CASE) { ID_HOTKEY_CASE } else { ID_HOTKEY };
    set_box(id, mods, key);
}

fn copy_text(t: &str) {
    if t.is_empty() || app::busy() {
        return; // never during a conversion (its clipboard checks would see our copy)
    }
    if !crate::platform::clipboard::put_plain_text(window(), t) {
        set_error(text(app::lang(), Msg::CopyFailed));
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as i32;
            let code = ((wparam >> 16) & 0xFFFF) as u32;
            match (id, code) {
                (ID_PAUSE, _) => app::toggle_pause(),
                (ID_SAVE, _) | (1, _) => on_save(), // 1 = IDOK (Enter)
                (ID_CLOSE, _) => close(),
                (ID_ABOUT, _) => app::show_about(hwnd),
                (ID_TRY, EN_CHANGE) => update_try(),
                (ID_WORK_MODE, CBN_SELCHANGE) | (ID_DIRECTION, CBN_SELCHANGE) => update_try(),
                // the position list only matters when the bubble is shown
                (ID_BUBBLE, BN_CLICKED) if !app::locked().bubble_position => {
                    // SAFETY: our control.
                    unsafe { EnableWindow(item(ID_BUBBLE_POS), checked(ID_BUBBLE) as i32) };
                }
                (ID_COPY_RESULT, _) => copy_text(&get_text(ID_RESULT)),
                (ID_DIAG_REFRESH, _) => refresh_diagnostic(),
                (ID_EXPORT, _) => on_export(),
                (ID_IMPORT, _) => on_import(),
                (ID_UNDO_IMPORT, _) => on_undo_import(),
                (ID_DIAG_COPY, _) if !app::busy() => {
                    refresh_diagnostic();
                    if crate::platform::clipboard::put_plain_text(window(), &crate::diagnostic::report().replace('\n', "\r\n")) {
                        message(text(app::lang(), Msg::DiagCopied), MB_ICONINFORMATION);
                    } else {
                        set_error(text(app::lang(), Msg::CopyFailed));
                    }
                }
                _ => {}
            }
            0
        }
        WM_ENABLE if wparam != 0 && PENDING_REFRESH.with(|p| p.get()) => {
            // the dialog closed: the school's settings changed meanwhile
            // SAFETY: posting to our own window.
            unsafe { PostMessageW(hwnd, WM_APP + 20, 0, 0) };
            0
        }
        m if m == WM_APP + 20 => {
            refresh_after_policy();
            0
        }
        WM_NOTIFY => {
            // SAFETY: lparam points to an NMHDR for WM_NOTIFY (documented).
            let nm = unsafe { &*(lparam as *const NMHDR) };
            if nm.idFrom == ID_TAB as usize && nm.code == TCN_SELCHANGE {
                show_page(send(ID_TAB, TCM_GETCURSEL, 0, 0).max(0) as usize);
            }
            0
        }
        WM_CTLCOLORSTATIC => {
            let dc = wparam as HDC;
            let child = lparam as HWND;
            // SAFETY: colours for our own static controls.
            unsafe {
                SetBkMode(dc, TRANSPARENT as i32);
                if child == item(ID_STATUS) {
                    // high contrast: the system text colour (audit of the 1.4.0 plan)
                    let c = if sys::high_contrast() {
                        GetSysColor(COLOR_WINDOWTEXT)
                    } else {
                        match app::hotkey_state() {
                            HotkeyState::Ready => 0x003E8E1F,       // green (BGR)
                            HotkeyState::Paused => 0x00007ACC,      // orange
                            HotkeyState::Unavailable => 0x002020C0, // red
                        }
                    };
                    SetTextColor(dc, c);
                } else if child == item(ID_RESULT) || child == item(ID_DIAG_TEXT) {
                    SetTextColor(dc, GetSysColor(COLOR_WINDOWTEXT));
                    SetBkMode(dc, OPAQUE as i32);
                    SetBkColor(dc, GetSysColor(COLOR_BTNFACE));
                    return GetSysColorBrush(COLOR_BTNFACE) as LRESULT;
                } else {
                    SetTextColor(dc, GetSysColor(COLOR_WINDOWTEXT));
                }
            }
            BRUSH.with(|b| b.get()) as LRESULT
        }
        WM_DPICHANGED => {
            let dpi = ((wparam >> 16) & 0xFFFF) as u32;
            DPI.with(|d| d.set(dpi));
            make_fonts(dpi);
            // SAFETY: lparam points to the suggested rectangle (documented).
            unsafe {
                let r = &*(lparam as *const RECT);
                SetWindowPos(hwnd, null_mut(), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
            }
            apply_layout();
            0
        }
        WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_THEMECHANGED => {
            // high contrast switched on or off: repaint with the right colours; the controls
            // get the message too, and the default handling runs
            // SAFETY: our window and its children.
            unsafe {
                if msg == WM_SYSCOLORCHANGE {
                    for (id, ..) in layout() {
                        SendMessageW(item(id), msg, wparam, lparam);
                    }
                }
                InvalidateRect(hwnd, null(), 1);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        // Enter in the window = the default button (Save)
        DM_GETDEFID => ((DC_HASDEFID as LRESULT) << 16) | ID_SAVE as LRESULT,
        WM_CLOSE => {
            close();
            0
        }
        WM_DESTROY => {
            HWND_.with(|c| c.set(null_mut()));
            0
        }
        // SAFETY: default handling.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub fn open() {
    let existing = window();
    if !existing.is_null() {
        // SAFETY: our window.
        unsafe {
            ShowWindow(existing, SW_SHOWNORMAL);
            SetForegroundWindow(existing);
        }
        return;
    }
    // the school's settings may have changed (an administration tool writes them without a notice)
    app::reload_policy();
    let s = app::settings();
    let l = app::lang_of(&s);
    let class = wide("KBLayoutFixerSettings");
    let title = wide(text(l, Msg::WindowTitle));
    // SAFETY: class registered once; strings outlive the calls; window owned by the UI thread.
    unsafe {
        if BRUSH.with(|b| b.get()).is_null() {
            BRUSH.with(|b| b.set(GetSysColorBrush(COLOR_WINDOW)));
        }
        // the class (and its icons) is registered only once per process
        static REGISTERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        let first = !REGISTERED.swap(true, std::sync::atomic::Ordering::SeqCst);
        let (big, small) = if first {
            (app::app_icon(GetSystemMetrics(SM_CXICON)), app::app_icon(GetSystemMetrics(SM_CXSMICON)))
        } else {
            (null_mut(), null_mut())
        };
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: app::instance(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: BRUSH.with(|b| b.get()),
            hIcon: big,
            hIconSm: small,
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        if first {
            RegisterClassExW(&wc);
        }
        let mut ex = WS_EX_CONTROLPARENT | WS_EX_APPWINDOW;
        if l.is_rtl() {
            ex |= WS_EX_LAYOUTRTL;
        }
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
        let hwnd = CreateWindowExW(ex, class.as_ptr(), title.as_ptr(), style, CW_USEDEFAULT, CW_USEDEFAULT, 100, 100, null_mut(), null_mut(), app::instance(), null());
        if hwnd.is_null() {
            log("Settings window could not be created");
            return;
        }
        HWND_.with(|c| c.set(hwnd));
        let dpi = GetDpiForWindow(hwnd).max(96);
        DPI.with(|d| d.set(dpi));
        make_fonts(dpi);

        // creation order = Tab order (children are added at the bottom of the Z order)
        let stat = SS_LEFT | SS_CENTERIMAGE | SS_ENDELLIPSIS;
        create("STATIC", "", stat, 0, ID_STATUS);
        create("BUTTON", "", BS_PUSHBUTTON | WS_TABSTOP, 0, ID_PAUSE);
        // Simple
        create("STATIC", "", stat, 0, ID_LBL_HOTKEY);
        create("msctls_hotkey32", "", WS_TABSTOP | WS_BORDER, 0, ID_HOTKEY);
        create("STATIC", "", stat, 0, ID_LBL_HOTKEY_CASE);
        create("msctls_hotkey32", "", WS_TABSTOP | WS_BORDER, 0, ID_HOTKEY_CASE);
        create("STATIC", "", stat, 0, ID_LBL_WORK);
        create("COMBOBOX", "", CBS_DROPDOWNLIST | WS_TABSTOP | WS_VSCROLL, 0, ID_WORK_MODE);
        create("BUTTON", "", BS_PUSHBUTTON | WS_TABSTOP, 0, ID_COPY_RESULT);
        create("STATIC", "", SS_LEFT | SS_ENDELLIPSIS, 0, ID_LBL_TRY);
        create("EDIT", "", ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN | WS_VSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_TRY);
        create("STATIC", "", SS_LEFT | SS_ENDELLIPSIS, 0, ID_LBL_RESULT);
        create("EDIT", "", ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY | WS_VSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_RESULT);
        create("STATIC", "", stat, 0, ID_LBL_LANG);
        create("COMBOBOX", "", CBS_DROPDOWNLIST | WS_TABSTOP | WS_VSCROLL, 0, ID_LANG);
        create("BUTTON", "", BS_AUTOCHECKBOX | WS_TABSTOP, 0, ID_AUTOSTART);
        create("BUTTON", "", BS_AUTOCHECKBOX | WS_TABSTOP, 0, ID_INDICATOR);
        // Advanced
        create("STATIC", "", stat, 0, ID_LBL_DIRECTION);
        create("COMBOBOX", "", CBS_DROPDOWNLIST | WS_TABSTOP | WS_VSCROLL, 0, ID_DIRECTION);
        for id in [ID_SWITCH, ID_BUBBLE, ID_SOUND, ID_CAREFUL, ID_LASTWORD, ID_CAPSOFF, ID_CONSOLE] {
            create("BUTTON", "", BS_AUTOCHECKBOX | WS_TABSTOP, 0, id);
            if id == ID_BUBBLE {
                create("COMBOBOX", "", CBS_DROPDOWNLIST | WS_TABSTOP | WS_VSCROLL, 0, ID_BUBBLE_POS);
            }
        }
        create("STATIC", "", stat, 0, ID_LBL_LIMIT);
        create("EDIT", "", ES_NUMBER | ES_AUTOHSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_LIMIT);
        create("STATIC", "", stat, 0, ID_LBL_DELAY);
        create("EDIT", "", ES_NUMBER | ES_AUTOHSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_DELAY);
        create("STATIC", "", stat, 0, ID_LBL_EXCLUDED);
        create("EDIT", "", ES_AUTOHSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_EXCLUDED);
        create("STATIC", "", stat, 0, ID_LAYOUTS);
        // Diagnostic
        create("STATIC", "", SS_LEFT, 0, ID_DIAG_LAST);
        create("EDIT", "", ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY | WS_VSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_DIAG_TEXT);
        create("BUTTON", "", BS_PUSHBUTTON | WS_TABSTOP, 0, ID_DIAG_COPY);
        create("BUTTON", "", BS_PUSHBUTTON | WS_TABSTOP, 0, ID_DIAG_REFRESH);
        create("BUTTON", "", BS_PUSHBUTTON | WS_TABSTOP, 0, ID_EXPORT);
        create("BUTTON", "", BS_PUSHBUTTON | WS_TABSTOP, 0, ID_IMPORT);
        create("BUTTON", "", BS_PUSHBUTTON | WS_TABSTOP, 0, ID_UNDO_IMPORT);
        // common bottom row
        create("BUTTON", "", BS_PUSHBUTTON | WS_TABSTOP, 0, ID_ABOUT);
        create("BUTTON", "", BS_DEFPUSHBUTTON | WS_TABSTOP, 0, ID_SAVE);
        create("BUTTON", "", BS_PUSHBUTTON | WS_TABSTOP, 0, ID_CLOSE);
        // the tab control last: at the bottom of the Z order, under the page controls (clicks
        // reach them), and last in the Tab order (Ctrl+Tab changes the page from anywhere)
        create("SysTabControl32", "", WS_TABSTOP | WS_CLIPSIBLINGS, 0, ID_TAB);
        SetWindowPos(item(ID_TAB), HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);

        fill_controls();
        apply_layout();
        show_page(0);

        // window size from the client size, centred on the screen of the cursor; on a small
        // work area the elastic parts give up to SHRINK_MAX units
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(mon, &mut mi);
        let wa = mi.rcWork;
        let mut frame = RECT { left: 0, top: 0, right: 100, bottom: 100 };
        AdjustWindowRectExForDpi(&mut frame, style, 0, ex, dpi);
        let frame_h = (frame.bottom - frame.top) - 100;
        let room = (wa.bottom - wa.top - frame_h) * 96 / dpi as i32; // client height available, 96-DPI units
        SHRINK.with(|s| s.set((FULL_HEIGHT - room).clamp(0, SHRINK_MAX)));
        apply_layout();
        let mut rc = RECT { left: 0, top: 0, right: scale(WIDTH), bottom: scale(height()) };
        AdjustWindowRectExForDpi(&mut rc, style, 0, ex, dpi);
        let (ww, wh) = (rc.right - rc.left, rc.bottom - rc.top);
        let x = wa.left + ((wa.right - wa.left) - ww) / 2;
        let y = wa.top + ((wa.bottom - wa.top) - wh) / 2;
        // a window taller than the work area starts at its top (its buttons stay reachable
        // with the keyboard: Enter = Save, Esc = Close)
        SetWindowPos(hwnd, null_mut(), x, y.max(wa.top), ww, wh, SWP_NOZORDER);
        ShowWindow(hwnd, SW_SHOWNORMAL);
        SetForegroundWindow(hwnd);
        SetFocus(item(ID_TRY));
    }
}

use windows_sys::Win32::UI::HiDpi::AdjustWindowRectExForDpi;
