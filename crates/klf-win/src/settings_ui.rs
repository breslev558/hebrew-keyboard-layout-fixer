//! The settings window: native Windows controls (Common Controls v6), system font at the
//! right DPI, mirrored right-to-left in Hebrew (WS_EX_LAYOUTRTL set at creation).
//! 2.0.0, in plain words: six tabs, General (language, "fixer on", start, language sign, message
//! after a fix, sound, a box to try or convert a text), Keys (the two keys and how they act),
//! While typing (suggestions, my words, text shortcuts), Programs (lists of programs), Advanced
//! (what one normally never changes, with a line saying what was changed) and Help (what happened
//! last time, keyboards, technical details, problem reports, updates, previous settings, origin of
//! the program). The controls of every tab are children of the window, shown or hidden with the
//! tab; Ctrl+Tab changes the tab. A grey line under an option explains it in one sentence.

use crate::app::{self, HotkeyState};
use crate::platform::sys;
use crate::util::{log, wide};
use klf_core::convert::{convert_with, swap_case};
use klf_core::i18n::{text, text_with_key, Lang, Msg};
use klf_core::layouts::{KeyMap, LatinKind};
use klf_core::settings::{
    hotkey_allowed, Action, KEY_NONE, key_to_vk, vk_to_key, AppSettings, MixedTextMode, RuleMode, UiLanguage, MOD_ALT, MOD_CONTROL, MOD_SHIFT,
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
const ID_TAB: i32 = 240;
const ID_SAVE: i32 = 214;
const ID_CLOSE: i32 = 2; // IDCANCEL: Esc closes the window
/// 2.0.0: "Defaults" (of the page shown), where the About button was
const ID_DEFAULTS: i32 = 269;
// General
const ID_LBL_LANG: i32 = 234;
const ID_LANG: i32 = 212;
const ID_ENABLED: i32 = 266;
const ID_NOTE_ENABLED: i32 = 267;
const ID_AUTOSTART: i32 = 211;
const ID_INDICATOR: i32 = 259;
const ID_BUBBLE: i32 = 208;
const ID_BUBBLE_POS: i32 = 263;
const ID_PREVIEW: i32 = 268;
const ID_NOTE_PREVIEW: i32 = 298;
const ID_SOUND: i32 = 209;
const ID_WORK_MODE: i32 = 243;
const ID_LBL_TRY: i32 = 232;
const ID_TRY: i32 = 205;
const ID_RESULT: i32 = 206;
const ID_COPY_RESULT: i32 = 244;
// Keys
const ID_LBL_HOTKEY: i32 = 230;
const ID_HOTKEY: i32 = 203;
const ID_NOTE_HOTKEY: i32 = 299;
const ID_LBL_HOTKEY_CASE: i32 = 241;
const ID_HOTKEY_CASE: i32 = 236;
const ID_NOTE_HOTKEY_CASE: i32 = 300;
const ID_SWITCH: i32 = 207;
const ID_NOTE_SWITCH: i32 = 301;
const ID_CAREFUL: i32 = 245;
const ID_NOTE_CAREFUL: i32 = 274;
// While typing
const ID_TYPING: i32 = 276;
const ID_NOTE_TYPING_ON: i32 = 302;
const ID_TYPING_WORDS: i32 = 278;
const ID_NOTE_WORDS: i32 = 303;
const ID_REPLACEMENTS: i32 = 279;
const ID_NOTE_SHORTCUTS: i32 = 304;
const ID_REPLACEMENTS_FILE: i32 = 280;
const ID_NOTE_TYPING: i32 = 282;
// Programs
const ID_LBL_RULE_OFF: i32 = 284;
const ID_RULE_OFF: i32 = 285;
const ID_NOTE_RULE_OFF: i32 = 305;
const ID_LBL_RULE_SEL: i32 = 286;
const ID_RULE_SEL: i32 = 287;
const ID_NOTE_RULE_SEL: i32 = 306;
const ID_NOTE_RULES2: i32 = 288;
const ID_ONE_LANGUAGE: i32 = 289;
const ID_NOTE_ONE_LANGUAGE: i32 = 308;
const ID_ONE_LANGUAGE_FILE: i32 = 290;
// Advanced
const ID_ADV_HEAD: i32 = 309;
const ID_LBL_DIRECTION: i32 = 231;
const ID_DIRECTION: i32 = 204;
const ID_F10_WRONG: i32 = 272;
const ID_DOUBLE_SHIFT: i32 = 270;
const ID_CTRL_TAP: i32 = 271;
/// 1.9.5: F10 in Windows Terminal (hidden on a PC managed by an administrator)
const ID_TERMINAL: i32 = 265;
const ID_TYPING_SOUND: i32 = 277;
const ID_MESSENGER: i32 = 281;
const ID_NOTE_MESSENGER: i32 = 310;
const ID_LBL_LIMIT: i32 = 247;
const ID_LIMIT: i32 = 248;
const ID_LBL_DELAY: i32 = 249;
const ID_DELAY: i32 = 250;
const ID_NOTE_LIMIT: i32 = 275;
const ID_EXPORT: i32 = 260;
const ID_IMPORT: i32 = 261;
// Help
const ID_PRIVACY: i32 = 311;
const ID_DIAG_LAST: i32 = 254;
const ID_LAYOUTS: i32 = 253;
const ID_DIAG_COPY: i32 = 256;
const ID_SHOW_REPORT: i32 = 312;
const ID_DIAG_TEXT: i32 = 255;
const ID_REPORT_PROBLEM: i32 = 291;
const ID_REPORT_SEND: i32 = 292;
const ID_LBL_REPORTS: i32 = 313;
const ID_REPORT_MODE: i32 = 314;
/// 1.8.9: "Check for updates" (2.0.0: Help page; the Windows test clicks it by this id)
const ID_CHECK_UPDATE: i32 = 264;
const ID_AUTO_UPDATE: i32 = 296;
const ID_UNDO_IMPORT: i32 = 262;
const ID_ABOUT_TEXT: i32 = 295;
/// 2.0.0: SysLinks « Our project on GitHub » and « Original project by Ravid Saadia » (each opens
/// one fixed address only)
const ID_GITHUB: i32 = 297;
const ID_ORIGIN: i32 = 315;

/// The only addresses the links open (the text of the controls is never used as an address).
const GITHUB_URL: &str = "https://github.com/breslev558/hebrew-keyboard-layout-fixer";
const ORIGIN_URL: &str = "https://github.com/RavidSaadia/Keyboard_Layout_Fixer";

const PAGE_GENERAL: &[i32] = &[
    ID_LBL_LANG, ID_LANG, ID_ENABLED, ID_NOTE_ENABLED, ID_AUTOSTART, ID_INDICATOR, ID_BUBBLE, ID_BUBBLE_POS, ID_PREVIEW, ID_NOTE_PREVIEW, ID_SOUND, ID_WORK_MODE, ID_LBL_TRY, ID_TRY, ID_RESULT,
    ID_COPY_RESULT,
];
const PAGE_KEYS: &[i32] = &[ID_LBL_HOTKEY, ID_HOTKEY, ID_NOTE_HOTKEY, ID_LBL_HOTKEY_CASE, ID_HOTKEY_CASE, ID_NOTE_HOTKEY_CASE, ID_SWITCH, ID_NOTE_SWITCH, ID_CAREFUL, ID_NOTE_CAREFUL];
const PAGE_TYPING: &[i32] = &[ID_TYPING, ID_NOTE_TYPING_ON, ID_TYPING_WORDS, ID_NOTE_WORDS, ID_REPLACEMENTS, ID_NOTE_SHORTCUTS, ID_REPLACEMENTS_FILE, ID_NOTE_TYPING];
const PAGE_APPS: &[i32] = &[
    ID_LBL_RULE_OFF, ID_RULE_OFF, ID_NOTE_RULE_OFF, ID_LBL_RULE_SEL, ID_RULE_SEL, ID_NOTE_RULE_SEL, ID_NOTE_RULES2, ID_ONE_LANGUAGE,
    ID_NOTE_ONE_LANGUAGE, ID_ONE_LANGUAGE_FILE,
];
const PAGE_ADVANCED: &[i32] = &[
    ID_ADV_HEAD, ID_LBL_DIRECTION, ID_DIRECTION, ID_F10_WRONG, ID_DOUBLE_SHIFT, ID_CTRL_TAP, ID_TERMINAL, ID_TYPING_SOUND, ID_MESSENGER, ID_NOTE_MESSENGER, ID_LBL_LIMIT,
    ID_LIMIT, ID_LBL_DELAY, ID_DELAY, ID_NOTE_LIMIT, ID_EXPORT, ID_IMPORT,
];
const PAGE_HELP: &[i32] = &[
    ID_PRIVACY, ID_DIAG_LAST, ID_LAYOUTS, ID_DIAG_COPY, ID_SHOW_REPORT, ID_DIAG_TEXT, ID_REPORT_PROBLEM, ID_REPORT_SEND, ID_LBL_REPORTS, ID_REPORT_MODE, ID_CHECK_UPDATE, ID_AUTO_UPDATE,
    ID_UNDO_IMPORT, ID_ABOUT_TEXT, ID_GITHUB, ID_ORIGIN,
];
/// 2.0.0: the pages, in the order of the tabs (never a number written by hand elsewhere)
const PAGES: [&[i32]; 6] = [PAGE_GENERAL, PAGE_KEYS, PAGE_TYPING, PAGE_APPS, PAGE_ADVANCED, PAGE_HELP];
const TABS: [Msg; 6] = [Msg::TabGeneral, Msg::TabConversion, Msg::TabTyping, Msg::TabApps, Msg::TabAdvanced, Msg::TabAbout];
const PAGE_ADV: usize = 4;
/// The Help tab (opened by « About » in the tray menu; holds what was the Diagnostic page).
pub const PAGE_ABOUT: usize = 5;
/// grey explanation lines (never reached by Tab; never just before a field, whose name they would become)
const NOTE_IDS: &[i32] = &[
    ID_NOTE_ENABLED, ID_NOTE_PREVIEW, ID_NOTE_HOTKEY, ID_NOTE_HOTKEY_CASE, ID_NOTE_SWITCH, ID_NOTE_CAREFUL, ID_NOTE_TYPING_ON, ID_NOTE_WORDS, ID_NOTE_SHORTCUTS, ID_NOTE_TYPING, ID_NOTE_RULE_OFF,
    ID_NOTE_RULE_SEL, ID_NOTE_RULES2, ID_NOTE_ONE_LANGUAGE, ID_ADV_HEAD, ID_NOTE_MESSENGER, ID_NOTE_LIMIT,
];
/// 2.0.0: what a page gives up when it does not fit any more, all at
/// once: its grey lines, and on the Help page the privacy line and the About text. The options
/// keep their explanation as a description for a screen reader.
const HIDEABLE_IDS: &[i32] = &[
    ID_NOTE_ENABLED, ID_NOTE_PREVIEW, ID_NOTE_HOTKEY, ID_NOTE_HOTKEY_CASE, ID_NOTE_SWITCH, ID_NOTE_CAREFUL, ID_NOTE_TYPING_ON, ID_NOTE_WORDS, ID_NOTE_SHORTCUTS, ID_NOTE_RULE_OFF, ID_NOTE_RULE_SEL,
    ID_NOTE_RULES2, ID_NOTE_ONE_LANGUAGE, ID_ADV_HEAD, ID_NOTE_MESSENGER, ID_NOTE_LIMIT, ID_PRIVACY, ID_ABOUT_TEXT,
];
/// Help page: the controls the technical report covers when it is shown
const UNDER_REPORT: &[i32] = &[ID_REPORT_PROBLEM, ID_REPORT_SEND, ID_LBL_REPORTS, ID_REPORT_MODE, ID_CHECK_UPDATE, ID_AUTO_UPDATE, ID_UNDO_IMPORT, ID_ABOUT_TEXT, ID_GITHUB, ID_ORIGIN];

/// 1.8.9: every control of the window has its own id (checked when compiling; 1.8.9 first gave the
/// new button the id of the bubble position list). 2.0.0: every control is on one page.
const _: () = {
    let common = [ID_STATUS, ID_TAB, ID_SAVE, ID_CLOSE, ID_DEFAULTS];
    let mut ids = [0i32; 160];
    let mut n = 0;
    let mut i = 0;
    while i < common.len() {
        ids[n] = common[i];
        n += 1;
        i += 1;
    }
    let mut p = 0;
    while p < PAGES.len() {
        let mut j = 0;
        while j < PAGES[p].len() {
            ids[n] = PAGES[p][j];
            n += 1;
            j += 1;
        }
        p += 1;
    }
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n {
            assert!(ids[i] != ids[j], "two controls of the settings window share an id");
            j += 1;
        }
        i += 1;
    }
    assert!(PAGES.len() == TABS.len() && PAGE_ABOUT < PAGES.len() && PAGE_ADV < PAGES.len());
};

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
const CB_GETDROPPEDSTATE: u32 = 0x0157;
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
const EM_GETMODIFY: u32 = 0x00B8;
const EM_SETMODIFY: u32 = 0x00B9;
const CBN_SELCHANGE: u32 = 1;
const CBN_CLOSEUP: u32 = 8;
const SS_LEFT: u32 = 0;
const SS_CENTERIMAGE: u32 = 0x0200;
/// 1.9.9: a static that wraps its words like a multi-line edit (never with SS_ENDELLIPSIS, which
/// forces one line); SS_NOPREFIX: an & is shown as it is
const SS_EDITCONTROL: u32 = 0x2000;
const SS_NOPREFIX: u32 = 0x0080;
const SS_ENDELLIPSIS: u32 = 0x4000;
const TCM_INSERTITEMW: u32 = 0x1300 + 62;
const TCM_GETCURSEL: u32 = 0x1300 + 11;
const TCM_SETCURSEL: u32 = 0x1300 + 12;
const TCIF_TEXT: u32 = 1;
const TCN_SELCHANGE: u32 = (-551i32) as u32;
/// SysLink (commctrl.h): transparent background; notifications of a click and of Enter
const LWS_TRANSPARENT: u32 = 0x0001;
const NM_CLICK: u32 = (-2i32) as u32;
const NM_RETURN: u32 = (-4i32) as u32;
/// 2.0.0: the language chosen with the keyboard (list closed) is applied after this pause, so
/// that going through the list with the arrows does not rebuild the window at every item
const TIMER_LANG: usize = 1;
const WM_APP_LANG: u32 = WM_APP + 21;

