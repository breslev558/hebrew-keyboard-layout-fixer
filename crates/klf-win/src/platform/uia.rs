//! 1.8.7: is the focused element a password field? Asked through UI Automation (browsers and
//! Electron apps do not mark their password boxes with ES_PASSWORD). Only on a worker thread of
//! its own (COM, multithreaded), never in the keyboard hook, with a time limit: an answer that
//! does not come in time counts as "maybe a password" (nothing is proposed).

use std::ffi::c_void;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};
use windows_sys::core::GUID;
use windows_sys::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};

const CLSID_CUIAUTOMATION: GUID = GUID::from_u128(0xff48dba4_60ef_4201_aa87_54103eef594e);
const IID_IUIAUTOMATION: GUID = GUID::from_u128(0x30cbe57d_d9d0_452a_ab13_7ac5ac4825ee);
/// UIA_IsPasswordPropertyId
const IS_PASSWORD: i32 = 30019;
const VT_BOOL: u16 = 11;
const VT_I4: u16 = 3;
/// UIA_ControlTypePropertyId, and the Pane / Window control types
const CONTROL_TYPE: i32 = 30003;
const PANE: i32 = 50033;
const WINDOW: i32 = 50032;
/// 1.9.2: the Edit and Document control types (a text field, a page or a rich editor)
const EDIT: i32 = 50004;
const DOCUMENT: i32 = 50030;
/// UIA_TextPatternId and IID_IUIAutomationTextPattern
const TEXT_PATTERN: i32 = 10014;
const IID_TEXT_PATTERN: GUID = GUID::from_u128(0x32eba289_3583_42c9_9c59_3b6d9a1e9b6a);

/// Frees what a VARIANT may hold (a text, an object...).
fn clear(v: &mut [u64; 3]) {
    // SAFETY: a VARIANT filled by UI Automation (or zeroed), 24 bytes.
    unsafe { windows_sys::Win32::System::Variant::VariantClear(v.as_mut_ptr() as *mut _) };
}

type Release = unsafe extern "system" fn(*mut c_void) -> u32;
type GetFocused = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
type GetProperty = unsafe extern "system" fn(*mut c_void, i32, *mut [u64; 3]) -> i32;

/// The method `index` of a COM interface pointer.
unsafe fn method<T: Copy>(obj: *mut c_void, index: usize) -> T {
    // SAFETY (caller): `obj` is a live COM interface whose vtable has at least `index + 1` entries.
    unsafe {
        let vtbl = *(obj as *const *const usize);
        std::mem::transmute_copy::<usize, T>(&*vtbl.add(index))
    }
}

/// Some(true) password, Some(false) not, None unknown.
fn ask(automation: *mut c_void) -> Option<bool> {
    // SAFETY: IUIAutomation::GetFocusedElement is method 8 (IUnknown 3 + CompareElements,
    // CompareRuntimeIds, GetRootElement, ElementFromHandle, ElementFromPoint);
    // IUIAutomationElement::GetCurrentPropertyValue is method 10 (IUnknown 3 + SetFocus,
    // GetRuntimeId, FindFirst, FindAll, FindFirstBuildCache, FindAllBuildCache,
    // BuildUpdatedCache); the VARIANT (24 bytes) is ours; every reference obtained is released.
    unsafe {
        let mut el: *mut c_void = std::ptr::null_mut();
        let get_focused: GetFocused = method(automation, 8);
        if get_focused(automation, &mut el) < 0 || el.is_null() {
            return None;
        }
        let get_prop: GetProperty = method(el, 10);
        // the element must be a real control: a pane or a window means the browser has not built
        // its accessibility tree yet (the answer would say "not a password" for anything)
        let mut t = [0u64; 3];
        let hr_t = get_prop(el, CONTROL_TYPE, &mut t);
        let control = if hr_t >= 0 && (t[0] & 0xFFFF) as u16 == VT_I4 { Some((t[1] & 0xFFFF_FFFF) as i32) } else { None };
        clear(&mut t);
        let mut v = [0u64; 3];
        let hr = get_prop(el, IS_PASSWORD, &mut v);
        let release: Release = method(el, 2);
        release(el);
        let vt = (v[0] & 0xFFFF) as u16;
        let answer = if hr < 0 || vt != VT_BOOL { None } else { Some((v[1] & 0xFFFF) as u16 != 0) };
        clear(&mut v);
        match control {
            Some(PANE | WINDOW) | None => answer.filter(|p| *p), // only a "yes" is believed then
            _ => answer,
        }
    }
}

