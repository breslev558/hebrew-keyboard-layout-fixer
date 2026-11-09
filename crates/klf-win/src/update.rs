//! Update in one click (1.5.5, PUBLIC per-user install only; see klf-core/src/update.rs).
//! WinHTTP (automatic proxy, TLS 1.2/1.3, Windows certificate store: never pinned, works behind
//! a filtering proxy), allowed hosts checked after redirects, SHA-256 + ECDSA P-256 signature
//! checked with Windows CNG, file version checked, then the exe is swapped in place:
//! KeyboardLayoutFixer.exe -> .old, .new -> .exe, the new copy is started (--restart waits for us),
//! we quit. Any failure before the swap leaves everything as it was; a failed start is undone.
//! No PowerShell, no telemetry: only GET requests to GitHub.

use crate::util::{log, wide};
use klf_core::update::{self as u, Release};
use std::ptr::{null, null_mut};
use std::sync::Mutex;
use windows_sys::Win32::Networking::WinHttp::*;
use windows_sys::Win32::Security::Cryptography::*;

/// Public key of the update signing key (P-256, X then Y). The private key stays offline.
const PUB_X: [u8; 32] = [0x8b, 0x7e, 0xd4, 0xcd, 0xbc, 0x07, 0x2e, 0x6e, 0x9a, 0x0b, 0xa1, 0x0f, 0xd1, 0xe0, 0x2e, 0x01, 0x24, 0xf1, 0xb1, 0xd7, 0xbb, 0xab, 0x08, 0xc4, 0x42, 0xde, 0xc5, 0x55, 0xdd, 0x93, 0x0b, 0x2d];
const PUB_Y: [u8; 32] = [0x5a, 0x63, 0xf8, 0xc8, 0xd2, 0xfa, 0x21, 0xd1, 0x63, 0x5e, 0xa2, 0x55, 0x25, 0xf7, 0xdf, 0x24, 0xca, 0xf8, 0x95, 0x43, 0xfc, 0x69, 0xa3, 0x39, 0xcb, 0xf9, 0x06, 0x74, 0x1b, 0x26, 0xfe, 0x77];

/// The update found by the last check (shown in the tray menu).
pub static OFFER: Mutex<Option<Release>> = Mutex::new(None);

fn current() -> (u32, u32, u32) {
    u::parse_version(klf_core::VERSION).unwrap_or((0, 0, 0))
}

fn local_programs() -> String {
    std::env::var("LOCALAPPDATA").map(|l| format!("{l}\\Programs\\KeyboardLayoutFixer")).unwrap_or_default()
}

