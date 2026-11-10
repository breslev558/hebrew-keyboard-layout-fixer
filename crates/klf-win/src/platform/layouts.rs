//! Installed keyboard layouts, read WITHOUT activating them and without changing anything in
//! Windows: GetKeyboardLayoutList, then each key by scan code with MapVirtualKeyExW +
//! ToUnicodeEx (flag 0x4: the kernel keyboard state, dead keys included, is left untouched).
//! AltGr is never read (no Ctrl/Alt in the key state); dead keys are left out.

use klf_core::layouts::{build_keymap, classify_hkl, KeyChar, KeyMap, LatinKind, LayoutKind};
use std::ptr::null_mut;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayoutList, MapVirtualKeyExW, ToUnicodeEx, HKL, MAPVK_VSC_TO_VK};

/// Scan codes of the character keys (digits row, three letter rows, the 102nd key).
const SCAN_CODES: &[u16] = &[
    0x29, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, //
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x2B, //
    0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, //
    0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x56,
];

/// ToUnicodeEx: do not change the keyboard state (Windows 10 1607 and later).
const TU_NO_STATE_CHANGE: u32 = 0x4;

/// Installed layouts (low 32 bits of each HKL), in Windows order.
pub fn installed() -> Vec<u32> {
    let mut list: [HKL; 64] = [null_mut(); 64];
    // SAFETY: buffer size passed; only written.
    let n = unsafe { GetKeyboardLayoutList(list.len() as i32, list.as_mut_ptr()) };
    list.iter().take(n.clamp(0, 64) as usize).map(|h| *h as usize as u32).collect()
}

/// The full HKL of an installed layout (the handle may have high bits on 64-bit Windows).
fn handle(hkl32: u32) -> Option<HKL> {
    let mut list: [HKL; 64] = [null_mut(); 64];
    // SAFETY: as above.
    let n = unsafe { GetKeyboardLayoutList(list.len() as i32, list.as_mut_ptr()) };
    list.iter().take(n.clamp(0, 64) as usize).copied().find(|h| *h as usize as u32 == hkl32)
}

/// Unshifted level of every character key of an installed layout.
pub fn read_layout(hkl32: u32) -> Option<Vec<KeyChar>> {
    read_level(hkl32, false)
}

/// 1.8.2: the Shift level (for text replacement shortcuts such as "@@").
pub fn read_layout_shifted(hkl32: u32) -> Option<Vec<KeyChar>> {
    read_level(hkl32, true)
}

/// 1.8.2: the AltGr level (Ctrl+Alt), for shortcuts typed with AltGr (@ on AZERTY).
pub fn read_layout_altgr(hkl32: u32) -> Option<Vec<KeyChar>> {
    read_level_state(hkl32, &[0x11, 0x12, 0xA2, 0xA5])
}

fn read_level(hkl32: u32, shift: bool) -> Option<Vec<KeyChar>> {
    read_level_state(hkl32, if shift { &[0x10, 0xA0] } else { &[] })
}

fn read_level_state(hkl32: u32, keys_down: &[usize]) -> Option<Vec<KeyChar>> {
    let hkl = handle(hkl32)?;
    let mut state = [0u8; 256]; // no Caps Lock; only the keys asked for are down
    for &k in keys_down {
        state[k] = 0x80;
    }
    let mut out = Vec::new();
    for &sc in SCAN_CODES {
        // SAFETY: plain values; the buffer is ours and its size is passed.
        unsafe {
            let vk = MapVirtualKeyExW(sc as u32, MAPVK_VSC_TO_VK, hkl);
            if vk == 0 {
                continue;
            }
            let mut buf = [0u16; 8];
            let n = ToUnicodeEx(vk, sc as u32, state.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, TU_NO_STATE_CHANGE, hkl);
            if n < 0 {
                // dead key: left out (with the 0x4 flag nothing is kept; on a Windows older than
                // 10 1607 the dead key would only stay in the state of THIS worker thread)
                continue;
            }
            if n != 1 {
                continue; // nothing, or several characters (ligature): left out
            }
            if let Some(ch) = char::from_u32(buf[0] as u32) {
                out.push(KeyChar { sc, ch });
            }
        }
    }
    Some(out)
}

