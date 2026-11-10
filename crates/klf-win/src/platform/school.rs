//! Settings imposed by the administrator: registry key SOFTWARE\Policies\KeyboardLayoutFixer,
//! machine (HKLM, the one to use: a standard user cannot write there) then user (HKCU); the machine
//! wins. Read only. Values: REG_DWORD / REG_SZ / REG_MULTI_SZ with the names of settings.json;
//! the AppRules and ExcludedProcesses lists also as sub-keys (one REG_SZ value per program).

use crate::util::wide;
use klf_core::school::RegValue;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::*;

pub const KEY: &str = "SOFTWARE\\Policies\\KeyboardLayoutFixer";

/// Opens a key for reading (None if absent).
fn open(root: HKEY, path: &str) -> Option<HKEY> {
    let p = wide(path);
    let mut hk: HKEY = null_mut();
    // SAFETY: valid string; key closed by the caller.
    (unsafe { RegOpenKeyExW(root, p.as_ptr(), 0, KEY_READ, &mut hk) } == ERROR_SUCCESS).then_some(hk)
}

/// An administrator manages this PC: the machine policy key exists (no automatic update then).
pub fn policy_key_exists() -> bool {
    match open(HKEY_LOCAL_MACHINE, KEY) {
        Some(hk) => {
            // SAFETY: key opened just above.
            unsafe { RegCloseKey(hk) };
            true
        }
        None => false,
    }
}

/// Every value of a key: (name, value); values of other types or too big are left out.
fn values(hk: HKEY) -> Vec<(String, RegValue)> {
    let mut out = Vec::new();
    for i in 0..512u32 {
        let mut name = [0u16; 256];
        let mut name_len = name.len() as u32;
        let mut ty = 0u32;
        let mut data = vec![0u8; 16 * 1024];
        let mut data_len = data.len() as u32;
        // SAFETY: buffers and their sizes passed; only written.
        let r = unsafe { RegEnumValueW(hk, i, name.as_mut_ptr(), &mut name_len, null(), &mut ty, data.as_mut_ptr(), &mut data_len) };
        if r != ERROR_SUCCESS {
            if r == 259 {
                break; // ERROR_NO_MORE_ITEMS
            }
            // too big (ERROR_MORE_DATA) or unreadable: said in the diagnostic
            let n = String::from_utf16_lossy(&name[..(name_len as usize).min(name.len())]);
            out.push((if n.is_empty() { format!("valeur n°{i}") } else { n }, RegValue::Text("(valeur trop grande ou illisible)".into())));
            continue;
        }
        let n = String::from_utf16_lossy(&name[..name_len as usize]);
        data.truncate(data_len as usize);
        let units: Vec<u16> = data.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        let v = match ty {
            REG_DWORD if data.len() >= 4 => RegValue::Dword(u32::from_le_bytes([data[0], data[1], data[2], data[3]])),
            REG_SZ => RegValue::Text(crate::util::from_wide(&units).trim().to_string()),
            REG_MULTI_SZ => RegValue::Lines(units.split(|&c| c == 0).map(String::from_utf16_lossy).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()),
            _ => RegValue::Text(String::from("(type non pris en charge)")),
        };
        out.push((n, v));
    }
    out
}

fn read_root(root: HKEY) -> Vec<(String, RegValue)> {
    let mut out = Vec::new();
    if let Some(hk) = open(root, KEY) {
        out.extend(values(hk));
        // SAFETY: opened above.
        unsafe { RegCloseKey(hk) };
    }
    // sub-key forms (easier in a .reg file and in an ADMX): one value per program
    if let Some(hk) = open(root, &format!("{KEY}\\AppRules")) {
        let lines = values(hk).into_iter().map(|(n, v)| match v {
            RegValue::Text(t) => format!("{n}={t}"),
            _ => format!("{n}=?"),
        });
        out.push(("AppRules".into(), RegValue::Lines(lines.collect())));
        // SAFETY: opened above.
        unsafe { RegCloseKey(hk) };
    }
    if let Some(hk) = open(root, &format!("{KEY}\\ExcludedProcesses")) {
        out.push(("ExcludedProcesses".into(), RegValue::Lines(values(hk).into_iter().map(|(n, _)| n).collect())));
        // SAFETY: opened above.
        unsafe { RegCloseKey(hk) };
    }
    out
}

/// The values of the administrator: the user's key first, then the machine's (which wins).
pub fn read() -> Vec<(String, RegValue)> {
    let mut v = read_root(HKEY_CURRENT_USER);
    v.extend(read_root(HKEY_LOCAL_MACHINE));
    v
}