fn exe_dir() -> String {
    let p = crate::platform::sys::exe_path();
    std::path::Path::new(&p).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The program is the public per-user install, no administrator policy, setting on.
pub fn allowed(setting_on: bool) -> bool {
    u::may_check(setting_on, crate::platform::school::policy_key_exists(), &exe_dir(), &local_programs())
}

/// Removes what an earlier update left behind (old exe, temporary files). At start.
pub fn cleanup() {
    let p = crate::platform::sys::exe_path();
    let _ = std::fs::remove_file(format!("{p}.old"));
    let _ = std::fs::remove_file(format!("{p}.new"));
    // an old copy that was still running at the previous update was set aside as .old-<number>
    let path = std::path::Path::new(&p);
    if let (Some(dir), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) {
        let prefix = format!("{name}.old-");
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if e.file_name().to_str().is_some_and(|n| n.starts_with(&prefix) && n[prefix.len()..].bytes().all(|b| b.is_ascii_digit())) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    // after an update in one click, "Installed apps" shows the new version number (1.6.0)
    if allowed(true) {
        use windows_sys::Win32::System::Registry::*;
        let key = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\KeyboardLayoutFixer");
        let name = wide("DisplayVersion");
        let val = wide(klf_core::VERSION);
        let mut hk: HKEY = null_mut();
        // SAFETY: valid strings; the key is only opened if it already exists (never created here).
        unsafe {
            if RegOpenKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, KEY_SET_VALUE, &mut hk) == 0 {
                RegSetValueExW(hk, name.as_ptr(), 0, REG_SZ, val.as_ptr() as *const u8, (val.len() * 2) as u32);
                RegCloseKey(hk);
            }
        }
    }
}

struct H(*mut core::ffi::c_void);
impl Drop for H {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: a WinHTTP handle we opened.
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

/// GET over HTTPS, at most `max` bytes, only from allowed hosts (also after redirects), status 200.
fn get(url: &str, max: usize) -> Option<Vec<u8>> {
    if !u::host_allowed(url) {
        return None;
    }
    request(url, None, max, &|fin| u::host_allowed(fin)).map(|(_, b)| b)
}

/// POST of a small JSON report to the reports receiver (1.6.1): no redirect followed. HTTP status,
/// None on a network problem.
pub fn post_json(url: &str, body: &str) -> Option<u32> {
    request(url, Some(body), 4096, &|_| true).map(|(st, _)| st)
}

/// HTTPS request (GET, or POST with a JSON body): Windows proxy settings, TLS 1.2/1.3, the final
/// address checked by `final_ok` after any redirect. (status, body).
fn request(url: &str, body: Option<&str>, max: usize, final_ok: &dyn Fn(&str) -> bool) -> Option<(u32, Vec<u8>)> {
    let rest = url.strip_prefix("https://")?;
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let agent = wide(if body.is_some() { "KeyboardLayoutFixer-report" } else { "KeyboardLayoutFixer-update" });
    let (h, p, get_) = (wide(host), wide(path), wide(if body.is_some() { "POST" } else { "GET" }));
    // SAFETY: WinHTTP calls with valid strings; handles closed by H; buffers sized.
    unsafe {
        let s = H(WinHttpOpen(agent.as_ptr(), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, null(), null(), 0));
        if s.0.is_null() {
            return None;
        }
        let proto: u32 = WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2 | WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_3;
        if WinHttpSetOption(s.0, WINHTTP_OPTION_SECURE_PROTOCOLS, &proto as *const u32 as *const _, 4) == 0 {
            // older Windows 10 without TLS 1.3: TLS 1.2 only, never the old protocols
            let p12: u32 = WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2;
            if WinHttpSetOption(s.0, WINHTTP_OPTION_SECURE_PROTOCOLS, &p12 as *const u32 as *const _, 4) == 0 {
                return None;
            }
        }
        WinHttpSetTimeouts(s.0, 15_000, 15_000, 15_000, 30_000);
        let c = H(WinHttpConnect(s.0, h.as_ptr(), INTERNET_DEFAULT_HTTPS_PORT, 0));
        if c.0.is_null() {
            return None;
        }
        let r = H(WinHttpOpenRequest(c.0, get_.as_ptr(), p.as_ptr(), null(), null(), null(), WINHTTP_FLAG_SECURE));
        if r.0.is_null() {
            return None;
        }
        if host == "api.github.com" || body.is_some() {
            // the API answer never needs a redirect: none is followed
            let never: u32 = WINHTTP_OPTION_REDIRECT_POLICY_NEVER;
            WinHttpSetOption(r.0, WINHTTP_OPTION_REDIRECT_POLICY, &never as *const u32 as *const _, 4);
        }
        let sent = match body {
            Some(b) => {
                let hdr = wide("Content-Type: application/json");
                WinHttpSendRequest(r.0, hdr.as_ptr(), u32::MAX, b.as_ptr() as *const _, b.len() as u32, b.len() as u32, 0)
            }
            None => WinHttpSendRequest(r.0, null(), 0, null(), 0, 0, 0),
        };
        if sent == 0 || WinHttpReceiveResponse(r.0, null_mut()) == 0 {
            return None;
        }
        let mut status: u32 = 0;
        let mut len: u32 = 4;
        if WinHttpQueryHeaders(r.0, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, null(), &mut status as *mut u32 as *mut _, &mut len, null_mut()) == 0 {
            return None;
        }
        if body.is_some() {
            return Some((status, Vec::new())); // a report: only the status matters
        }
        if status != 200 {
            return None;
        }
        // final address after GitHub's redirects
        let mut buf = vec![0u16; 4096];
        let mut blen = (buf.len() * 2) as u32;
        if WinHttpQueryOption(r.0, WINHTTP_OPTION_URL, buf.as_mut_ptr() as *mut _, &mut blen) == 0 {
            return None;
        }
        let fin = String::from_utf16_lossy(&buf[..(blen as usize / 2)]);
        if !final_ok(fin.trim_end_matches('\0')) {
            log(&format!("Update: download refused from {}", fin.split('/').nth(2).unwrap_or("?")));
            return None;
        }
        let mut out = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            if std::time::Instant::now() > deadline {
                return None; // a server sending drop by drop never holds the thread for hours
            }
            let mut chunk = [0u8; 16384];
            let mut got: u32 = 0;
            if WinHttpReadData(r.0, chunk.as_mut_ptr() as *mut _, chunk.len() as u32, &mut got) == 0 {
                return None;
            }
            if got == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..got as usize]);
            if out.len() > max {
                return None;
            }
        }
        Some((status, out))
    }
}