/// 1.9.2: is the selection of the focused text empty? Some(true) nothing selected, Some(false)
/// something selected, None unknown (no text pattern, the page not ready, any error). Only believed
/// on an Edit or Document element (a pane or a window: the browser has not built its tree yet).
fn ask_selection_empty(automation: *mut c_void) -> Option<bool> {
    type GetPatternAs = unsafe extern "system" fn(*mut c_void, i32, *const GUID, *mut *mut c_void) -> i32;
    type GetSelection = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
    type GetLength = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
    type GetElement = unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void) -> i32;
    type CompareEndpoints = unsafe extern "system" fn(*mut c_void, i32, *mut c_void, i32, *mut i32) -> i32;
    // SAFETY: vtable indices from UIAutomationClient.h: IUIAutomation::GetFocusedElement 8;
    // IUIAutomationElement::GetCurrentPropertyValue 10, GetCurrentPatternAs 14;
    // IUIAutomationTextPattern::GetSelection 5; IUIAutomationTextRangeArray::get_Length 3,
    // GetElement 4; IUIAutomationTextRange::CompareEndpoints 5 (Start = 0, End = 1). Every
    // reference obtained is released once; the VARIANT is ours.
    unsafe {
        let mut el: *mut c_void = std::ptr::null_mut();
        let get_focused: GetFocused = method(automation, 8);
        if get_focused(automation, &mut el) < 0 || el.is_null() {
            return None;
        }
        let release_el: Release = method(el, 2);
        let get_prop: GetProperty = method(el, 10);
        let mut t = [0u64; 3];
        let hr_t = get_prop(el, CONTROL_TYPE, &mut t);
        let control = if hr_t >= 0 && (t[0] & 0xFFFF) as u16 == VT_I4 { Some((t[1] & 0xFFFF_FFFF) as i32) } else { None };
        clear(&mut t);
        if !matches!(control, Some(EDIT | DOCUMENT)) {
            release_el(el);
            return None;
        }
        let mut pat: *mut c_void = std::ptr::null_mut();
        let get_pattern: GetPatternAs = method(el, 14);
        let hr_p = get_pattern(el, TEXT_PATTERN, &IID_TEXT_PATTERN, &mut pat);
        release_el(el);
        if hr_p < 0 || pat.is_null() {
            return None;
        }
        let release_pat: Release = method(pat, 2);
        let mut arr: *mut c_void = std::ptr::null_mut();
        let get_sel: GetSelection = method(pat, 5);
        let hr_s = get_sel(pat, &mut arr);
        release_pat(pat);
        if hr_s < 0 || arr.is_null() {
            return None;
        }
        let release_arr: Release = method(arr, 2);
        let mut n = 0i32;
        let get_len: GetLength = method(arr, 3);
        if get_len(arr, &mut n) < 0 {
            release_arr(arr);
            return None;
        }
        // no range at all: unknown, not « nothing selected » (audit Fable)
        let mut empty = if n > 0 { Some(true) } else { None };
        for i in 0..n.min(16) {
            let mut r: *mut c_void = std::ptr::null_mut();
            let get_el: GetElement = method(arr, 4);
            if get_el(arr, i, &mut r) < 0 || r.is_null() {
                empty = None;
                break;
            }
            let cmp: CompareEndpoints = method(r, 5);
            let mut c = 0i32;
            let hr_c = cmp(r, 0, r, 1, &mut c);
            let release_r: Release = method(r, 2);
            release_r(r);
            if hr_c < 0 {
                empty = None;
                break;
            }
            if c != 0 {
                empty = Some(false);
                break;
            }
        }
        release_arr(arr);
        empty
    }
}

/// 1.9.5: UIA_ProcessIdPropertyId, UIA_ClassNamePropertyId, VT_BSTR; Windows Terminal's text
/// control is "TermControl"
const PROCESS_ID: i32 = 30002;
const CLASS_NAME: i32 = 30012;
const VT_BSTR: u16 = 8;

/// A BSTR as a String (None if null).
unsafe fn bstr(p: *const u16) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY (caller): `p` is a valid BSTR (its length is stored before it).
    unsafe {
        let n = windows_sys::Win32::Foundation::SysStringLen(p) as usize;
        Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, n)))
    }
}