/// What the conversion will use, for a target whose active layout is `active` (low 32 bits).
pub enum Choice {
    /// key map + HKL of the Latin layout to switch to (None: none installed, US table used)
    Map(KeyMap, Option<u32>),
    /// the active layout is not supported (its name)
    Unsupported(String),
}

/// 1.8.6: the last Latin layout seen active (with 3 layouts or more, Hebrew is converted to it).
static LAST_LATIN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// 1.8.8: the Latin layout chosen in the settings, copied here by the UI thread: F10 works on its
/// own thread, where `app::settings()` (thread-local) only gives the defaults (audit Fable).
pub static PREFERRED_LATIN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Notes the active layout of a window (called when a program comes to the front).
pub fn note_active(hkl: Option<u32>) {
    use std::collections::HashMap;
    static LATIN: std::sync::Mutex<Option<HashMap<u32, bool>>> = std::sync::Mutex::new(None);
    let Some(h) = hkl.filter(|h| h & 0x3FF != 0x0D && *h != 0) else { return };
    // only Latin layouts (not Russian, Arabic...): read once per layout
    let latin = {
        let mut g = LATIN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *g.get_or_insert_with(HashMap::new).entry(h).or_insert_with(|| {
            matches!(classify_hkl(h), LayoutKind::Latin(_)) || read_layout(h).is_some_and(|t| klf_core::layouts::is_latin_table(&t))
        })
    };
    if latin {
        LAST_LATIN.store(h, std::sync::atomic::Ordering::Relaxed);
    }
}

/// What F10 makes of a layout: known by its name, or recognised from its table read from Windows
/// (Hebrew variants, Latin layouts with the US letters).
fn layout_kind(h: u32) -> LayoutKind {
    use klf_core::layouts::{is_latin_table, same_hebrew_letters};
    match classify_hkl(h) {
        LayoutKind::Unsupported => match read_layout(h) {
            Some(t) if same_hebrew_letters(&t) || hebrew_variant_ok(h, &t) => LayoutKind::HebrewStandard,
            Some(t) if is_latin_table(&t) => LayoutKind::Latin(LatinKind::Us),
            _ => LayoutKind::Unsupported,
        },
        k => k,
    }
}

/// 1.8.9: the same answer as F10 (the settings window showed ✗ for a Hebrew variant F10 accepts).
pub fn supported(h: u32) -> bool {
    !matches!(layout_kind(h), LayoutKind::Unsupported)
}