fn sha256(data: &[u8]) -> Option<[u8; 32]> {
    let alg = wide("SHA256");
    // SAFETY: CNG calls with sized buffers; provider closed.
    unsafe {
        let mut h: BCRYPT_ALG_HANDLE = null_mut();
        if BCryptOpenAlgorithmProvider(&mut h, alg.as_ptr(), null(), 0) != 0 {
            return None;
        }
        let mut out = [0u8; 32];
        let r = BCryptHash(h, null(), 0, data.as_ptr() as *mut u8, data.len() as u32, out.as_mut_ptr(), 32);
        BCryptCloseAlgorithmProvider(h, 0);
        (r == 0).then_some(out)
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// ECDSA P-256 signature (r||s) of SHA-256(`msg`) by the update key.
fn signature_ok(msg: &[u8], sig: &[u8; 64]) -> bool {
    let Some(hash) = sha256(msg) else { return false };
    let alg = wide("ECDSA_P256");
    let blob_type = wide("ECCPUBLICBLOB");
    let mut blob = Vec::with_capacity(8 + 64);
    blob.extend_from_slice(&0x3153_4345u32.to_le_bytes()); // BCRYPT_ECDSA_PUBLIC_P256_MAGIC "ECS1"
    blob.extend_from_slice(&32u32.to_le_bytes());
    blob.extend_from_slice(&PUB_X);
    blob.extend_from_slice(&PUB_Y);
    // SAFETY: CNG calls with sized buffers; key and provider closed.
    unsafe {
        let mut h: BCRYPT_ALG_HANDLE = null_mut();
        if BCryptOpenAlgorithmProvider(&mut h, alg.as_ptr(), null(), 0) != 0 {
            return false;
        }
        let mut key: BCRYPT_KEY_HANDLE = null_mut();
        let ok = BCryptImportKeyPair(h, null_mut(), blob_type.as_ptr(), &mut key, blob.as_ptr(), blob.len() as u32, 0) == 0
            && BCryptVerifySignature(key, null(), hash.as_ptr(), 32, sig.as_ptr(), 64, 0) == 0;
        if !key.is_null() {
            BCryptDestroyKey(key);
        }
        BCryptCloseAlgorithmProvider(h, 0);
        ok
    }
}

fn file_version(path: &str) -> Option<(u32, u32, u32)> {
    use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO};
    let w = wide(path);
    let root = wide("\\");
    // SAFETY: buffer sized by GetFileVersionInfoSizeW; the pointer returned points into it.
    unsafe {
        let n = GetFileVersionInfoSizeW(w.as_ptr(), null_mut());
        if n == 0 {
            return None;
        }
        let mut buf = vec![0u8; n as usize];
        if GetFileVersionInfoW(w.as_ptr(), 0, n, buf.as_mut_ptr() as *mut _) == 0 {
            return None;
        }
        let mut p: *mut core::ffi::c_void = null_mut();
        let mut l: u32 = 0;
        if VerQueryValueW(buf.as_ptr() as *const _, root.as_ptr(), &mut p, &mut l) == 0 || p.is_null() {
            return None;
        }
        let fi = &*(p as *const VS_FIXEDFILEINFO);
        Some((fi.dwFileVersionMS >> 16, fi.dwFileVersionMS & 0xFFFF, fi.dwFileVersionLS >> 16))
    }
}

/// Looks for an update (worker thread). Some(release) if one should be offered.
pub fn check(skip_tag: &str, skip_at: u64) -> Option<Release> {
    let body = get(u::API_URL, u::MAX_API_BYTES)?;
    let r = u::parse_release(std::str::from_utf8(&body).ok()?)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let skip = (!skip_tag.is_empty()).then_some((skip_tag, skip_at));
    u::should_offer(current(), &r, skip, now).then_some(r)
}

/// Why an installation did not happen: a network problem is retried at the next check, a
/// refused file or a failed swap freezes this version for 7 days.
pub enum Failure {
    Network(String),
    Refused(String),
}

impl From<&str> for Failure {
    fn from(s: &str) -> Self {
        Failure::Refused(s.to_string())
    }
}
impl From<String> for Failure {
    fn from(s: String) -> Self {
        Failure::Refused(s)
    }
}

/// Downloads, checks and swaps the exe, then starts the new copy (watched after we quit).
pub fn install(r: &Release) -> Result<(), Failure> {
    let net = |m: &str| Failure::Network(m.to_string());
    // 1.6.4: the signed description carries the version, the size and the SHA-256 of the exe
    let upd_file = get(&r.upd_url, 1024).ok_or_else(|| net("update description not downloaded"))?;
    let sig_file = get(&r.upd_sig_url, 1024).ok_or_else(|| net("signature not downloaded"))?;
    let sig = u::parse_sig(std::str::from_utf8(&sig_file).map_err(|_| "signature unreadable")?).ok_or("signature unreadable")?;
    if !signature_ok(&upd_file, &sig) {
        return Err("signature of the update description is NOT valid".into());
    }
    let info = u::parse_update_file(std::str::from_utf8(&upd_file).map_err(|_| "update description unreadable")?).ok_or("update description unreadable")?;
    if !u::update_info_ok(&info, r, current()) {
        return Err(format!("signed description ({:?}, {} bytes) does not match the release {}", info.version, info.size, r.tag).into());
    }
    let announced = info.sha256;
    let exe = get(&r.exe_url, u::MAX_EXE_BYTES as usize).ok_or_else(|| net("exe not downloaded"))?;
    let got = hex(&sha256(&exe).ok_or("sha256 failed")?);
    // size and SHA-256 checked BEFORE anything is written or parsed
    if exe.len() as u64 != r.exe_size || !got.eq_ignore_ascii_case(&announced) {
        return Err(format!("download refused (size {} / {}, or SHA-256 different)", exe.len(), r.exe_size).into());
    }
    let cur = crate::platform::sys::exe_path();
    let new = format!("{cur}.new");
    let old = format!("{cur}.old");
    // a local file problem (disk full, antivirus holding the file a moment) is tried again at the
    // next check, never frozen for 7 days like a refused file
    std::fs::write(&new, &exe).map_err(|e| net(&format!("cannot write the new file: {e}")))?;
    let fv = file_version(&new);
    if !u::download_ok(r, exe.len() as u64, &got, &announced, fv) {
        let _ = std::fs::remove_file(&new);
        return Err(format!("download refused (version {fv:?} is not {})", r.tag).into());
    }
    if std::fs::remove_file(&old).is_err() && std::path::Path::new(&old).exists() {
        // an older copy still running from .old: set aside under another name (removed at start)
        let _ = std::fs::rename(&old, format!("{cur}.old-{}", std::process::id()));
    }
    // a running exe can be renamed (not deleted): the swap never leaves the user without a program
    std::fs::rename(&cur, &old).map_err(|e| {
        let _ = std::fs::remove_file(&new);
        net(&format!("cannot move the running program aside: {e}"))
    })?;
    if let Err(e) = std::fs::rename(&new, &cur) {
        let _ = std::fs::rename(&old, &cur);
        let _ = std::fs::remove_file(&new);
        return Err(net(&format!("cannot put the new program in place: {e}")));
    }
    // the new copy waits up to 30 s for us to quit (--update), then tells us it really started
    // (named event). We watch it after our window is gone: no fixed wait, and if it never starts
    // or stops before, the old program is put back and started again
    let undo = |why: &str| -> Failure {
        let _ = std::fs::remove_file(&cur);
        let _ = std::fs::rename(&old, &cur);
        Failure::Refused(format!("{why} (old program put back)"))
    };
    let event = started_event(true);
    if !event.is_null() {
        // never a signal left from an earlier attempt
        // SAFETY: event created just above.
        unsafe { windows_sys::Win32::System::Threading::ResetEvent(event) };
    }
    let child = std::process::Command::new(&cur).arg("--update").spawn().map_err(|_| {
        close(event);
        undo("the new program could not be started")
    })?;
    let previous = WATCH.lock().unwrap_or_else(std::sync::PoisonError::into_inner).replace(Watch { child, event: event as usize, cur, old, tag: r.tag.clone() });
    if let Some(p) = previous {
        close(p.event as windows_sys::Win32::Foundation::HANDLE);
    }
    log(&format!("Update to {} installed, new copy started (watched until it is ready)", r.tag));
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// 1.6.4: the new copy is validated only once it has really started

const STARTED_EVENT: &str = "Local\\KeyboardLayoutFixer_UpdateStarted";
/// how long the new copy has to start (it first waits for us to quit)
const START_LIMIT_MS: u32 = 60_000;

struct Watch {
    child: std::process::Child,
    event: usize,
    cur: String,
    old: String,
    tag: String,
}
static WATCH: Mutex<Option<Watch>> = Mutex::new(None);

fn started_event(create: bool) -> windows_sys::Win32::Foundation::HANDLE {
    use windows_sys::Win32::System::Threading::{CreateEventW, OpenEventW, EVENT_MODIFY_STATE};
    let name = wide(STARTED_EVENT);
    // SAFETY: valid name; manual-reset event, not signalled at creation; closed by the caller.
    unsafe {
        if create {
            CreateEventW(null(), 1, 0, name.as_ptr())
        } else {
            OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr())
        }
    }
}

fn close(h: windows_sys::Win32::Foundation::HANDLE) {
    if !h.is_null() {
        // SAFETY: a handle we opened.
        unsafe { windows_sys::Win32::Foundation::CloseHandle(h) };
    }
}

/// New copy started with --update: tells the old copy it is running (window and hotkey ready),
/// then removes the old exe once the old copy has quit.
pub fn signal_started() {
    let h = started_event(false);
    if !h.is_null() {
        // SAFETY: event opened with EVENT_MODIFY_STATE.
        unsafe { windows_sys::Win32::System::Threading::SetEvent(h) };
        close(h);
    }
    let _ = std::thread::Builder::new().name("cleanup-old".into()).spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(15));
        let p = crate::platform::sys::exe_path();
        let _ = std::fs::remove_file(format!("{p}.old"));
    });
}