/// 1.9.5: the text just before the cursor in the focused Windows Terminal control of process
/// `pid` (at least `n` characters when there are). Refused when the focus is elsewhere (command
/// palette, search box, settings: XAML fields of the same window), when there is no
/// cursor range or a selection is active.
fn ask_terminal(automation: *mut c_void, n: usize, pid: u32) -> Result<String, &'static str> {
    type GetPatternAs = unsafe extern "system" fn(*mut c_void, i32, *const GUID, *mut *mut c_void) -> i32;
    type GetSelection = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
    type GetLength = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
    type GetElement = unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void) -> i32;
    type CompareEndpoints = unsafe extern "system" fn(*mut c_void, i32, *mut c_void, i32, *mut i32) -> i32;
    type CloneRange = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
    type MoveEndpoint = unsafe extern "system" fn(*mut c_void, i32, i32, i32, *mut i32) -> i32;
    type GetText = unsafe extern "system" fn(*mut c_void, i32, *mut *const u16) -> i32;
    // SAFETY: vtable indices from UIAutomationClient.h: IUIAutomation::GetFocusedElement 8;
    // IUIAutomationElement::GetCurrentPropertyValue 10, GetCurrentPatternAs 14;
    // IUIAutomationTextPattern::GetSelection 5; IUIAutomationTextRangeArray::get_Length 3,
    // GetElement 4; IUIAutomationTextRange::Clone 3, CompareEndpoints 5, GetText 12,
    // MoveEndpointByUnit 14 (endpoint Start = 0, End = 1; unit Character = 0). Every reference
    // obtained is released once; VARIANTs and BSTRs are freed.
    unsafe {
        let mut el: *mut c_void = std::ptr::null_mut();
        let get_focused: GetFocused = method(automation, 8);
        if get_focused(automation, &mut el) < 0 || el.is_null() {
            return Err("no focused element");
        }
        let release_el: Release = method(el, 2);
        let get_prop: GetProperty = method(el, 10);
        let mut v = [0u64; 3];
        let hr = get_prop(el, PROCESS_ID, &mut v);
        let el_pid = if hr >= 0 && (v[0] & 0xFFFF) as u16 == VT_I4 { Some((v[1] & 0xFFFF_FFFF) as u32) } else { None };
        clear(&mut v);
        let hr = get_prop(el, CLASS_NAME, &mut v);
        let class = if hr >= 0 && (v[0] & 0xFFFF) as u16 == VT_BSTR { bstr(v[1] as *const u16) } else { None };
        clear(&mut v);
        if el_pid != Some(pid) || class.as_deref() != Some("TermControl") {
            release_el(el);
            return Err("focus not in the terminal text");
        }
        let mut pat: *mut c_void = std::ptr::null_mut();
        let get_pattern: GetPatternAs = method(el, 14);
        let hr_p = get_pattern(el, TEXT_PATTERN, &IID_TEXT_PATTERN, &mut pat);
        release_el(el);
        if hr_p < 0 || pat.is_null() {
            return Err("no text pattern");
        }
        let release_pat: Release = method(pat, 2);
        let mut arr: *mut c_void = std::ptr::null_mut();
        let get_sel: GetSelection = method(pat, 5);
        let hr_s = get_sel(pat, &mut arr);
        release_pat(pat);
        if hr_s < 0 || arr.is_null() {
            return Err("no cursor range");
        }
        let release_arr: Release = method(arr, 2);
        let mut count = 0i32;
        let get_len: GetLength = method(arr, 3);
        let mut r: *mut c_void = std::ptr::null_mut();
        let get_el: GetElement = method(arr, 4);
        let ok = get_len(arr, &mut count) >= 0 && count == 1 && get_el(arr, 0, &mut r) >= 0 && !r.is_null();
        release_arr(arr);
        if !ok {
            return Err("no single cursor range");
        }
        let release_r: Release = method(r, 2);
        let cmp: CompareEndpoints = method(r, 5);
        let mut c = 0i32;
        if cmp(r, 0, r, 1, &mut c) < 0 || c != 0 {
            release_r(r);
            return Err("selection active");
        }
        let mut r2: *mut c_void = std::ptr::null_mut();
        let clone: CloneRange = method(r, 3);
        let hr_c = clone(r, &mut r2);
        release_r(r);
        if hr_c < 0 || r2.is_null() {
            return Err("no cursor range");
        }
        let release_r2: Release = method(r2, 2);
        let mv: MoveEndpoint = method(r2, 14);
        let mut moved = 0i32;
        // a few characters more than typed: display line breaks may count as characters
        let back = -((n.min(400) + 8) as i32);
        let hr_m = mv(r2, 0, 0, back, &mut moved);
        let mut text: *const u16 = std::ptr::null();
        let get_text: GetText = method(r2, 12);
        let hr_t = if hr_m >= 0 { get_text(r2, 2048, &mut text) } else { -1 };
        release_r2(r2);
        let s = if hr_t >= 0 { bstr(text) } else { None };
        if !text.is_null() {
            // the screen text is wiped before it is freed
            let len = windows_sys::Win32::Foundation::SysStringLen(text) as usize;
            std::ptr::write_bytes(text as *mut u16, 0, len);
            windows_sys::Win32::Foundation::SysFreeString(text);
        }
        s.ok_or("text not read")
    }
}