pub fn choose(active: Option<u32>) -> Choice {
    let installed = installed();
    // what a layout is: by its exact value, else by its letters
    let kind = layout_kind;
    let first_latin = || {
        // 1.8.6: the Latin layout chosen in the settings, else the last one used, else a layout
        // supported by name first, then any Latin layout (US table)
        let fixed = PREFERRED_LATIN.load(std::sync::atomic::Ordering::Relaxed);
        let last = LAST_LATIN.load(std::sync::atomic::Ordering::Relaxed);
        [fixed, last].into_iter().find(|h| *h != 0 && installed.contains(h) && matches!(kind(*h), LayoutKind::Latin(_)))
            .or_else(|| installed.iter().copied().find(|h| matches!(classify_hkl(*h), LayoutKind::Latin(_))))
            .or_else(|| installed.iter().copied().find(|h| matches!(kind(*h), LayoutKind::Latin(_))))
    };
    let latin_hkl = match active.map(kind) {
        Some(LayoutKind::Latin(_)) => {
            LAST_LATIN.store(active.unwrap_or(0), std::sync::atomic::Ordering::Relaxed);
            active
        }
        Some(LayoutKind::HebrewStandard) | None => first_latin(),
        Some(LayoutKind::Unsupported) => {
            return Choice::Unsupported(klf_core::layouts::hkl_name(active.unwrap_or(0)));
        }
    };
    match latin_hkl.map(classify_hkl) {
        Some(LayoutKind::Latin(LatinKind::FrenchAzerty)) => {
            let h = latin_hkl.unwrap_or(0);
            // the Hebrew side: the installed Hebrew layout, or the reference table
            let he = hebrew_layout().and_then(read_layout).unwrap_or_else(klf_core::layouts::reference::hebrew);
            match read_layout(h).map(|fr| build_keymap(LatinKind::FrenchAzerty, &fr, &he)) {
                Some(Ok(m)) => Choice::Map(m, Some(h)),
                // a French layout that does not look like the standard one: refused, never guessed
                Some(Err(e)) => Choice::Unsupported(format!("français AZERTY ({e:?})")),
                None => Choice::Unsupported("français AZERTY (illisible)".into()),
            }
        }
        // 1.8.2: a Latin layout whose letters are not on the US keys (QWERTZ, Belgian AZERTY):
        // its own table read from Windows, if it builds; else the US table as before
        _ if latin_hkl.is_some_and(|h| classify_hkl(h) == LayoutKind::Unsupported) => {
            let h = latin_hkl.unwrap_or(0);
            match read_layout(h) {
                Some(t) if !klf_core::layouts::qwerty_letters(&t) => {
                    let he = hebrew_layout().and_then(read_layout).unwrap_or_else(klf_core::layouts::reference::hebrew);
                    match build_keymap(LatinKind::OtherFromWindows, &t, &he) {
                        Ok(m) => Choice::Map(m, latin_hkl),
                        Err(e) => {
                            crate::util::log(&format!("Latin layout {h:08X}: own table refused ({e:?}), US table used"));
                            Choice::Map(klf_core::convert::us_map(), latin_hkl)
                        }
                    }
                }
                _ => Choice::Map(klf_core::convert::us_map(), latin_hkl),
            }
        }
        // 1.8.5: a Hebrew variant whose keys differ from the classic Hebrew layout (Hebrew
        // Standard 2018, 0003040D: Q, W, comma, period, slash, apostrophe...): its own table
        _ if hebrew_layout().and_then(|h| read_layout(h).map(|t| (h, t))).is_some_and(|(h, t)| h != 0x040D_040D && !same_unshifted(&t, &klf_core::layouts::reference::hebrew())) => {
            let he = hebrew_layout().and_then(read_layout).unwrap_or_else(klf_core::layouts::reference::hebrew);
            match build_keymap(LatinKind::Us, &klf_core::layouts::reference::us(), &he) {
                Ok(m) => Choice::Map(m, latin_hkl),
                Err(_) => Choice::Map(klf_core::convert::us_map(), latin_hkl),
            }
        }
        // US, or a Latin layout with the US letters (UK, US-International...): the fixed US table
        _ => Choice::Map(klf_core::convert::us_map(), latin_hkl),
    }
}

/// 1.8.5: a Hebrew-language layout (0x040D) whose 27 letters each have their own key, wherever
/// they are: it converts with its own table.
fn hebrew_variant_ok(hkl: u32, t: &[KeyChar]) -> bool {
    hkl & 0xFFFF == 0x040D && build_keymap(LatinKind::Us, &klf_core::layouts::reference::us(), t).is_ok()
}

/// The same character on every key the two tables share.
fn same_unshifted(a: &[KeyChar], b: &[KeyChar]) -> bool {
    a.iter().all(|k| b.iter().find(|x| x.sc == k.sc).is_none_or(|x| x.ch == k.ch))
}

/// The standard Hebrew layout, if installed.
pub fn hebrew_layout() -> Option<u32> {
    let installed = installed();
    installed.iter().copied().find(|h| classify_hkl(*h) == LayoutKind::HebrewStandard)
        // 1.8.2: only a variant installed ("Hebrew (Standard)" SI-1452, 0002040D): the keyboard
        // must still switch to it after a conversion (before, it stayed on English)
        .or_else(|| installed.iter().copied().find(|h| h & 0xFFFF == 0x040D && read_layout(*h).is_some_and(|t| klf_core::layouts::same_hebrew_letters(&t) || hebrew_variant_ok(*h, &t))))
}

/// Asks the window to switch to an installed layout (WM_INPUTLANGCHANGEREQUEST).
pub fn request(hwnd: windows_sys::Win32::Foundation::HWND, hkl32: u32) -> bool {
    let Some(h) = handle(hkl32) else { return false };
    // SAFETY: posting a message, no pointers.
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(hwnd, windows_sys::Win32::UI::WindowsAndMessaging::WM_INPUTLANGCHANGEREQUEST, 0, h as isize) != 0 }
}