/// Old copy, after its window is gone and the single-instance lock released: waits until the new
/// copy says it started; if it stops first or takes more than 60 s, puts the old program back,
/// does not offer that version again for 7 days, and starts the old program again.
pub fn watch_after_exit() {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::WaitForMultipleObjects;
    let Some(mut w) = WATCH.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take() else { return };
    let event = w.event as HANDLE;
    let started = if event.is_null() {
        // no event (very unlikely): the old rule, the copy must still run after 5 s
        std::thread::sleep(std::time::Duration::from_secs(5));
        matches!(w.child.try_wait(), Ok(None))
    } else {
        let handles = [event, w.child.as_raw_handle() as HANDLE];
        // SAFETY: two valid handles; returns which one is signalled first, or a timeout.
        let r = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, START_LIMIT_MS) };
        close(event);
        r == WAIT_OBJECT_0
    };
    if started {
        log(&format!("Update to {}: new copy started, old copy quits", w.tag));
        return;
    }
    let _ = w.child.kill();
    let _ = w.child.wait();
    std::thread::sleep(std::time::Duration::from_millis(500));
    // an antivirus may hold the new file a moment: a few tries, else the old exe is started as it is
    let mut back = false;
    for _ in 0..10 {
        let _ = std::fs::remove_file(&w.cur);
        if std::fs::rename(&w.old, &w.cur).is_ok() {
            back = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    if !back {
        log(&format!("Update to {} FAILED; the old program could not be put back in place, started from {}", w.tag, w.old));
        let _ = std::process::Command::new(&w.old).spawn();
        return;
    }
    let mut s = crate::app::user_settings();
    s.update_skip_tag = w.tag.clone();
    s.update_skip_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let _ = crate::app::save_settings(&s);
    log(&format!("Update to {} FAILED (new copy did not start): old program put back and started again", w.tag));
    let _ = std::process::Command::new(&w.cur).spawn();
}