enum Kind {
    Password,
    SelectionEmpty,
    /// 1.9.5: the text before the cursor in Windows Terminal (characters, process, reply)
    Terminal(usize, u32, Sender<Result<String, &'static str>>),
}

/// where each question goes: what is asked, and the reply channel of the caller
type Asker = Sender<(Kind, Sender<Option<bool>>)>;
static TX: OnceLock<Mutex<Option<Asker>>> = OnceLock::new();

/// Asks on the UIA thread; the answer is sent to `reply` (from that thread).
pub fn focused_is_password(reply: Sender<Option<bool>>) {
    send(Kind::Password, reply);
}

/// 1.9.2: is the selection of the focused field empty? Waits at most `ms` (worker thread only, never
/// the hook thread). None: unknown, or no answer in time.
pub fn selection_empty(ms: u64) -> Option<bool> {
    let (tx, rx) = channel();
    send(Kind::SelectionEmpty, tx);
    rx.recv_timeout(std::time::Duration::from_millis(ms)).ok().flatten()
}

/// 1.9.5: the text just before the cursor in the focused Windows Terminal control of process
/// `pid`, waiting at most `ms` (worker thread only). Err(why) when it cannot be told.
pub fn terminal_before_caret(n: usize, pid: u32, ms: u64) -> Result<String, &'static str> {
    let (tx, rx) = channel();
    let (dummy, _r) = channel();
    send(Kind::Terminal(n, pid, tx), dummy);
    rx.recv_timeout(std::time::Duration::from_millis(ms)).unwrap_or(Err("no answer in time"))
}

/// 1.9.3: the UI Automation thread is answering a question.
static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 1.9.2: a web page or an Electron app came to the front: a first question is asked now (answer
/// dropped), so that the browser has its accessibility ready when F10 asks (its first answer comes
/// too late or empty: 422 ms lost in the Windows test). Once per window.
pub fn warm_up(hwnd: usize) {
    // 1.9.3: the same window is warmed again after 10 s: Chrome switches its accessibility off by
    // itself when nobody asks for a while (the first F10 after that met a new child window)
    static LAST: Mutex<(usize, Option<std::time::Instant>)> = Mutex::new((0, None));
    // never queued while a question is being answered (an F10 may be waiting for it)
    if BUSY.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    {
        let mut last = LAST.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if last.0 == hwnd && last.1.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(10)) {
            return;
        }
        *last = (hwnd, Some(std::time::Instant::now()));
    }
    let (tx, _rx) = channel();
    send(Kind::SelectionEmpty, tx);
}

fn send(kind: Kind, reply: Sender<Option<bool>>) {
    let tx = TX.get_or_init(|| {
        let (tx, rx) = channel::<(Kind, Sender<Option<bool>>)>();
        let ok = std::thread::Builder::new().name("klf-uia".into()).spawn(move || {
            // SAFETY: COM for this thread only; the object lives as long as the thread.
            let automation = unsafe {
                CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32);
                let mut p: *mut c_void = std::ptr::null_mut();
                let hr = CoCreateInstance(&CLSID_CUIAUTOMATION, std::ptr::null_mut(), CLSCTX_INPROC_SERVER, &IID_IUIAUTOMATION, &mut p);
                if hr < 0 { std::ptr::null_mut() } else { p }
            };
            while let Ok(mut q) = rx.recv() {
                // only the latest question is answered (older askers gave up already)
                while let Ok(newer) = rx.try_recv() {
                    let _ = q.1.send(None);
                    if let Kind::Terminal(_, _, r) = &q.0 {
                        let _ = r.send(Err("superseded"));
                    }
                    q = newer;
                }
                BUSY.store(true, std::sync::atomic::Ordering::SeqCst);
                let answer = if automation.is_null() {
                    None
                } else {
                    match &q.0 {
                        Kind::Password => ask(automation),
                        Kind::SelectionEmpty => ask_selection_empty(automation),
                        Kind::Terminal(n, pid, r) => {
                            // an answer nobody waits for any more is wiped
                            if let Err(std::sync::mpsc::SendError(Ok(late))) = r.send(ask_terminal(automation, *n, *pid)) {
                                klf_core::terminal::wipe_string(late);
                            }
                            None
                        }
                    }
                };
                BUSY.store(false, std::sync::atomic::Ordering::SeqCst);
                let _ = q.1.send(answer);
            }
        });
        Mutex::new(ok.is_ok().then_some(tx))
    });
    let sent = tx.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().is_some_and(|t| t.send((kind, reply.clone())).is_ok());
    if !sent {
        let _ = reply.send(None);
    }
}

/// The browsers and Electron apps whose password fields only UI Automation can tell.
pub fn needs_uia(focus_class: &str) -> bool {
    // (in Chrome, Edge and Electron the focus is often on the top window Chrome_WidgetWin_1)
    matches!(focus_class, "Chrome_RenderWidgetHostHWND" | "Chrome_WidgetWin_0" | "Chrome_WidgetWin_1" | "MozillaWindowClass" | "Internet Explorer_Server")
}
