//! Clipboard: complete save / restore (all memory formats), text read/write, with OUR hidden
//! window as owner (OpenClipboard(NULL) + EmptyClipboard makes SetClipboardData fail).
//! Never sleep while the clipboard is open (it would freeze the other programs).
//! Every check of "is this still ours?" is made in the SAME opening as the write it protects:
//! nobody can copy in between.

use crate::util::wide;
use klf_core::flow::{may_publish, may_restore, still_ours, Capture, ClipObs, ClipState, PublishResult, RestoreResult, SaveResult, StillOurs};
use windows_sys::Win32::Graphics::Gdi::{DeleteEnhMetaFile, GetEnhMetaFileBits, SetEnhMetaFileBits, HENHMETAFILE};
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::DataExchange::*;
use windows_sys::Win32::System::Memory::*;

const CF_UNICODETEXT: u32 = 13;
/// Windows lists an image twice (DIB + DIBV5): a 4K screenshot is ~66 MB.
const MAX_SNAPSHOT_BYTES: usize = 128 * 1024 * 1024;
/// Largest text read for conversion or comparison (audit B10): checked BEFORE copying.
const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;
const CF_BITMAP: u32 = 2;
const CF_ENHMETAFILE: u32 = 14;

/// OLE formats describe the live object of the source program; put back by another owner
/// they can confuse Office: skipped on purpose (same list as 1.2.x).
/// GlobalFlags answer for a handle that is not a memory block (GMEM_INVALID_HANDLE)
const GMEM_INVALID: u32 = 0x8000;
const SKIPPED_NAMES: &[&str] = &[
    "DataObject", "Ole Private Data", "Embed Source", "Embedded Object", "Link Source",
    "Link Source Descriptor", "Object Descriptor", "OwnerLink", "ObjectLink", "Native", "Link",
];

/// Marks that keep OUR texts out of the Win+V history, cloud sync and clipboard managers.
const MARK_NAMES: &[&str] = &["ExcludeClipboardContentFromMonitorProcessing", "CanIncludeInClipboardHistory", "CanUploadToCloudClipboard"];

/// Saved formats: memory blocks, and the bytes of a vector picture (CF_ENHMETAFILE).
pub struct Snapshot {
    items: Vec<(u32, Vec<u8>)>,
    /// the copy was marked "do not keep" by its program (password managers): never kept in
    /// memory for later if it cannot be put back at once
    pub secret: bool,
}

/// Opens the clipboard; closed when dropped. Retries happen BEFORE opening.
struct Open;

impl Open {
    fn new(owner: HWND, sleep_ms: u64) -> Option<Open> {
        for _ in 0..10 {
            // SAFETY: owner is our hidden window (or null).
            if unsafe { OpenClipboard(owner) } != 0 {
                return Some(Open);
            }
            std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
        }
        None
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: we opened it.
        unsafe {
            CloseClipboard();
        }
    }
}

fn format_name(format: u32) -> Option<String> {
    if format < 0xC000 {
        return None;
    }
    let mut buf = [0u16; 256];
    // SAFETY: buffer size passed.
    let n = unsafe { GetClipboardFormatNameW(format, buf.as_mut_ptr(), buf.len() as i32) };
    if n <= 0 {
        return None;
    }
    Some(crate::util::prefix(&buf, n as usize))
}

/// What to do with a format found in the user's clipboard before emptying it (audit A4).
enum Kind {
    /// plain memory block: copied
    Memory,
    /// vector picture: its bytes are copied (GetEnhMetaFileBits)
    EnhMetafile,
    /// rebuilt by Windows from another saved format (CF_BITMAP from DIB, METAFILEPICT from
    /// ENHMETAFILE, PALETTE), our marks, or OLE descriptions of the source program
    /// (skipped on purpose, documented)
    Skip,
    /// cannot be duplicated (owner-drawn, private, GDI objects): nothing is done at all
    Unsupported,
}