/// Client size in 96-DPI units. When the screen is too small (200 % on 768 px), up to SHRINK_MAX
/// units are taken from the elastic parts only: text boxes, spacing of the check boxes, and (2.0.0)
/// the grey lines of a page that would not fit.
/// 2.0.0: 452 units = 904 px at 200 %, under a 1080 px screen with its taskbar; 368 at least
/// (768 px at 175 % with the taskbar).
const WIDTH: i32 = 500;
const FULL_HEIGHT: i32 = 452;
const SHRINK_MAX: i32 = 84;

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
    /// 2.0.0: a language was picked in the open list: applied when the list closes
    static LANG_PENDING: Cell<bool> = const { Cell::new(false) };
    /// 2.0.0: pages whose grey lines are hidden (small screen; worked out by layout)
    static HIDDEN_NOTES: Cell<[bool; 6]> = const { Cell::new([false; 6]) };
    /// 2.0.0: the technical report of the Help tab is shown
    static REPORT_SHOWN: Cell<bool> = const { Cell::new(false) };
    /// 2.0.0: the key in the box (fix key, capitals key) is held by another program
    static HOTKEY_WARN: Cell<[bool; 2]> = const { Cell::new([false; 2]) };
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

fn enable(id: i32, on: bool) {
    // SAFETY: our control.
    unsafe { EnableWindow(item(id), on as i32) };
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

/// 1.9.9 (Windows test of 17/11): a screen reader names a classic control after the label just
/// before it; the bubble position list (next to its check box) and the tab strip had no name. The
/// name is given through oleacc's dynamic annotation (IAccPropServices::SetHwndPropStr), and
/// cleared at WM_DESTROY.
/// 2.0.0: the GitHub link gets its name the same way, and the options explained by a grey line get
/// that line as their description (read after the name by a screen reader).
const CLSID_ACC_PROP_SERVICES: windows_sys::core::GUID = windows_sys::core::GUID::from_u128(0xb5f8350b_0548_48b1_a6ee_88bd00b4a5e7);
const IID_IACC_PROP_SERVICES: windows_sys::core::GUID = windows_sys::core::GUID::from_u128(0x6e26e776_04f0_495d_80e4_3330352e3169);
const PROPID_ACC_NAME_: windows_sys::core::GUID = windows_sys::core::GUID::from_u128(0x608d3df8_8128_4aa7_a428_f55e49267291);
/// PROPID_ACC_DESCRIPTION (oleacc.h: 4d48dfe4-bd3f-491f-a648-492d6f20c588)
const PROPID_ACC_DESCRIPTION_: windows_sys::core::GUID = windows_sys::core::GUID::from_u128(0x4d48dfe4_bd3f_491f_a648_492d6f20c588);
const NAMED_IDS: [i32; 9] = [ID_BUBBLE_POS, ID_TAB, ID_GITHUB, ID_ORIGIN, ID_ABOUT_TEXT, ID_WORK_MODE, ID_RESULT, ID_DIAG_TEXT, ID_TYPING_WORDS];
const DESCRIBED_IDS: [i32; 12] = [ID_ENABLED, ID_DIRECTION, ID_CAREFUL, ID_LIMIT, ID_DELAY, ID_RULE_OFF, ID_RULE_SEL, ID_PREVIEW, ID_SWITCH, ID_TYPING, ID_REPLACEMENTS, ID_MESSENGER];
// Method numbers in the vtable, from oleacc.h (IAccPropServicesVtbl: QueryInterface 0, AddRef 1,
// Release 2, SetPropValue 3, SetPropServer 4, ClearProps 5, SetHwndProp 6, SetHwndPropStr 7,
// SetHwndPropServer 8, ClearHwndProps 9). Checked against the header by tools/verifier-vtables.py
// in the CI: keep the « vtable » comments as they are.
const ACC_RELEASE: usize = 2; // vtable IAccPropServices::Release
const ACC_SET_HWND_PROP_STR: usize = 7; // vtable IAccPropServices::SetHwndPropStr
const ACC_CLEAR_HWND_PROPS: usize = 9; // vtable IAccPropServices::ClearHwndProps

/// Runs `f` with an IAccPropServices (nothing if COM or the service is not available).
fn with_acc_props(f: impl FnOnce(*mut std::ffi::c_void)) {
    use windows_sys::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
    thread_local! { static COM: Cell<bool> = const { Cell::new(false) }; }
    // SAFETY: COM for this (UI) thread, once, never uninitialized while the thread lives
    // (RPC_E_CHANGED_MODE = already multithreaded: usable as it is); the object is released here;
    // IUnknown::Release is method 2.
    unsafe {
        if !COM.with(|c| c.replace(true)) {
            CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
        }
        let mut p: *mut std::ffi::c_void = null_mut();
        if CoCreateInstance(&CLSID_ACC_PROP_SERVICES, null_mut(), CLSCTX_INPROC_SERVER, &IID_IACC_PROP_SERVICES, &mut p) < 0 || p.is_null() {
            return;
        }
        f(p);
        let vtbl = *(p as *const *const usize);
        let release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32 = std::mem::transmute(*vtbl.add(ACC_RELEASE));
        release(p);
    }
}

fn set_acc_names(l: Lang, key: &str) {
    let t = |m: Msg| text(l, m).to_string();
    let names: Vec<(i32, windows_sys::core::GUID, String)> = vec![
        (ID_BUBBLE_POS, PROPID_ACC_NAME_, t(Msg::NameBubblePosition)),
        (ID_TAB, PROPID_ACC_NAME_, t(Msg::NameSettingsPages)),
        (ID_GITHUB, PROPID_ACC_NAME_, t(Msg::LinkOurs)),
        (ID_ORIGIN, PROPID_ACC_NAME_, t(Msg::LinkOrigin)),
        (ID_ABOUT_TEXT, PROPID_ACC_NAME_, t(Msg::MenuAbout)),
        (ID_WORK_MODE, PROPID_ACC_NAME_, t(Msg::NameWorkMode)),
        (ID_RESULT, PROPID_ACC_NAME_, t(Msg::LabelResult)),
        (ID_DIAG_TEXT, PROPID_ACC_NAME_, t(Msg::NameReport)),
        // a button keeps its own text as name; the description is read after it
        (ID_TYPING_WORDS, PROPID_ACC_DESCRIPTION_, t(Msg::NoteWords)),
        (ID_ENABLED, PROPID_ACC_DESCRIPTION_, t(Msg::NoteEnabled)),
        (ID_DIRECTION, PROPID_ACC_DESCRIPTION_, t(Msg::NoteDirection)),
        (ID_CAREFUL, PROPID_ACC_DESCRIPTION_, t(Msg::NoteCareful)),
        (ID_LIMIT, PROPID_ACC_DESCRIPTION_, text_with_key(l, Msg::NoteLimitDelay, key)),
        (ID_DELAY, PROPID_ACC_DESCRIPTION_, text_with_key(l, Msg::NoteLimitDelay, key)),
        (ID_RULE_OFF, PROPID_ACC_DESCRIPTION_, t(Msg::NoteRuleOff)),
        (ID_RULE_SEL, PROPID_ACC_DESCRIPTION_, t(Msg::NoteRuleSel)),
        (ID_PREVIEW, PROPID_ACC_DESCRIPTION_, t(Msg::NotePreview)),
        (ID_SWITCH, PROPID_ACC_DESCRIPTION_, t(Msg::NoteSwitch)),
        (ID_TYPING, PROPID_ACC_DESCRIPTION_, t(Msg::NoteTypingOn)),
        (ID_REPLACEMENTS, PROPID_ACC_DESCRIPTION_, t(Msg::NoteShortcuts)),
        (ID_MESSENGER, PROPID_ACC_DESCRIPTION_, text_with_key(l, Msg::NoteMessenger, key)),
    ];
    with_acc_props(|p| {
        type SetHwndPropStr = unsafe extern "system" fn(*mut std::ffi::c_void, HWND, u32, u32, windows_sys::core::GUID, *const u16) -> i32;
        // SAFETY: IAccPropServices (IUnknown 3, SetPropValue, SetPropServer, ClearProps, SetHwndProp,
        // then SetHwndPropStr = method 7); OBJID_CLIENT, CHILDID_SELF; the string is copied by oleacc.
        unsafe {
            let vtbl = *(p as *const *const usize);
            let set: SetHwndPropStr = std::mem::transmute(*vtbl.add(ACC_SET_HWND_PROP_STR));
            for (id, prop, value) in names {
                let w = wide(&value);
                set(p, item(id), OBJID_CLIENT as u32, CHILDID_SELF, prop, w.as_ptr());
            }
        }
    });
}

fn clear_acc_names() {
    with_acc_props(|p| {
        type ClearHwndProps = unsafe extern "system" fn(*mut std::ffi::c_void, HWND, u32, u32, *const windows_sys::core::GUID, i32) -> i32;
        let props = [PROPID_ACC_NAME_, PROPID_ACC_DESCRIPTION_];
        // SAFETY: IAccPropServices::ClearHwndProps is method 9 (after SetHwndPropStr 7 and
        // SetHwndPropServer 8, oleacc.idl; avis Fable); two property ids in the array.
        unsafe {
            let vtbl = *(p as *const *const usize);
            let clear: ClearHwndProps = std::mem::transmute(*vtbl.add(ACC_CLEAR_HWND_PROPS));
            for id in NAMED_IDS.iter().chain(DESCRIBED_IDS.iter()) {
                clear(p, item(*id), OBJID_CLIENT as u32, CHILDID_SELF, props.as_ptr(), props.len() as i32);
            }
        }
    });
}

/// 1.8.9: technical text (diagnostic, program names, numbers) always left to right, even in the
/// Hebrew window. The window does not hand its mirroring down (WS_EX_NOINHERITLAYOUT): every
/// other control is mirrored here, one by one. (1.8.5 took the mirroring off AFTER creation, which
/// an EDIT ignores: the text stayed on the right.)
/// 2.0.0: the program lists of the rules and the two key boxes too ("Ctrl + F10" stays in order).
const LTR_IDS: &[i32] = &[ID_DIAG_TEXT, ID_RULE_OFF, ID_RULE_SEL, ID_LIMIT, ID_DELAY, ID_HOTKEY, ID_HOTKEY_CASE];

thread_local! {
    static MIRRORED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn create(class: &str, txt: &str, style: u32, ex: u32, id: i32) {
    let c = wide(class);
    let t = wide(txt);
    let ex = if MIRRORED.with(|m| m.get()) && !LTR_IDS.contains(&id) { ex | WS_EX_LAYOUTRTL } else { ex };
    // SAFETY: child of our window; strings outlive the call.
    unsafe {
        CreateWindowExW(ex, c.as_ptr(), t.as_ptr(), WS_CHILD | WS_VISIBLE | style, 0, 0, 10, 10, window(), id as usize as HMENU, app::instance(), null());
    }
}

type Place = (i32, i32, i32, i32, i32);

/// The bottom of the page area (96-DPI units): content never goes lower.
fn page_limit() -> i32 {
    height() - 46 - 4
}

/// One page laid out from its top; `notes`: with its grey lines. Returns the places and the
/// bottom reached. Hidden grey lines keep a place (they are hidden), they take no room.
fn page_layout(page: usize, k: i32, notes: bool) -> (Vec<Place>, i32) {
    let m = 16;
    let x0 = m + 10; // inside the tab
    let lw = 158; // label column
    let cx = x0 + lw;
    let right = WIDTH - m - 10;
    let cw = right - cx;
    let full = right - x0;
    let ind = 18; // an option that depends on the one above, or the grey line of an option
    let note = 18; // one grey line
    let note2 = 2 * note - 2; // room for two lines (pages with room: a longer translation never cut)
    let gap = |n: i32| if notes { n } else { 0 }; // the room of a grey line, or none
    let ch = if k > 0 { 18 } else { 20 }; // check boxes a little closer on a small screen
    let mut v: Vec<Place> = Vec::new();
    let mut y = 74;
    match page {
        0 => {
            // « Language · שפה · Langue » in three scripts: a wider label than the others
            v.push((ID_LBL_LANG, x0, y + 2, 190, 22));
            v.push((ID_LANG, x0 + 194, y, full - 194, 200));
            y += 30;
            v.push((ID_ENABLED, x0, y, full, 20));
            y += 20;
            v.push((ID_NOTE_ENABLED, x0 + ind, y, full - ind, note));
            y += gap(note + 4);
            v.push((ID_AUTOSTART, x0, y, full, ch));
            y += ch;
            v.push((ID_INDICATOR, x0, y, full, ch));
            y += ch;
            // the message box and its place on one line (the list is named for a screen reader)
            v.push((ID_BUBBLE, x0, y + 1, 262, ch));
            v.push((ID_BUBBLE_POS, x0 + 266, y - 1, full - 266, 200));
            y += ch + 4;
            v.push((ID_PREVIEW, x0 + ind, y, full - ind, ch));
            y += ch;
            v.push((ID_NOTE_PREVIEW, x0 + 2 * ind, y, full - 2 * ind, note));
            y += gap(note + 4);
            v.push((ID_SOUND, x0, y, full, ch));
            y += ch + 8;
            v.push((ID_LBL_TRY, x0, y + 2, 204, 22));
            v.push((ID_WORK_MODE, x0 + 208, y, full - 208, 200));
            y += 28;
            let half = (full - 8) / 2;
            let tk = (k / 2).min(22); // the text boxes give some of the room on a small screen
            v.push((ID_TRY, x0, y, half, 52 - tk));
            v.push((ID_RESULT, x0 + half + 8, y, half, 52 - tk));
            y += 56 - tk;
            v.push((ID_COPY_RESULT, right - 160, y, 160, 26));
            y += 26;
        }
        1 => {
            for (lbl, id, n) in [(ID_LBL_HOTKEY, ID_HOTKEY, ID_NOTE_HOTKEY), (ID_LBL_HOTKEY_CASE, ID_HOTKEY_CASE, ID_NOTE_HOTKEY_CASE)] {
                v.push((lbl, x0, y + 2, lw - 6, 22));
                v.push((id, cx, y, cw, 24));
                y += 26;
                v.push((n, x0, y, full, note2));
                y += gap(note2) + 6;
            }
            for (id, n) in [(ID_SWITCH, ID_NOTE_SWITCH), (ID_CAREFUL, ID_NOTE_CAREFUL)] {
                v.push((id, x0, y, full, 20));
                y += 20;
                v.push((n, x0 + ind, y, full - ind, note2));
                y += gap(note2) + 6;
            }
        }
        2 => {
            v.push((ID_TYPING, x0, y, full, 20));
            y += 20;
            v.push((ID_NOTE_TYPING_ON, x0 + ind, y, full - ind, note2));
            y += gap(note2) + 6;
            v.push((ID_TYPING_WORDS, x0 + ind, y, 180, 26));
            y += 28;
            v.push((ID_NOTE_WORDS, x0 + ind, y, full - ind, note2));
            y += gap(note2) + 12;
            v.push((ID_REPLACEMENTS, x0, y, full, 20));
            y += 20;
            v.push((ID_NOTE_SHORTCUTS, x0 + ind, y, full - ind, note2));
            y += gap(note2) + 6;
            v.push((ID_REPLACEMENTS_FILE, x0 + ind, y, 180, 26));
            y += 38;
            v.push((ID_NOTE_TYPING, x0, y, full, 3 * note - 4));
            y += 3 * note - 4;
        }
        3 => {
            // the label above its list (long labels; a screen reader names the list after it)
            for (lbl, id, n) in [(ID_LBL_RULE_OFF, ID_RULE_OFF, ID_NOTE_RULE_OFF), (ID_LBL_RULE_SEL, ID_RULE_SEL, ID_NOTE_RULE_SEL)] {
                v.push((lbl, x0, y, full, 18));
                y += 18;
                v.push((id, x0, y, full, 22));
                y += 24;
                v.push((n, x0, y, full, note));
                y += gap(note) + 4;
            }
            v.push((ID_NOTE_RULES2, x0, y, full, 2 * note - 2));
            y += gap(2 * note + 4);
            v.push((ID_ONE_LANGUAGE, x0, y, full, 20));
            y += 20;
            v.push((ID_NOTE_ONE_LANGUAGE, x0 + ind, y, full - ind, note));
            y += gap(note) + 4;
            v.push((ID_ONE_LANGUAGE_FILE, x0 + ind, y, 180, 26));
            y += 26;
        }
        4 => {
            v.push((ID_ADV_HEAD, x0, y, full, 2 * note - 2));
            y += gap(2 * note + 2);
            v.push((ID_LBL_DIRECTION, x0, y + 2, lw - 6, 22));
            v.push((ID_DIRECTION, cx, y, cw, 200));
            y += 28;
            for id in [ID_F10_WRONG, ID_DOUBLE_SHIFT, ID_CTRL_TAP, ID_TERMINAL, ID_TYPING_SOUND, ID_MESSENGER] {
                v.push((id, x0, y, full, ch));
                y += ch;
                if id == ID_MESSENGER {
                    v.push((ID_NOTE_MESSENGER, x0 + ind, y, full - ind, note));
                    y += gap(note + 2);
                }
            }
            y += 4;
            v.push((ID_LBL_LIMIT, x0, y + 2, 186, 22));
            v.push((ID_LIMIT, x0 + 190, y, 50, 22));
            v.push((ID_LBL_DELAY, x0 + 250, y + 2, 150, 22));
            v.push((ID_DELAY, right - 46, y, 46, 22));
            y += 24;
            v.push((ID_NOTE_LIMIT, x0, y, full, note));
            y += gap(note) + 4;
            v.push((ID_EXPORT, x0, y, 180, 26));
            v.push((ID_IMPORT, x0 + 186, y, 180, 26));
            y += 26;
        }
        _ => {
            v.push((ID_PRIVACY, x0, y, full, note));
            y += gap(note + 2);
            v.push((ID_DIAG_LAST, x0, y, full, 32));
            y += 34;
            // 1.9.9: the keyboards line on two lines (three keyboards did not fit on one, Windows test)
            // 2.0.0: 34 units (two lines measured 50 px for 48 at 150 %, Windows test 200)
            v.push((ID_LAYOUTS, x0, y, full, 34));
            y += 36;
            v.push((ID_DIAG_COPY, x0, y, 228, 26));
            v.push((ID_SHOW_REPORT, x0 + 234, y, full - 234, 26));
            y += 28;
            let report_top = y;
            v.push((ID_REPORT_PROBLEM, x0, y, 176, 26));
            v.push((ID_REPORT_SEND, x0 + 182, y, full - 182, 26));
            y += 28;
            v.push((ID_LBL_REPORTS, x0, y + 2, 190, 22));
            v.push((ID_REPORT_MODE, x0 + 194, y, full - 194, 200));
            y += 26;
            v.push((ID_CHECK_UPDATE, x0, y, 210, 26));
            v.push((ID_AUTO_UPDATE, x0 + 218, y + 3, full - 218, 20));
            y += 28;
            v.push((ID_UNDO_IMPORT, x0, y, 210, 26));
            y += 28;
            v.push((ID_ABOUT_TEXT, x0, y, full, 80));
            y += gap(82);
            v.push((ID_GITHUB, x0, y, 210, 18));
            v.push((ID_ORIGIN, x0 + 214, y, full - 214, 18));
            y += 18;
            // the technical report, when shown, covers everything from the problem reports down
            v.push((ID_DIAG_TEXT, x0, report_top, full, (y - report_top).max(60)));
        }
    }
    (v, y)
}

/// (id, x, y, width, height) in 96-DPI units, for every control. 2.0.0: a page that does not fit
/// on a small screen gives up its grey lines (kept in HIDDEN_NOTES for show_page).
fn layout() -> Vec<Place> {
    let k = SHRINK.with(|s| s.get());
    #[allow(non_snake_case)]
    let HEIGHT = height();
    let m = 16;
    let mut v = vec![
        (ID_STATUS, m, 10, WIDTH - 2 * m, 26),
        (ID_TAB, m, 42, WIDTH - 2 * m, HEIGHT - 42 - 46),
        (ID_DEFAULTS, m, HEIGHT - 38, 140, 28),
        (ID_SAVE, WIDTH - m - 230, HEIGHT - 38, 110, 28),
        (ID_CLOSE, WIDTH - m - 110, HEIGHT - 38, 110, 28),
    ];
    let mut hidden = [false; 6];
    for (page, h) in hidden.iter_mut().enumerate() {
        let (mut places, end) = page_layout(page, k, true);
        if end > page_limit() {
            places = page_layout(page, k, false).0;
            *h = true;
        }
        v.extend(places);
    }
    HIDDEN_NOTES.with(|c| c.set(hidden));
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

/// 1.9.5: a PC managed by an administrator (any policy): F10 in the terminal is hidden there.
fn school_pc() -> bool {
    !app::policy().is_empty()
}

/// Shows the controls of one tab, hides the others.
fn show_page(page: usize) {
    cancel_language_timer();
    let page = page.min(PAGES.len() - 1);
    PAGE.with(|p| p.set(page));
    send(ID_TAB, TCM_SETCURSEL, page, 0);
    let hidden = HIDDEN_NOTES.with(|c| c.get());
    let report = REPORT_SHOWN.with(|r| r.get());
    for (i, ids) in PAGES.iter().enumerate() {
        for id in *ids {
            let show = i == page
                && !(*id == ID_TERMINAL && school_pc())
                && !(hidden[i] && HIDEABLE_IDS.contains(id))
                && (*id != ID_DIAG_TEXT || report)
                && !(report && UNDER_REPORT.contains(id));
            // SAFETY: our child windows.
            unsafe { ShowWindow(item(*id), if show { SW_SHOW } else { SW_HIDE }) };
        }
    }
    // "Defaults" only where there are settings to put back
    enable(ID_DEFAULTS, page != PAGE_ABOUT);
    if page == PAGE_ABOUT {
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
        let (p, n) = (PAGE.with(|p| p.get()), PAGES.len());
        show_page(if back { (p + n - 1) % n } else { (p + 1) % n });
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
        (Lang::He, LatinKind::OtherFromWindows) => "מקלדת לטינית אחרת",
        (Lang::Fr, LatinKind::OtherFromWindows) => "autre clavier latin",
        (Lang::En, LatinKind::OtherFromWindows) => "other Latin layout",
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
    // "Send the last report": only when one is waiting (public install only)
    enable(ID_REPORT_SEND, crate::report::enabled() && crate::report::pending());
}

pub fn refresh_status() {
    if window().is_null() {
        return;
    }
    let s = app::settings();
    let l = app::lang_of(&s);
    let key = s.hotkey_description();
    let state = app::hotkey_state();
    let msg = match state {
        HotkeyState::Ready => Msg::StatusReady,
        HotkeyState::Paused => Msg::StatusPaused,
        HotkeyState::Unavailable => Msg::StatusUnavailable,
    };
    let mut status = format!("●  {}", text_with_key(l, msg, &key));
    // the Caps Lock hotkey taken by another program: said here too (audit 1.3.5, N4)
    if state != HotkeyState::Paused && !app::case_hotkey_ok() {
        status.push_str(" · ");
        status.push_str(&text_with_key(l, Msg::CaseKeyBusy, &s.case_hotkey_description()));
    }
    if app::locked().any() {
        status.push_str(" · ");
        status.push_str(text(l, Msg::SchoolManaged));
    }
    set_text(ID_STATUS, &status);
    // 2.0.0: "Fixer on" is the pause state itself (the menu, the end of a 10-minute pause)
    set_checked(ID_ENABLED, state != HotkeyState::Paused);
    // SAFETY: repaint our control.
    unsafe { InvalidateRect(item(ID_STATUS), null(), 1) };
}

/// The user's rules of one kind, as shown in their field ("a.exe, b.exe").
fn rules_text(s: &AppSettings, mode: RuleMode) -> String {
    s.app_rules.iter().filter(|r| r.mode == mode).map(|r| r.process.as_str()).collect::<Vec<_>>().join(", ")
}

/// 2.0.0: a rule changed from the tray menu while the window is open: its two lists shown again
/// (otherwise « Apply » would write the old lists back).
pub fn refresh_rules() {
    if window().is_null() {
        return;
    }
    let u = app::user_settings();
    // a list the user is editing (not applied yet) is left as it is
    for (id, mode) in [(ID_RULE_OFF, RuleMode::Disabled), (ID_RULE_SEL, RuleMode::SelectionOnly)] {
        if send(id, EM_GETMODIFY, 0, 0) == 0 {
            set_text(id, &rules_text(&u, mode)); // (WM_SETTEXT clears the "modified" flag)
        }
    }
}

/// 2.0.0: the top of the Advanced tab says which of its settings are no longer as they came.
fn refresh_advanced_head() {
    let s = app::settings();
    let l = app::lang_of(&s);
    let changed = klf_core::settings::advanced_changes(&s);
    let head = if changed.is_empty() {
        text(l, Msg::AdvancedNothing).to_string()
    } else {
        let names: Vec<&str> = changed.iter().map(|m| text(l, *m)).collect();
        text(l, Msg::AdvancedChanged).replace("{list}", &names.join(", "))
    };
    set_text(ID_ADV_HEAD, &head);
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
            let ok = crate::platform::layouts::supported(*h);
            // 1.8.9: in Hebrew each item is closed by a right-to-left mark, so that the sign and the
            // number in brackets stay with their own layout (they were drawn in the wrong place)
            let end = if l.is_rtl() { "\u{200F}" } else { "" };
            format!("{}{}{end}", klf_core::layouts::hkl_name_in(*h, l), if ok { " ✓" } else { " ✗" })
        })
        .collect();
    text(l, Msg::LayoutsLine).replace("{list}", &list.join(", "))
}

fn lang_index(u: UiLanguage) -> usize {
    match u {
        UiLanguage::Auto => 0,
        UiLanguage::He => 1,
        UiLanguage::Fr => 2,
        UiLanguage::En => 3,
    }
}

fn lang_of_index(i: isize) -> UiLanguage {
    match i {
        1 => UiLanguage::He,
        2 => UiLanguage::Fr,
        3 => UiLanguage::En,
        _ => UiLanguage::Auto,
    }
}

fn report_index(mode: u32) -> usize {
    match mode {
        klf_core::report::ALWAYS => 1,
        klf_core::report::NEVER => 2,
        _ => 0,
    }
}

/// The options that only matter when the one above is ticked: the message (its place, the fixed
/// text in it), the suggestions (my words, their sound, chat apps), the shortcuts (my shortcuts).
fn update_dependents() {
    let lk = app::locked();
    let bubble = checked(ID_BUBBLE);
    enable(ID_BUBBLE_POS, !lk.bubble_position && bubble);
    enable(ID_PREVIEW, !lk.show_preview && bubble);
    let allowed = crate::typing::allowed();
    let typing = allowed && checked(ID_TYPING);
    for id in [ID_TYPING_WORDS, ID_TYPING_SOUND, ID_MESSENGER] {
        enable(id, typing);
    }
    enable(ID_REPLACEMENTS_FILE, allowed && checked(ID_REPLACEMENTS));
}

/// 2.0.0: a key that another program already holds is said as soon as it is chosen in its box
/// (a trial registration, released at once; our own keys are never tried: we hold them).
fn check_hotkey_box(id: i32) {
    let (note, normal, slot) = if id == ID_HOTKEY { (ID_NOTE_HOTKEY, Msg::NoteHotkey, 0) } else { (ID_NOTE_HOTKEY_CASE, Msg::NoteHotkeyCase, 1) };
    let hk = send(id, HKM_GETHOTKEY, 0, 0) as u32;
    let s = app::settings();
    let taken = match vk_to_key(hk & 0xFF) {
        Some(k) => {
            let mods = hotkeyf_to_mods((hk >> 8) & 0xFF);
            let ours = (mods, k) == (s.hotkey_modifiers, s.hotkey_key) || (mods, k) == (s.case_hotkey_modifiers, s.case_hotkey_key);
            hotkey_allowed(mods, k) && !ours && !app::hotkey_free(mods, k)
        }
        None => false,
    };
    HOTKEY_WARN.with(|w| {
        let mut v = w.get();
        v[slot] = taken;
        w.set(v);
    });
    let l = app::lang_of(&s);
    set_text(note, text(l, if taken { Msg::HotkeyTaken } else { normal }));
    // SAFETY: repaint our control (its colour changes).
    unsafe { InvalidateRect(item(note), null(), 1) };
}

/// A setting imposed by the administrator: a small lock after its text (or after its label).
const LOCK: &str = " 🔒";

fn add_lock(id: i32) {
    let t = get_text(id);
    if !t.ends_with(LOCK) {
        set_text(id, &format!("{t}{LOCK}"));
    }
}

fn fill_controls() {
    let s = app::settings();
    let u = app::user_settings();
    let l = app::lang_of(&s);
    let key = s.hotkey_description();
    set_acc_names(l, &key);
    for (i, m) in TABS.iter().enumerate() {
        let mut t = wide(text(l, *m));
        // SAFETY: TCITEMW read during the call; text buffer alive.
        let mut it: TCITEMW = unsafe { std::mem::zeroed() };
        it.mask = TCIF_TEXT;
        it.pszText = t.as_mut_ptr();
        send(ID_TAB, TCM_INSERTITEMW, i, &it as *const TCITEMW as isize);
    }
    for (id, m) in [
        (ID_LBL_LANG, Msg::LabelLanguageFixed),
        (ID_ENABLED, Msg::CheckEnabled),
        (ID_NOTE_ENABLED, Msg::NoteEnabled),
        (ID_INDICATOR, Msg::CheckIndicator),
        (ID_BUBBLE, Msg::CheckBubble),
        (ID_PREVIEW, Msg::MenuPreview),
        (ID_NOTE_PREVIEW, Msg::NotePreview),
        (ID_SOUND, Msg::CheckSound),
        (ID_LBL_TRY, Msg::LabelText),
        (ID_COPY_RESULT, Msg::ButtonCopyResult),
        (ID_LBL_HOTKEY, Msg::LabelHotkeyLayout),
        (ID_NOTE_HOTKEY, Msg::NoteHotkey),
        (ID_LBL_HOTKEY_CASE, Msg::LabelHotkeyCase),
        (ID_NOTE_HOTKEY_CASE, Msg::NoteHotkeyCase),
        (ID_SWITCH, Msg::CheckSwitchLanguage),
        (ID_NOTE_SWITCH, Msg::NoteSwitch),
        (ID_CAREFUL, Msg::CheckCareful),
        (ID_NOTE_CAREFUL, Msg::NoteCareful),
        (ID_TYPING, Msg::MenuTyping),
        (ID_NOTE_TYPING_ON, Msg::NoteTypingOn),
        (ID_TYPING_WORDS, Msg::MenuTypingWords),
        (ID_NOTE_WORDS, Msg::NoteWords),
        (ID_REPLACEMENTS, Msg::MenuReplacements),
        (ID_NOTE_SHORTCUTS, Msg::NoteShortcuts),
        (ID_REPLACEMENTS_FILE, Msg::MenuReplacementsFile),
        (ID_LBL_RULE_OFF, Msg::LabelRulesDisabled),
        (ID_NOTE_RULE_OFF, Msg::NoteRuleOff),
        (ID_LBL_RULE_SEL, Msg::LabelRulesSelection),
        (ID_NOTE_RULE_SEL, Msg::NoteRuleSel),
        (ID_NOTE_RULES2, Msg::NoteRulesHow),
        (ID_ONE_LANGUAGE, Msg::MenuOneLanguage),
        (ID_NOTE_ONE_LANGUAGE, Msg::NoteOneLanguage),
        (ID_ONE_LANGUAGE_FILE, Msg::MenuOneLanguageFile),
        (ID_LBL_DIRECTION, Msg::LabelDirection),
        (ID_F10_WRONG, Msg::MenuF10WrongWords),
        (ID_CTRL_TAP, Msg::MenuCtrlTap),
        (ID_TERMINAL, Msg::CheckTerminal),
        (ID_TYPING_SOUND, Msg::MenuTypingSound),
        (ID_MESSENGER, Msg::MenuMessengerGuard),
        (ID_LBL_LIMIT, Msg::LabelLimit),
        (ID_LBL_DELAY, Msg::LabelDelay),
        (ID_EXPORT, Msg::ButtonExport),
        (ID_IMPORT, Msg::ButtonImport),
        (ID_PRIVACY, Msg::PrivacyLine),
        (ID_DIAG_COPY, Msg::ButtonCopyDiag),
        (ID_SHOW_REPORT, if REPORT_SHOWN.with(|r| r.get()) { Msg::ButtonHideReport } else { Msg::ButtonShowReport }),
        (ID_REPORT_PROBLEM, Msg::MenuReportProblem),
        (ID_REPORT_SEND, Msg::ButtonSendReport),
        (ID_LBL_REPORTS, Msg::LabelReports),
        (ID_CHECK_UPDATE, Msg::ButtonCheckUpdate),
        (ID_AUTO_UPDATE, Msg::MenuCheckUpdates),
        (ID_UNDO_IMPORT, Msg::ButtonUndoImport),
        (ID_DEFAULTS, Msg::ButtonDefaults),
        (ID_SAVE, Msg::ButtonSave),
        (ID_CLOSE, Msg::ButtonClose),
    ] {
        set_text(id, text(l, m));
    }
    // the texts that name the key really chosen (F10 or another)
    for (id, m) in [(ID_DOUBLE_SHIFT, Msg::MenuDoubleShift), (ID_NOTE_MESSENGER, Msg::NoteMessenger), (ID_NOTE_LIMIT, Msg::NoteLimitDelay)] {
        set_text(id, &text_with_key(l, m, &key));
    }
    // Help: version, independent project, license (one text, read by Tab)
    set_text(ID_ABOUT_TEXT, &text(l, Msg::AboutText).replace("{version}", klf_core::VERSION).replace('\n', "\r\n"));
    set_text(ID_LAYOUTS, &layouts_line(l));
    refresh_advanced_head();
    fill_work_modes(l);
    for (combo, items) in [
        (ID_DIRECTION, [Msg::DirectionAuto, Msg::DirectionToHebrew, Msg::DirectionToEnglish]),
        (ID_BUBBLE_POS, [Msg::BubbleTop, Msg::BubbleBottom, Msg::BubbleCaret]),
        (ID_REPORT_MODE, [Msg::ReportModeAsk, Msg::ReportModeAlways, Msg::ReportModeNever]),
    ] {
        for m in items {
            let w = wide(text(l, m));
            send(combo, CB_ADDSTRING, 0, w.as_ptr() as isize);
        }
    }
    send(ID_DIRECTION, CB_SETCURSEL, s.mixed_text_mode.as_number() as usize, 0);
    send(ID_BUBBLE_POS, CB_SETCURSEL, s.bubble_position.min(klf_core::settings::BUBBLE_CARET) as usize, 0);
    for t in [text(l, Msg::LanguageAuto), "עברית", "Français", "English"] {
        let w = wide(t);
        send(ID_LANG, CB_ADDSTRING, 0, w.as_ptr() as isize);
    }
    send(ID_LANG, CB_SETCURSEL, lang_index(s.ui_language), 0);
    send(ID_HOTKEY, HKM_SETRULES, 0, 0); // F1-F11 alone allowed (F12 refused by validate)
    send(ID_HOTKEY_CASE, HKM_SETRULES, 0, 0);
    set_box(ID_HOTKEY, s.hotkey_modifiers, s.hotkey_key);
    set_box(ID_HOTKEY_CASE, s.case_hotkey_modifiers, s.case_hotkey_key);
    set_checked(ID_SWITCH, s.switch_language_after_convert);
    set_checked(ID_BUBBLE, s.show_bubble);
    set_checked(ID_PREVIEW, s.show_preview);
    set_checked(ID_SOUND, s.play_sound);
    set_checked(ID_CAREFUL, s.require_selection);
    set_checked(ID_TERMINAL, s.terminal_support);
    set_checked(ID_INDICATOR, s.indicator);
    set_text(ID_LIMIT, &s.max_character_limit.to_string());
    set_text(ID_DELAY, &klf_core::settings::delay_text(s.paste_delay_ms, l == Lang::Fr));
    // only the user's own lists (the administrator's programs and rules are added on top, never
    // saved here; 2.0.0: the « Excluded » list of older versions is in the first list now)
    set_text(ID_RULE_OFF, &rules_text(&u, RuleMode::Disabled));
    set_text(ID_RULE_SEL, &rules_text(&u, RuleMode::SelectionOnly));
    // public per-user install only (as the menu before 2.0.0): shown greyed and unticked elsewhere
    let typing = crate::typing::allowed();
    for (id, on) in [
        (ID_F10_WRONG, s.f10_only_wrong_words),
        (ID_DOUBLE_SHIFT, s.double_shift_convert),
        (ID_CTRL_TAP, s.ctrl_tap_language),
        (ID_TYPING, s.typing_suggestions),
        (ID_TYPING_SOUND, s.typing_sound),
        (ID_MESSENGER, s.messenger_guard),
        (ID_REPLACEMENTS, s.text_replacements),
        (ID_ONE_LANGUAGE, s.one_language_programs),
    ] {
        set_checked(id, on && typing);
        enable(id, typing);
    }
    enable(ID_ONE_LANGUAGE_FILE, typing);
    set_text(ID_NOTE_TYPING, if typing { "" } else { text(l, Msg::NoteNotHere) });
    let reports = crate::report::enabled();
    send(ID_REPORT_MODE, CB_SETCURSEL, if reports { report_index(s.report_mode) } else { 0 }, 0);
    for id in [ID_REPORT_PROBLEM, ID_REPORT_MODE] {
        enable(id, reports);
    }
    let updates = crate::update::allowed(true);
    set_checked(ID_AUTO_UPDATE, updates && s.check_updates);
    // settings imposed by the administrator: shown, greyed out, with a small lock
    let lk = app::locked();
    enable(ID_AUTO_UPDATE, updates && !lk.check_updates);
    for (id, locked, mark) in [
        (ID_HOTKEY, lk.hotkey, ID_LBL_HOTKEY),
        (ID_HOTKEY_CASE, lk.case_hotkey, ID_LBL_HOTKEY_CASE),
        (ID_DIRECTION, lk.mixed_text_mode, ID_LBL_DIRECTION),
        (ID_SWITCH, lk.switch_language_after_convert, ID_SWITCH),
        (ID_BUBBLE, lk.show_bubble, ID_BUBBLE),
        (ID_BUBBLE_POS, lk.bubble_position, ID_BUBBLE),
        (ID_PREVIEW, lk.show_preview, ID_PREVIEW),
        (ID_SOUND, lk.play_sound, ID_SOUND),
        (ID_CAREFUL, lk.require_selection, ID_CAREFUL),
        (ID_LIMIT, lk.max_character_limit, ID_LBL_LIMIT),
        (ID_DELAY, lk.paste_delay_ms, ID_LBL_DELAY),
        (ID_LANG, lk.ui_language, ID_LBL_LANG),
        (ID_INDICATOR, lk.indicator, ID_INDICATOR),
        (ID_AUTO_UPDATE, lk.check_updates && updates, ID_AUTO_UPDATE),
    ] {
        if locked {
            enable(id, false);
            add_lock(mark);
        }
    }
    update_dependents();
    let managed = sys::autostart_managed();
    MANAGED.with(|c| c.set(managed));
    if managed {
        set_text(ID_AUTOSTART, text(l, Msg::AutostartManaged));
        enable(ID_AUTOSTART, false);
    } else {
        set_text(ID_AUTOSTART, text(l, Msg::CheckAutostart));
        set_checked(ID_AUTOSTART, sys::autostart_user());
    }
    refresh_status();
}

/// 2.0.0 « Defaults »: the default values of the page shown, put in its controls only (saved by
/// « Apply »). Never the lists (words, shortcuts, rules, one-language
/// programs), never a setting imposed by the administrator, never the pause state nor the start
/// with Windows (a state of Windows, set by the installer).
fn on_defaults() {
    cancel_language_timer();
    let d = AppSettings::default();
    let lk = app::locked();
    let typing = crate::typing::allowed();
    let check = |id: i32, locked: bool, on: bool| {
        if !locked {
            set_checked(id, on);
        }
    };
    match PAGE.with(|p| p.get()) {
        0 => {
            if !lk.ui_language {
                send(ID_LANG, CB_SETCURSEL, lang_index(d.ui_language), 0);
            }
            check(ID_INDICATOR, lk.indicator, d.indicator);
            check(ID_BUBBLE, lk.show_bubble, d.show_bubble);
            if !lk.bubble_position {
                send(ID_BUBBLE_POS, CB_SETCURSEL, d.bubble_position as usize, 0);
            }
            check(ID_PREVIEW, lk.show_preview, d.show_preview);
            check(ID_SOUND, lk.play_sound, d.play_sound);
        }
        1 => {
            if !lk.hotkey {
                set_box(ID_HOTKEY, d.hotkey_modifiers, d.hotkey_key);
            }
            if !lk.case_hotkey {
                set_box(ID_HOTKEY_CASE, d.case_hotkey_modifiers, d.case_hotkey_key);
            }
            check_hotkey_box(ID_HOTKEY);
            check_hotkey_box(ID_HOTKEY_CASE);
            check(ID_SWITCH, lk.switch_language_after_convert, d.switch_language_after_convert);
            check(ID_CAREFUL, lk.require_selection, d.require_selection);
        }
        2 => {
            check(ID_TYPING, !typing, d.typing_suggestions);
            check(ID_REPLACEMENTS, !typing, d.text_replacements);
        }
        3 => {
            check(ID_ONE_LANGUAGE, !typing, d.one_language_programs);
        }
        PAGE_ADV => {
            if !lk.mixed_text_mode {
                send(ID_DIRECTION, CB_SETCURSEL, d.mixed_text_mode.as_number() as usize, 0);
            }
            check(ID_F10_WRONG, !typing, d.f10_only_wrong_words);
            check(ID_DOUBLE_SHIFT, !typing, d.double_shift_convert);
            check(ID_CTRL_TAP, !typing, d.ctrl_tap_language);
            check(ID_TERMINAL, school_pc(), d.terminal_support);
            check(ID_TYPING_SOUND, !typing, d.typing_sound);
            check(ID_MESSENGER, !typing, d.messenger_guard);
            if !lk.max_character_limit {
                set_text(ID_LIMIT, &d.max_character_limit.to_string());
            }
            if !lk.paste_delay_ms {
                set_text(ID_DELAY, &klf_core::settings::delay_text(d.paste_delay_ms, app::lang() == Lang::Fr));
            }
        }
        _ => {}
    }
    update_dependents();
    update_try();
}

/// Reads the controls; None (with a message) if a value is not allowed.
/// Builds the USER settings to save: a setting imposed by the administrator keeps the user's own value
/// (the administrator's value is never written into settings.json).
fn read_controls() -> Option<AppSettings> {
    let old = app::user_settings();
    let lk = app::locked();
    let mut s = old.clone();
    let l = app::lang();
    let hk = send(ID_HOTKEY, HKM_GETHOTKEY, 0, 0) as u32;
    let vk = hk & 0xFF;
    // the control cannot show Win: kept if nothing else changed
    let mods = klf_core::settings::keep_win(old.hotkey_modifiers, old.hotkey_key, vk, hotkeyf_to_mods((hk >> 8) & 0xFF));
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
    let vk = hk & 0xFF;
    let mods = klf_core::settings::keep_win(old.case_hotkey_modifiers, old.case_hotkey_key, vk, hotkeyf_to_mods((hk >> 8) & 0xFF));
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
    // 2.0.0: the delay is typed in seconds (0.1 to 10), kept in ms
    let delay: Option<i64> = if lk.paste_delay_ms { Some(old.paste_delay_ms) } else { klf_core::settings::parse_delay_seconds(&get_text(ID_DELAY)) };
    let (Some(limit), Some(delay)) = (limit, delay) else {
        set_error(text(l, Msg::NumberInvalid));
        return None;
    };
    s.max_character_limit = limit;
    s.paste_delay_ms = delay;
    // 2.0.0: the rules of the tray menu, as two lists (a program in both: disabled)
    s.app_rules = klf_core::settings::rules_from_lists(&get_text(ID_RULE_OFF), &get_text(ID_RULE_SEL));
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
    s.show_preview = keep(lk.show_preview, old.show_preview, ID_PREVIEW);
    s.play_sound = keep(lk.play_sound, old.play_sound, ID_SOUND);
    s.require_selection = keep(lk.require_selection, old.require_selection, ID_CAREFUL);
    // 1.9.5: hidden (and forced off) on a PC managed by an administrator
    s.terminal_support = keep(school_pc(), old.terminal_support, ID_TERMINAL);
    s.indicator = keep(lk.indicator, old.indicator, ID_INDICATOR);
    // public per-user install only: elsewhere the user's values stay as they are
    let no_typing = !crate::typing::allowed();
    s.f10_only_wrong_words = keep(no_typing, old.f10_only_wrong_words, ID_F10_WRONG);
    s.double_shift_convert = keep(no_typing, old.double_shift_convert, ID_DOUBLE_SHIFT);
    s.ctrl_tap_language = keep(no_typing, old.ctrl_tap_language, ID_CTRL_TAP);
    s.typing_suggestions = keep(no_typing, old.typing_suggestions, ID_TYPING);
    s.typing_sound = keep(no_typing, old.typing_sound, ID_TYPING_SOUND);
    s.messenger_guard = keep(no_typing, old.messenger_guard, ID_MESSENGER);
    s.text_replacements = keep(no_typing, old.text_replacements, ID_REPLACEMENTS);
    s.one_language_programs = keep(no_typing, old.one_language_programs, ID_ONE_LANGUAGE);
    s.check_updates = keep(!crate::update::allowed(true) || lk.check_updates, old.check_updates, ID_AUTO_UPDATE);
    if crate::report::enabled() {
        s.report_mode = match send(ID_REPORT_MODE, CB_GETCURSEL, 0, 0) {
            1 => klf_core::report::ALWAYS,
            2 => klf_core::report::NEVER,
            _ => klf_core::report::ASK,
        };
    }
    if !lk.ui_language {
        s.ui_language = lang_of_index(send(ID_LANG, CB_GETCURSEL, 0, 0));
    }
    s.validate();
    Some(s)
}

fn on_save() -> bool {
    let Some(new_user) = read_controls() else { return false };
    let old = app::settings();
    // the keys and the language in force after saving (the administrator's settings stay on top)
    let new = klf_core::school::effective(&new_user, &app::policy()).0;
    // a new hotkey is tried BEFORE the old one is given up (audit 1.3.2)
    // a combination we already hold (F9 and F10 exchanged) is not "busy"
    let ours = |m: u32, k: u32| (m, k) == (old.hotkey_modifiers, old.hotkey_key) || (m, k) == (old.case_hotkey_modifiers, old.case_hotkey_key);
    let key_changed = (new.hotkey_modifiers, new.hotkey_key) != (old.hotkey_modifiers, old.hotkey_key);
    if key_changed && !ours(new.hotkey_modifiers, new.hotkey_key) && !app::hotkey_free(new.hotkey_modifiers, new.hotkey_key) {
        set_error(&text_with_key(app::lang(), Msg::HotkeyBusy, &new.hotkey_description()));
        return false;
    }
    let case_changed = (new.case_hotkey_modifiers, new.case_hotkey_key) != (old.case_hotkey_modifiers, old.case_hotkey_key);
    if case_changed && new.case_hotkey_key != KEY_NONE && !ours(new.case_hotkey_modifiers, new.case_hotkey_key) && !app::hotkey_free(new.case_hotkey_modifiers, new.case_hotkey_key) {
        set_error(&text_with_key(app::lang(), Msg::HotkeyBusy, &new.case_hotkey_description()));
        return false;
    }
    // settings.json first: if it cannot be written, NOTHING is changed (registry included)
    if !app::save_settings(&new_user) {
        set_error(text(app::lang(), Msg::SaveFailed));
        return false;
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
    let report_mode = new_user.report_mode;
    app::apply_settings(new_user);
    crate::report::mode_saved(report_mode);
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
        // the two rule lists are applied: a rule changed later from the menu is shown again there
        for id in [ID_RULE_OFF, ID_RULE_SEL] {
            send(id, EM_SETMODIFY, 0, 0);
        }
        refresh_rules();
        refresh_advanced_head();
        refresh_status();
    }
    relang
}

/// 2.0.0: a language picked in the list is applied at once (with the rest of the page, as
/// « Apply »: the window is made again in the new direction, on the same tab, the language list
/// keeping the focus). Nothing if it is the language already in force.
fn apply_language_now() {
    LANG_PENDING.with(|p| p.set(false));
    if window().is_null() || app::locked().ui_language {
        return;
    }
    // a dialog of ours is open (import, export, message): the window is never destroyed under its
    // modal loop; tried again later
    // SAFETY: our window.
    if unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(window()) } == 0 {
        // SAFETY: our window.
        unsafe { SetTimer(window(), TIMER_LANG, 800, None) };
        return;
    }
    if lang_of_index(send(ID_LANG, CB_GETCURSEL, 0, 0)) == app::user_settings().ui_language {
        return;
    }
    if on_save() && PAGE.with(|p| p.get()) == 0 {
        // SAFETY: our (new) window's control, on the page shown.
        unsafe { SetFocus(item(ID_LANG)) };
    }
}

/// 2.0.0: a language waiting to be applied is forgotten (another page, « Defaults »).
fn cancel_language_timer() {
    LANG_PENDING.with(|p| p.set(false));
    if !window().is_null() {
        // SAFETY: our window.
        unsafe { KillTimer(window(), TIMER_LANG) };
    }
}

/// The administrator's settings changed while the window is open: shown again, same tab.
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
        .ok_or(klf_core::profile::ImportError::TooBig)
        .and_then(|t| klf_core::profile::import(&t));
    let new = match read {
        Ok(s) => s,
        Err(why) => {
            set_error(&format!("{} {}", text(l, Msg::ImportFailed), text(l, why.msg())));
            return;
        }
    };
    let old = app::user_settings();
    // a file never turns on "send reports without asking": that choice stays the user's
    let mut new = new;
    new.report_mode = old.report_mode;
    new.typing_suggestions = old.typing_suggestions; // watching the keys is the user's own choice
    new.double_shift_convert = old.double_shift_convert;
    new.typing_sound = old.typing_sound;
    new.f10_only_wrong_words = old.f10_only_wrong_words; // downloads the word lists: the user's choice
    new.ctrl_tap_language = old.ctrl_tap_language; // these watch the keys: the user's own choices
    new.messenger_guard = old.messenger_guard;
    new.one_language_programs = old.one_language_programs;
    new.text_replacements = old.text_replacements; // also watches the keys: the user's own choice
    new.typing_skip_code_editors = old.typing_skip_code_editors;
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

/// 2.0.0: the two links (click or Enter): only their fixed address, never the text of the control.
/// Opened on a short thread of its own: ShellExecute can wait for the browser, and the interface
/// thread carries the keyboard hooks.
fn open_link(url: &'static str) {
    let started = std::thread::Builder::new().name("klf-link".into()).spawn(move || {
        use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        let (verb, url) = (wide("open"), wide(url));
        // SAFETY: COM for this thread only (the Shell needs it), released before the thread ends;
        // valid strings; no owner window (it belongs to the interface thread), no parameters.
        unsafe {
            let com = CoInitializeEx(null(), (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32) >= 0;
            ShellExecuteW(null_mut(), verb.as_ptr(), url.as_ptr(), null(), null(), SW_SHOWNORMAL);
            if com {
                CoUninitialize();
            }
        }
    });
    if started.is_err() {
        log("Link: thread not started");
    }
}

/// Enter with the focus on a link opens the link (NM_RETURN), never « Apply » too.
fn focus_on_link() -> bool {
    // SAFETY: GetFocus for our own thread.
    let f = unsafe { GetFocus() };
    !f.is_null() && (f == item(ID_GITHUB) || f == item(ID_ORIGIN))
}

/// 2.0.0: the technical report of the Help tab, shown or folded away again.
fn toggle_report() {
    let shown = !REPORT_SHOWN.with(|r| r.get());
    REPORT_SHOWN.with(|r| r.set(shown));
    let l = app::lang();
    set_text(ID_SHOW_REPORT, text(l, if shown { Msg::ButtonHideReport } else { Msg::ButtonShowReport }));
    show_page(PAGE.with(|p| p.get()));
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as i32;
            let code = ((wparam >> 16) & 0xFFFF) as u32;
            match (id, code) {
                // 1 = IDOK (Enter); Enter on the GitHub link is the link's own (NM_RETURN)
                (ID_SAVE, _) | (1, _) if !focus_on_link() => {
                    on_save();
                }
                (ID_CLOSE, _) => close(),
                (ID_DEFAULTS, BN_CLICKED) => on_defaults(),
                // 2.0.0: the pause state, at once (not saved: the program starts active)
                (ID_ENABLED, BN_CLICKED) => app::set_active(checked(ID_ENABLED)),
                // 2.0.0: the language at once; list open: when it closes; list closed (arrows):
                // after a short pause
                (ID_LANG, CBN_SELCHANGE) => {
                    if send(ID_LANG, CB_GETDROPPEDSTATE, 0, 0) != 0 {
                        LANG_PENDING.with(|p| p.set(true));
                    } else {
                        // SAFETY: our window.
                        unsafe { SetTimer(hwnd, TIMER_LANG, 800, None) };
                    }
                }
                (ID_LANG, CBN_CLOSEUP) if LANG_PENDING.with(|p| p.get()) => {
                    // never destroy the list from its own notification: posted
                    // SAFETY: posting to our own window.
                    unsafe { PostMessageW(hwnd, WM_APP_LANG, 0, 0) };
                }
                (ID_TRY, EN_CHANGE) => update_try(),
                (ID_WORK_MODE, CBN_SELCHANGE) | (ID_DIRECTION, CBN_SELCHANGE) => update_try(),
                // the position list and the preview only matter when the bubble is shown
                (ID_BUBBLE, BN_CLICKED) | (ID_TYPING, BN_CLICKED) | (ID_REPLACEMENTS, BN_CLICKED) => update_dependents(),
                // 2.0.0: a key another program holds is said at once
                (ID_HOTKEY, EN_CHANGE) | (ID_HOTKEY_CASE, EN_CHANGE) => check_hotkey_box(id),
                (ID_SHOW_REPORT, BN_CLICKED) => toggle_report(),
                (ID_REPORT_PROBLEM, BN_CLICKED) if crate::report::enabled() => crate::report::open(),
                (ID_REPORT_SEND, BN_CLICKED) if crate::report::enabled() => {
                    crate::report::send_pending(app::main_window());
                    refresh_diagnostic();
                }
                (ID_TYPING_WORDS, BN_CLICKED) if crate::typing::allowed() => crate::typing::open_personal(),
                (ID_REPLACEMENTS_FILE, BN_CLICKED) if crate::typing::allowed() => crate::typing::open_replacements(),
                (ID_ONE_LANGUAGE_FILE, BN_CLICKED) if crate::typing::allowed() => crate::typing::open_one_language(),
                (ID_COPY_RESULT, _) => copy_text(&get_text(ID_RESULT)),
                (ID_EXPORT, _) => on_export(),
                (ID_IMPORT, _) => on_import(),
                (ID_UNDO_IMPORT, _) => on_undo_import(),
                (ID_CHECK_UPDATE, BN_CLICKED) => app::check_updates_now(),
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
        WM_TIMER if wparam == TIMER_LANG => {
            // SAFETY: our window.
            unsafe { KillTimer(hwnd, TIMER_LANG) };
            apply_language_now();
            0
        }
        m if m == WM_APP_LANG => {
            apply_language_now();
            0
        }
        WM_ENABLE if wparam != 0 && PENDING_REFRESH.with(|p| p.get()) => {
            // the dialog closed: the administrator's settings changed meanwhile
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
            } else if nm.code == NM_CLICK || nm.code == NM_RETURN {
                if nm.idFrom == ID_GITHUB as usize && nm.hwndFrom == item(ID_GITHUB) {
                    open_link(GITHUB_URL);
                } else if nm.idFrom == ID_ORIGIN as usize && nm.hwndFrom == item(ID_ORIGIN) {
                    open_link(ORIGIN_URL);
                }
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
                } else if child == item(ID_ABOUT_TEXT) {
                    // read-only edit: opaque, on the window's own background
                    SetTextColor(dc, GetSysColor(COLOR_WINDOWTEXT));
                    SetBkMode(dc, OPAQUE as i32);
                    SetBkColor(dc, GetSysColor(COLOR_WINDOW));
                    return GetSysColorBrush(COLOR_WINDOW) as LRESULT;
                } else if !sys::high_contrast() && ((child == item(ID_NOTE_HOTKEY) && HOTKEY_WARN.with(|w| w.get()[0])) || (child == item(ID_NOTE_HOTKEY_CASE) && HOTKEY_WARN.with(|w| w.get()[1]))) {
                    // 2.0.0: « this key is already used by another program », in red
                    SetTextColor(dc, 0x002020C0);
                } else if NOTE_IDS.iter().any(|id| child == item(*id)) && !sys::high_contrast() {
                    // 2.0.0: the explanation lines in grey (normal text colour in high contrast)
                    SetTextColor(dc, GetSysColor(COLOR_GRAYTEXT));
                } else {
                    SetTextColor(dc, GetSysColor(COLOR_WINDOWTEXT));
                }
            }
            BRUSH.with(|b| b.get()) as LRESULT
        }
        WM_DPICHANGED => {
            let dpi = (((wparam >> 16) & 0xFFFF) as u32).clamp(96, 480);
            let old = DPI.with(|d| d.replace(dpi)).max(96);
            make_fonts(dpi);
            // 1.9.0: the new size is worked out here from the current one (the rectangle passed in
            // lparam is never read: a message posted by another program could carry anything)
            // SAFETY: our window; the RECT is ours.
            unsafe {
                let mut r: RECT = std::mem::zeroed();
                if GetWindowRect(hwnd, &mut r) != 0 {
                    let w = (r.right - r.left) * dpi as i32 / old as i32;
                    let h = (r.bottom - r.top) * dpi as i32 / old as i32;
                    SetWindowPos(hwnd, null_mut(), r.left, r.top, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
                }
            }
            let _ = lparam;
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
            // SAFETY: our window (a timer of a destroyed window is gone anyway).
            unsafe { KillTimer(hwnd, TIMER_LANG) };
            LANG_PENDING.with(|p| p.set(false));
            clear_acc_names();
            HWND_.with(|c| c.set(null_mut()));
            0
        }
        // SAFETY: default handling.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// 2.0.0: the window opened on one tab (« About » in the tray menu).
pub fn open_on(page: usize) {
    open();
    if !window().is_null() {
        show_page(page);
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
    // the administrator's settings may have changed (an administration tool writes them without a notice)
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
            ex |= WS_EX_LAYOUTRTL | WS_EX_NOINHERITLAYOUT;
        }
        MIRRORED.with(|m| m.set(l.is_rtl()));
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
        let hwnd = CreateWindowExW(ex, class.as_ptr(), title.as_ptr(), style, CW_USEDEFAULT, CW_USEDEFAULT, 100, 100, null_mut(), null_mut(), app::instance(), null());
        if hwnd.is_null() {
            log("Settings window could not be created");
            return;
        }
        HWND_.with(|c| c.set(hwnd));
        // 1.9.0: KLF_TEST_DPI (120 to 288) draws the window as at 125 % to 300 %: only for the visual
        // test on GitHub's Windows, where the screen scale cannot be changed without a new logon
        let test_dpi = std::env::var("KLF_TEST_DPI").ok().and_then(|v| v.parse::<u32>().ok()).filter(|d| (120..=288).contains(d));
        let dpi = test_dpi.unwrap_or_else(|| GetDpiForWindow(hwnd).max(96));
        DPI.with(|d| d.set(dpi));
        make_fonts(dpi);

        // creation order = Tab order (children are added at the bottom of the Z order); a label
        // is created just before its field (a screen reader names the field after it); a grey
        // line never just before a field
        let stat = SS_LEFT | SS_CENTERIMAGE | SS_ENDELLIPSIS;
        let note = SS_LEFT | SS_EDITCONTROL | SS_NOPREFIX;
        let check = BS_AUTOCHECKBOX | WS_TABSTOP;
        let button = BS_PUSHBUTTON | WS_TABSTOP;
        let combo = CBS_DROPDOWNLIST | WS_TABSTOP | WS_VSCROLL;
        create("STATIC", "", stat, 0, ID_STATUS);
        // General
        create("STATIC", "", stat, 0, ID_LBL_LANG);
        create("COMBOBOX", "", combo, 0, ID_LANG);
        create("BUTTON", "", check, 0, ID_ENABLED);
        create("STATIC", "", note, 0, ID_NOTE_ENABLED);
        create("BUTTON", "", check, 0, ID_AUTOSTART);
        create("BUTTON", "", check, 0, ID_INDICATOR);
        create("BUTTON", "", check, 0, ID_BUBBLE);
        create("COMBOBOX", "", combo, 0, ID_BUBBLE_POS); // named « Where to show the message »
        create("BUTTON", "", check, 0, ID_PREVIEW);
        create("STATIC", "", note, 0, ID_NOTE_PREVIEW);
        create("BUTTON", "", check, 0, ID_SOUND);
        create("COMBOBOX", "", combo, 0, ID_WORK_MODE); // named « Conversion direction »
        create("STATIC", "", SS_LEFT | SS_CENTERIMAGE | SS_ENDELLIPSIS, 0, ID_LBL_TRY);
        create("EDIT", "", ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN | WS_VSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_TRY);
        create("EDIT", "", ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY | WS_VSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_RESULT); // named « Result »
        create("BUTTON", "", button, 0, ID_COPY_RESULT);
        // Keys
        create("STATIC", "", stat, 0, ID_LBL_HOTKEY);
        create("msctls_hotkey32", "", WS_TABSTOP | WS_BORDER, 0, ID_HOTKEY);
        create("STATIC", "", note, 0, ID_NOTE_HOTKEY);
        create("STATIC", "", stat, 0, ID_LBL_HOTKEY_CASE);
        create("msctls_hotkey32", "", WS_TABSTOP | WS_BORDER, 0, ID_HOTKEY_CASE);
        create("STATIC", "", note, 0, ID_NOTE_HOTKEY_CASE);
        create("BUTTON", "", check, 0, ID_SWITCH);
        create("STATIC", "", note, 0, ID_NOTE_SWITCH);
        create("BUTTON", "", check, 0, ID_CAREFUL);
        create("STATIC", "", note, 0, ID_NOTE_CAREFUL);
        // While typing
        create("BUTTON", "", check, 0, ID_TYPING);
        create("STATIC", "", note, 0, ID_NOTE_TYPING_ON);
        create("BUTTON", "", button, 0, ID_TYPING_WORDS);
        create("STATIC", "", note, 0, ID_NOTE_WORDS);
        create("BUTTON", "", check, 0, ID_REPLACEMENTS);
        create("STATIC", "", note, 0, ID_NOTE_SHORTCUTS);
        create("BUTTON", "", button, 0, ID_REPLACEMENTS_FILE);
        create("STATIC", "", note, 0, ID_NOTE_TYPING);
        // Programs
        create("STATIC", "", stat, 0, ID_LBL_RULE_OFF);
        create("EDIT", "", ES_AUTOHSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_RULE_OFF);
        create("STATIC", "", note, 0, ID_NOTE_RULE_OFF);
        create("STATIC", "", stat, 0, ID_LBL_RULE_SEL);
        create("EDIT", "", ES_AUTOHSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_RULE_SEL);
        create("STATIC", "", note, 0, ID_NOTE_RULE_SEL);
        create("STATIC", "", note, 0, ID_NOTE_RULES2);
        create("BUTTON", "", check, 0, ID_ONE_LANGUAGE);
        create("STATIC", "", note, 0, ID_NOTE_ONE_LANGUAGE);
        create("BUTTON", "", button, 0, ID_ONE_LANGUAGE_FILE);
        // Advanced
        create("STATIC", "", note, 0, ID_ADV_HEAD);
        create("STATIC", "", stat, 0, ID_LBL_DIRECTION);
        create("COMBOBOX", "", combo, 0, ID_DIRECTION);
        for id in [ID_F10_WRONG, ID_DOUBLE_SHIFT, ID_CTRL_TAP, ID_TERMINAL, ID_TYPING_SOUND, ID_MESSENGER] {
            create("BUTTON", "", check, 0, id);
        }
        create("STATIC", "", note, 0, ID_NOTE_MESSENGER);
        create("STATIC", "", stat, 0, ID_LBL_LIMIT);
        create("EDIT", "", ES_NUMBER | ES_AUTOHSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_LIMIT);
        create("STATIC", "", stat, 0, ID_LBL_DELAY);
        // seconds, with a decimal point or comma (2.0.0): not ES_NUMBER
        create("EDIT", "", ES_AUTOHSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_DELAY);
        create("STATIC", "", note, 0, ID_NOTE_LIMIT);
        create("BUTTON", "", button, 0, ID_EXPORT);
        create("BUTTON", "", button, 0, ID_IMPORT);
        // Help
        create("STATIC", "", note, 0, ID_PRIVACY);
        create("STATIC", "", SS_LEFT | SS_NOPREFIX, 0, ID_DIAG_LAST);
        create("STATIC", "", SS_LEFT | SS_EDITCONTROL | SS_NOPREFIX, 0, ID_LAYOUTS);
        create("BUTTON", "", button, 0, ID_DIAG_COPY);
        create("BUTTON", "", button, 0, ID_SHOW_REPORT);
        create("EDIT", "", ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY | WS_VSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_DIAG_TEXT); // named « Technical report »
        create("BUTTON", "", button, 0, ID_REPORT_PROBLEM);
        create("BUTTON", "", button, 0, ID_REPORT_SEND);
        create("STATIC", "", stat, 0, ID_LBL_REPORTS);
        create("COMBOBOX", "", combo, 0, ID_REPORT_MODE);
        create("BUTTON", "", button, 0, ID_CHECK_UPDATE);
        create("BUTTON", "", check, 0, ID_AUTO_UPDATE);
        create("BUTTON", "", button, 0, ID_UNDO_IMPORT);
        // a read-only edit, reached by Tab: a screen reader reads the version and the license
        // (a static never gets the focus); no border, the window's background
        create("EDIT", "", ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY | WS_TABSTOP, 0, ID_ABOUT_TEXT);
        for (id, url, m) in [(ID_GITHUB, GITHUB_URL, Msg::LinkOurs), (ID_ORIGIN, ORIGIN_URL, Msg::LinkOrigin)] {
            let link = format!("<a href=\"{url}\">{}</a>", text(l, m));
            create("SysLink", &link, WS_TABSTOP | LWS_TRANSPARENT, 0, id);
        }
        // common bottom row
        create("BUTTON", "", button, 0, ID_DEFAULTS);
        create("BUTTON", "", BS_DEFPUSHBUTTON | WS_TABSTOP, 0, ID_SAVE);
        create("BUTTON", "", button, 0, ID_CLOSE);
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
        show_page(0); // the grey lines hidden on a small screen
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