fn kind_of(format: u32) -> Kind {
    if let Some(name) = format_name(format) {
        if SKIPPED_NAMES.iter().any(|s| s.eq_ignore_ascii_case(&name)) || MARK_NAMES.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
            return Kind::Skip;
        }
    }
    match format {
        CF_ENHMETAFILE => Kind::EnhMetafile,
        CF_BITMAP | 3 | 9 => Kind::Skip,
        0x80 | 0x82 | 0x83 | 0x8E => Kind::Unsupported,
        // CF_PRIVATEFIRST..LAST: private data of the source program (often how it recognises
        // its own copy): cannot be duplicated, so nothing is done at all (audit 1.3.2, R4)
        f if (0x200..=0x2FF).contains(&f) => Kind::Unsupported,
        // CF_GDIOBJFIRST..LAST: GDI objects that cannot be duplicated
        f if (0x300..=0x3FF).contains(&f) => Kind::Unsupported,
        _ => Kind::Memory,
    }
}

/// Copies a global memory block of `size` bytes (None if it cannot be locked).
///
/// # Safety
/// `h` must come from GetClipboardData while the clipboard is open, `size` = GlobalSize(h).
unsafe fn read_global_sized(h: HANDLE, size: usize) -> Option<Vec<u8>> {
    let p = GlobalLock(h as HGLOBAL) as *const u8;
    if p.is_null() {
        return None;
    }
    let data = std::slice::from_raw_parts(p, size).to_vec();
    GlobalUnlock(h as HGLOBAL);
    Some(data)
}

/// Standard formats that are plain memory blocks: if one of them cannot be read, the copy
/// cannot be saved completely -> nothing is done (text, ANSI/OEM text, locale, DIB, DIBV5,
/// files, SYLK, DIF, TIFF, pen data, RIFF, WAVE).
fn is_standard_memory(format: u32) -> bool {
    matches!(format, 1 | 4 | 5 | 6 | 7 | 8 | 10 | 11 | 12 | 13 | 15 | 16 | 17)
}

fn alloc_global(bytes: &[u8]) -> Option<HGLOBAL> {
    // SAFETY: size passed; the block is locked only while copying; freed on failure.
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1));
        if h.is_null() {
            return None;
        }
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            GlobalFree(h);
            return None;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        GlobalUnlock(h);
        Some(h)
    }
}

fn register(name: &str) -> u32 {
    let w = wide(name);
    // SAFETY: valid string.
    unsafe { RegisterClipboardFormatW(w.as_ptr()) }
}

/// The marks: CanIncludeInClipboardHistory = 0, CanUploadToCloudClipboard = 0, Exclude... (any data).
fn mark_items() -> Vec<(u32, Vec<u8>)> {
    MARK_NAMES.iter().map(|n| register(n)).filter(|f| *f != 0).map(|f| (f, 0u32.to_le_bytes().to_vec())).collect()
}

pub fn sequence() -> u32 {
    // SAFETY: no arguments.
    unsafe { GetClipboardSequenceNumber() }
}

/// Saves every format then empties the clipboard, in ONE opening (nobody can copy between the
/// save and the emptying). Returns the sequence number right after emptying.
pub fn save_and_clear(owner: HWND) -> (SaveResult, u32, Option<Snapshot>) {
    let r = save_and_clear_inner(owner);
    if let (SaveResult::ReadError, Some(f)) = (r.0, r.3) {
        crate::util::log(&format!("Clipboard read error or format that cannot be kept: {f}"));
    }
    (r.0, r.1, r.2)
}

fn save_and_clear_inner(owner: HWND) -> (SaveResult, u32, Option<Snapshot>, Option<u32>) {
    let Some(open) = Open::new(owner, 20) else { return (SaveResult::Busy, 0, None, None) };
    let mut items = Vec::new();
    let mut total = 0usize;
    let mut format = 0u32;
    let (mut bitmap, mut dib, mut others) = (false, false, false);
    let marks: Vec<u32> = MARK_NAMES.iter().map(|n| register(n)).collect();
    let mut secret = false;
    loop {
        // SAFETY: clipboard open. EnumClipboardFormats does not always clear the last error.
        format = unsafe {
            SetLastError(0);
            EnumClipboardFormats(format)
        };
        if format == 0 {
            // SAFETY: right after the call.
            if unsafe { GetLastError() } != 0 {
                return (SaveResult::ReadError, 0, None, Some(0));
            }
            break;
        }
        if !marks.contains(&format) {
            others = true;
        }
        secret |= format == marks[0]; // ExcludeClipboardContentFromMonitorProcessing
        match kind_of(format) {
            Kind::Skip => {
                bitmap |= format == CF_BITMAP;
                continue;
            }
            Kind::Unsupported => return (SaveResult::ReadError, 0, None, Some(format)),
            Kind::EnhMetafile => {
                // SAFETY: clipboard open; the handle belongs to the clipboard (never deleted here).
                let h = unsafe { GetClipboardData(format) } as HENHMETAFILE;
                if h.is_null() {
                    return (SaveResult::ReadError, 0, None, Some(format));
                }
                // SAFETY: size query then copy into a buffer of that size.
                let size = unsafe { GetEnhMetaFileBits(h, 0, std::ptr::null_mut()) } as usize;
                total = total.saturating_add(size);
                if size == 0 || total > MAX_SNAPSHOT_BYTES {
                    return if size == 0 { (SaveResult::ReadError, 0, None, Some(format)) } else { (SaveResult::TooBig, 0, None, None) };
                }
                let mut buf = vec![0u8; size];
                // SAFETY: buffer of `size` bytes.
                if unsafe { GetEnhMetaFileBits(h, size as u32, buf.as_mut_ptr()) } as usize != size {
                    return (SaveResult::ReadError, 0, None, Some(format));
                }
                items.push((format, buf));
            }
            Kind::Memory => {
                let standard = is_standard_memory(format);
                // SAFETY: clipboard open.
                let h = unsafe { GetClipboardData(format) };
                if h.is_null() {
                    if standard {
                        return (SaveResult::ReadError, 0, None, Some(format)); // e.g. Office frozen while rendering
                    }
                    continue; // registered delayed format its owner no longer renders: nobody can read it
                }
                // a registered format may hold a GDI handle, not a memory block: checked first
                // SAFETY: h comes from GetClipboardData, clipboard open.
                let size = if unsafe { GlobalFlags(h as HGLOBAL) } == GMEM_INVALID { 0 } else { unsafe { GlobalSize(h as HGLOBAL) } };
                if size == 0 {
                    if standard {
                        return (SaveResult::ReadError, 0, None, Some(format));
                    }
                    continue; // not a memory block (GDI handle in a registered format)
                }
                // the limit is checked BEFORE copying the block
                total = total.saturating_add(size);
                if total > MAX_SNAPSHOT_BYTES {
                    return (SaveResult::TooBig, 0, None, None);
                }
                // SAFETY: as above, size = GlobalSize(h).
                let Some(data) = (unsafe { read_global_sized(h, size) }) else {
                    return (SaveResult::ReadError, 0, None, Some(format));
                };
                dib |= matches!(format, 8 | 17);
                items.push((format, data));
            }
        }
    }
    // a bitmap Windows cannot rebuild, or a clipboard where nothing could be kept: untouched
    if (bitmap && !dib) || (others && items.is_empty()) {
        return (SaveResult::ReadError, 0, None, Some(if bitmap { CF_BITMAP } else { 0 }));
    }
    // SAFETY: clipboard open.
    if unsafe { EmptyClipboard() } == 0 {
        return (SaveResult::Busy, 0, None, None);
    }
    // the sequence number may only move at CloseClipboard: read it after closing
    drop(open);
    (SaveResult::Complete, sequence(), Some(Snapshot { items, secret }), None)
}

enum TextRead {
    Absent,
    Text(String),
    TooBig,
}

/// Clipboard must be open. The size is checked BEFORE copying (audit B10); UTF-16 decoded
/// straight from the locked block.
fn read_text_open() -> TextRead {
    // SAFETY: clipboard open; the block is read up to its size and the first NUL.
    unsafe {
        let h = GetClipboardData(CF_UNICODETEXT);
        if h.is_null() {
            return TextRead::Absent;
        }
        let size = GlobalSize(h as HGLOBAL);
        if size == 0 {
            return TextRead::Absent;
        }
        if size > MAX_TEXT_BYTES {
            return TextRead::TooBig;
        }
        let p = GlobalLock(h as HGLOBAL) as *const u16;
        if p.is_null() {
            return TextRead::Absent;
        }
        let units = std::slice::from_raw_parts(p, size / 2);
        let end = units.iter().position(|u| *u == 0).unwrap_or(units.len());
        let t: String = char::decode_utf16(units[..end].iter().copied()).map(|r| r.unwrap_or('\u{FFFD}')).collect();
        GlobalUnlock(h as HGLOBAL);
        TextRead::Text(t)
    }
}

fn clipboard_owner() -> HWND {
    // SAFETY: no arguments.
    unsafe { GetClipboardOwner() }
}

fn pid_of(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    // SAFETY: pid written.
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    pid
}

/// Text, sequence number and owner, read in one opening. `target_pids`: processes of the
/// target window, its focused control and its child windows (UWP: the app's CoreWindow).
pub fn capture(owner: HWND, target_pids: &[u32]) -> Capture {
    let Some(_open) = Open::new(owner, 8) else { return Capture { seq: sequence(), busy: true, ..Default::default() } };
    let o = clipboard_owner();
    let foreign_owner = !o.is_null() && o != owner && {
        let pid = pid_of(o);
        pid != 0 && !target_pids.contains(&pid)
    };
    let (text, too_big) = match read_text_open() {
        TextRead::Text(t) => (Some(t), false),
        TextRead::TooBig => (None, true),
        TextRead::Absent => (None, false),
    };
    let c = Capture { text, seq: sequence(), owner: o as usize, foreign_owner, too_big, busy: false, formats: list_formats() };
    drop(_open); // the log is written once the clipboard is closed (never held during a disk write)
    if foreign_owner {
        crate::util::log(&format!("Clipboard owner is process {} (not the target)", pid_of(o)));
    }
    c
}

/// Clipboard must be open. Every format listed, in order.
fn list_formats() -> Vec<u32> {
    let mut v = Vec::new();
    let mut f = 0u32;
    loop {
        // SAFETY: clipboard open.
        f = unsafe { EnumClipboardFormats(f) };
        if f == 0 || v.len() >= 256 {
            return v;
        }
        v.push(f);
    }
}

/// Clipboard must be open. True if it holds a format that is neither text (with the formats
/// Windows derives from it) nor one of our marks: an image, files, a cell... from someone else.
fn foreign_formats_present() -> bool {
    let marks: Vec<u32> = MARK_NAMES.iter().map(|n| register(n)).collect();
    let mut f = 0u32;
    loop {
        // SAFETY: clipboard open.
        f = unsafe { EnumClipboardFormats(f) };
        if f == 0 {
            return false;
        }
        if !matches!(f, 1 | 7 | 13 | 16) && !marks.contains(&f) {
            return true;
        }
    }
}

/// Clipboard must be open. `owner` is our hidden window: never null (the provenance rules
/// rely on "the owner is us").
fn observe(owner: HWND) -> ClipObs {
    debug_assert!(!owner.is_null());
    let o = clipboard_owner();
    let (text, big) = match read_text_open() {
        TextRead::Text(t) => (Some(t), false),
        TextRead::TooBig => (None, true),
        TextRead::Absent => (None, false),
    };
    ClipObs {
        seq: sequence(),
        owner_is_us: !owner.is_null() && o == owner,
        owner: o as usize,
        text,
        formats: list_formats(),
        // a text too big to read is never "ours"
        foreign_formats: big || foreign_formats_present(),
    }
}

/// A prepared clipboard item: a memory block or a vector picture, owned by us until given.
enum Prepared {
    Global(u32, HGLOBAL),
    Emf(HENHMETAFILE),
}

fn free_all(items: &[Prepared]) {
    for it in items {
        // SAFETY: handles we created and still own.
        unsafe {
            match it {
                Prepared::Global(_, h) => {
                    GlobalFree(*h);
                }
                Prepared::Emf(h) => {
                    DeleteEnhMetaFile(*h);
                }
            }
        }
    }
}

/// Prepares every item BEFORE opening (no allocation while the clipboard is open).
fn prepare<'a>(items: impl Iterator<Item = (u32, &'a [u8])>) -> Option<Vec<Prepared>> {
    let mut out: Vec<Prepared> = Vec::new();
    for (f, d) in items {
        let p = if f == CF_ENHMETAFILE {
            // SAFETY: bytes saved by GetEnhMetaFileBits.
            let h = unsafe { SetEnhMetaFileBits(d.len() as u32, d.as_ptr()) };
            (!h.is_null()).then_some(Prepared::Emf(h))
        } else {
            alloc_global(d).map(|h| Prepared::Global(f, h))
        };
        match p {
            Some(p) => out.push(p),
            None => {
                free_all(&out);
                return None;
            }
        }
    }
    Some(out)
}

enum Put {
    Ok,
    FailedBeforeEmpty,
    FailedAfterEmpty,
}

/// Clipboard must be open. Takes ownership of the items (given to the system or freed).
fn put_open(items: Vec<Prepared>) -> Put {
    // SAFETY: clipboard open; after a successful SetClipboardData the system owns the handle.
    unsafe {
        if EmptyClipboard() == 0 {
            free_all(&items);
            return Put::FailedBeforeEmpty;
        }
        let mut ok = true;
        for it in items {
            let given = match it {
                Prepared::Global(f, h) => !SetClipboardData(f, h as HANDLE).is_null() || {
                    GlobalFree(h);
                    false
                },
                Prepared::Emf(h) => !SetClipboardData(CF_ENHMETAFILE, h as HANDLE).is_null() || {
                    DeleteEnhMetaFile(h);
                    false
                },
            };
            ok &= given;
        }
        if ok { Put::Ok } else { Put::FailedAfterEmpty }
    }
}

/// "Copy" button of the settings window (diagnostic report, conversion workshop): a plain text
/// put in the clipboard on a click, outside the F10 sequence. False if it could not be done.
pub fn put_plain_text(owner: HWND, text: &str) -> bool {
    let body = text_bytes(text);
    let Some(items) = prepare(std::iter::once((CF_UNICODETEXT, body.as_slice()))) else { return false };
    let Some(_open) = Open::new(owner, 30) else {
        free_all(&items);
        return false;
    };
    matches!(put_open(items), Put::Ok)
}

fn text_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().chain(std::iter::once(0)).flat_map(|u| u.to_le_bytes()).collect()
}

/// Puts our converted text (with the "not in history" marks) in the clipboard, only if what is
/// there is still the application's copy made for our Ctrl+C (checked in the same opening).
pub fn publish(owner: HWND, text: &str, st: &ClipState) -> PublishResult {
    let body = text_bytes(text);
    let marks = mark_items();
    let items = std::iter::once((CF_UNICODETEXT, body.as_slice())).chain(marks.iter().map(|(f, d)| (*f, d.as_slice())));
    let Some(prepared) = prepare(items) else { return PublishResult::Failed };
    let Some(open) = Open::new(owner, 10) else {
        free_all(&prepared);
        return PublishResult::Failed;
    };
    if !may_publish(st, &observe(owner)) {
        free_all(&prepared);
        return PublishResult::NewCopy;
    }
    let r = put_open(prepared);
    drop(open);
    // read after closing: the number may only move at CloseClipboard
    match r {
        Put::Ok => PublishResult::Done(sequence()),
        Put::FailedBeforeEmpty => PublishResult::Failed,
        Put::FailedAfterEmpty => PublishResult::FailedAfterEmpty,
    }
}

/// Right before Ctrl+V: is our converted text still in the clipboard? (audit A2)
pub fn is_still_ours(owner: HWND, st: &ClipState) -> StillOurs {
    match Open::new(owner, 10) {
        Some(_open) if still_ours(st, &observe(owner)) => StillOurs::Yes,
        Some(_open) => StillOurs::No,
        // busy is NOT a new copy: the saved clipboard must not be dropped
        None => StillOurs::Busy,
    }
}

/// Puts the snapshot back, only if what is in the clipboard now is ours (or the application's
/// copy made for us): a new copy of the user is never overwritten (checked in the same opening).
pub fn restore(owner: HWND, snap: &Snapshot, st: &ClipState) -> RestoreResult {
    // no clone of the snapshot: the items are prepared straight from it
    let marks = mark_items();
    let prepared = if snap.items.is_empty() {
        Vec::new() // the user's clipboard was empty: leave it empty (not even our marks)
    } else {
        let items = snap.items.iter().map(|(f, d)| (*f, d.as_slice())).chain(marks.iter().map(|(f, d)| (*f, d.as_slice())));
        match prepare(items) {
            Some(p) => p,
            None => return RestoreResult::Failed,
        }
    };
    let Some(_open) = Open::new(owner, 20) else {
        free_all(&prepared);
        return RestoreResult::Failed;
    };
    if !may_restore(st, &observe(owner)) {
        free_all(&prepared);
        return RestoreResult::KeptUserCopy;
    }
    match put_open(prepared) {
        Put::Ok => RestoreResult::Restored,
        _ => RestoreResult::Failed,
    }
}
