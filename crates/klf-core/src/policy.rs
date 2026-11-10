//! Pure decision rules (no Windows calls), so that they are tested on Linux.

/// Kind of console window, from its window class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleKind {
    /// Classic console host (conhost) of Windows 10/11: handled by the console path.
    Classic,
    /// Windows Terminal, ConEmu, mintty...: a simulated Ctrl+C would be an interrupt, and the
    /// console API cannot reach the right tab -> nothing is done.
    Unsupported,
}

pub fn console_kind(window_class: &str) -> Option<ConsoleKind> {
    match window_class {
        "ConsoleWindowClass" => Some(ConsoleKind::Classic),
        "CASCADIA_HOSTING_WINDOW_CLASS" | "VirtualConsoleClass" | "mintty" | "PuTTY" => Some(ConsoleKind::Unsupported),
        _ => None,
    }
}

/// Programs where F10 never does anything: remote control / virtual machines (the keys belong
/// to the other computer) and password managers. Compared on the exe name, lower case.
const BUILT_IN_EXCLUDED: &[&str] = &[
    "rustdesk.exe",
    "mstsc.exe",
    "msrdc.exe",
    "vmconnect.exe",
    "anydesk.exe",
    "teamviewer.exe",
    "vmware.exe",
    "vmware-vmx.exe",
    "vmplayer.exe",
    "virtualbox.exe",
    "virtualboxvm.exe",
    "keepass.exe",
    "keepassxc.exe",
    "1password.exe",
    "bitwarden.exe",
    "lastpass.exe",
    // code editors: their integrated terminal is not a console window, and the first Ctrl+C
    // leaves before we can know whether text is selected -> it would stop the running command
    "code.exe",
    "code - insiders.exe",
    "cursor.exe",
    "vscodium.exe",
];

/// Base name of a path, lower case ("C:\\X\\RustDesk.EXE" -> "rustdesk.exe").
pub fn exe_base_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_lowercase()
}

/// 1.9.0: remote desktop and virtual machine windows: while one is in front, F10 and F9 are given
/// to it (not taken here), so that the copy running on the other PC gets them; before, F10 was
/// taken and nothing was done on either side.
pub fn is_remote_screen(exe_name: &str) -> bool {
    matches!(exe_base_name(exe_name).as_str(), "mstsc.exe" | "msrdc.exe" | "vmconnect.exe" | "anydesk.exe" | "teamviewer.exe" | "rustdesk.exe" | "vncviewer.exe" | "tvnviewer.exe" | "parsecd.exe")
}

pub fn is_excluded(exe_name: &str, extra: &[String]) -> bool {
    let n = exe_base_name(exe_name);
    BUILT_IN_EXCLUDED.contains(&n.as_str()) || extra.iter().any(|e| exe_base_name(e) == n)
}

/// Text fields where "only the last word" is tried: Windows Edit/RichEdit, Word,
/// Chromium (Chrome, Edge, Teams, Outlook web), LibreOffice. Anywhere else (Excel above all:
/// Shift+Home there selects CELLS) the usual "start of the line" is used.
pub fn last_word_field(window_class: &str, focus_class: &str) -> bool {
    let f = focus_class.to_ascii_lowercase();
    if window_class == "XLMAIN" || f == "excel7" || f.starts_with("excel") {
        return false;
    }
    f == "edit" || f.starts_with("richedit") || f == "_wwg" || f == "chrome_renderwidgethosthwnd" || f == "salframe" || f == "salsubframe"
}

/// Programs a rule may name: a real program name, never ours, never the desktop/file explorer
/// (a rule there would also cut F10 in file names and the taskbar search).
pub fn rule_allowed_for(exe_name: &str) -> bool {
    let n = exe_base_name(exe_name);
    !n.is_empty() && n.ends_with(".exe") && !matches!(n.as_str(), "keyboardlayoutfixer.exe" | "explorer.exe" | "applicationframehost.exe")
}

/// What we know about the selection before copying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Empty,
    NonEmpty,
    Unknown,
}

/// EM_GETSEL called with null pointers returns start in LOWORD, end in HIWORD (or -1 if > 65535).
pub fn selection_from_em_getsel(result: usize) -> Selection {
    let start = result & 0xFFFF;
    let end = (result >> 16) & 0xFFFF;
    if start == 0xFFFF || end == 0xFFFF {
        Selection::Unknown
    } else if start == end {
        Selection::Empty
    } else {
        Selection::NonEmpty
    }
}

/// Window classes whose Ctrl+C never copies "the whole line" when nothing is selected.
const NO_LINE_COPY_CLASSES: &[&str] = &["_WwG", "Edit", "RICHEDIT50W", "RichEdit20W", "RichEdit20A", "RichEditD2DPT"];

/// Some programs (VS Code, Visual Studio, Excel outside cell editing...) copy the whole line or
/// cell when Ctrl+C is pressed WITHOUT a selection. Pasting the conversion would then duplicate
/// the line, or replace an Excel formula by text. Sign: exactly one line break, at the end.
pub fn line_copy_suspect(text: &str, selection: Selection, focus_class: &str) -> bool {
    // Excel grid: a copied range (tabs, several lines) pasted back would replace formulas by
    // values. Editing inside a cell gives no tab and, except with Alt+Enter, no line break.
    if focus_class == "EXCEL7" && text.contains(['\t', '\r', '\n']) {
        return true;
    }
    if selection == Selection::NonEmpty || NO_LINE_COPY_CLASSES.contains(&focus_class) {
        return false;
    }
    let body = text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).or_else(|| text.strip_suffix('\r'));
    match body {
        Some(b) => !b.is_empty() && !b.contains(['\r', '\n']),
        None => false,
    }
}

pub fn contains_line_break(text: &str) -> bool {
    text.contains(['\r', '\n'])
}

// ---- console (classic conhost) ----

/// Console input mode bits (GetConsoleMode on CONIN$).
pub const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
pub const ENABLE_LINE_INPUT: u32 = 0x0002;
pub const ENABLE_ECHO_INPUT: u32 = 0x0004;
pub const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleReader {
    /// cmd.exe (or set /p) reading a line in "cooked" mode: Home/End/Backspace are handled by
    /// the console itself and never reach a program.
    CmdLine,
    /// PowerShell with PSReadLine waiting for a command.
    PsReadLine,
}

/// Decides whether the console is in a safe state. `processes` = exe names of all
/// processes attached to the console except ours. Anything else -> None (do nothing).
pub fn console_reader(processes: &[String], input_mode: u32) -> Option<ConsoleReader> {
    if processes.is_empty() {
        return None;
    }
    let names: Vec<String> = processes.iter().map(|p| exe_base_name(p)).collect();
    if !names.iter().all(|n| n == "cmd.exe" || n == "powershell.exe" || n == "pwsh.exe") {
        return None; // a program is running (python, ping, vim, more...): never touch it
    }
    if input_mode & ENABLE_VIRTUAL_TERMINAL_INPUT != 0 {
        return None;
    }
    let has_ps = names.iter().any(|n| n != "cmd.exe");
    let cooked = ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT;
    if !has_ps && input_mode & cooked == cooked {
        return Some(ConsoleReader::CmdLine);
    }
    // PSReadLine clears LINE and PROCESSED while editing (PlatformWindows.cs)
    if has_ps && input_mode & (ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT) == 0 {
        return Some(ConsoleReader::PsReadLine);
    }
    None
}

/// Text read from the console line: printable, one cell per character, reasonable length.
pub fn console_text_ok(text: &str, max_chars: usize) -> bool {
    let n = text.chars().count();
    n > 0 && n <= max_chars && text.chars().all(|c| c >= ' ' && (c as u32) < 0x1100)
}

/// F9 then "switch Caps Lock off": one press of Caps Lock ONLY if it was on at F9, a fresh
/// reading says it is still on, AND no Caps Lock press was seen in between (count of the
/// keyboard hook, same before and after). A press switches it: anything uncertain (`None`:
/// frozen window, reading refused, no hook) means no key at all (audit 1.3.7, P2).
pub fn caps_lock_press_needed(
    was_on_at_hotkey: Option<bool>,
    on_now: Option<bool>,
    presses_at_hotkey: Option<u32>,
    presses_now: Option<u32>,
) -> bool {
    let untouched = matches!((presses_at_hotkey, presses_now), (Some(a), Some(b)) if a == b);
    was_on_at_hotkey == Some(true) && on_now == Some(true) && untouched
}

/// 1.8.5: the fields where F10 without a selection may grow to the whole paragraph: web pages and
/// Electron apps (their content area), Word, plain multi-line controls; never code editors,
/// spreadsheets, terminals, address bars or lists (Up does something else there).
pub fn paragraph_field(focus_class: &str, exe: &str) -> bool {
    let c = focus_class.to_ascii_lowercase();
    // (WinForms boxes are Edit / RichEdit controls with a longer class name)
    let known = c == "chrome_renderwidgethostwnd" || c == "chrome_renderwidgethosthwnd" || c == "edit" || c.starts_with("richedit") || c == "_wwg" || c == "mozillawindowclass"
        || c.starts_with("windowsforms10.edit") || c.starts_with("windowsforms10.richedit");
    known && !no_paragraph_extension(exe) && !crate::typing::is_code_editor(exe)
}

/// 1.9.2: code editors with a terminal panel inside the same window (indistinguishable by the class):
/// Ctrl+C there may stop the program running in the terminal.
pub fn has_terminal_panel(exe: &str) -> bool {
    matches!(exe_base_name(exe).as_str(), "code.exe" | "code - insiders.exe" | "cursor.exe" | "codium.exe" | "vscodium.exe" | "windsurf.exe" | "hyper.exe" | "tabby.exe" | "trae.exe" | "positron.exe")
}

/// 1.9.2: web pages and Electron apps (Chrome, Edge, Claude Code, Teams, VS Code's web views) and
/// Firefox: Ctrl+C with nothing selected copies nothing, so the first copy is not waited for long.
pub fn copies_nothing_without_selection(focus_class: &str) -> bool {
    let c = focus_class.to_ascii_lowercase();
    // (Chrome_WidgetWin_*: the browser's own boxes such as the address bar, and Electron apps that
    // give the focus to their top window; seen in the 1.9.2 Windows test)
    c == "chrome_renderwidgethostwnd" || c == "chrome_renderwidgethosthwnd" || c == "mozillawindowclass" || c == "chrome_widgetwin_0" || c == "chrome_widgetwin_1"
}

/// 1.8.5: programs where F10 without a selection never extends to the whole paragraph (Shift+Up
/// would leave the cell being edited in Excel, or do something else in a terminal).
/// 1.8.8: cmd / PowerShell / conhost are no longer listed: their console windows never reach this
/// test (handled earlier by their window class), while a window made by a PowerShell script
/// (WinForms text box) is a real text field (this is what the Windows test showed).
pub fn no_paragraph_extension(exe: &str) -> bool {
    matches!(exe.to_ascii_lowercase().as_str(), "excel.exe" | "scalc.exe" | "soffice.bin" | "windowsterminal.exe" | "wt.exe" | "mintty.exe" | "putty.exe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_powershell_script_window_is_a_text_field() {
        assert!(paragraph_field("WindowsForms10.EDIT.app.0.141b42a_r6_ad1", "powershell.exe"));
        assert!(!paragraph_field("WindowsForms10.EDIT.app.0.141b42a_r6_ad1", "excel.exe"));
        assert!(!paragraph_field("ConsoleWindowClass", "powershell.exe"), "a console is never a paragraph field");
    }

    #[test]
    fn remote_screens_get_the_keys() {
        assert!(is_remote_screen("C:\\Windows\\System32\\MSTSC.EXE") && is_remote_screen("AnyDesk.exe"));
        assert!(!is_remote_screen("notepad.exe"));
    }

    #[test]
    fn consoles() {
        assert_eq!(console_kind("ConsoleWindowClass"), Some(ConsoleKind::Classic));
        assert_eq!(console_kind("CASCADIA_HOSTING_WINDOW_CLASS"), Some(ConsoleKind::Unsupported));
        assert_eq!(console_kind("Notepad"), None);
    }

    #[test]
    fn exclusions() {
        assert!(is_excluded("C:\\Program Files\\RustDesk\\RustDesk.EXE", &[]));
        assert!(!is_excluded("winword.exe", &[]));
        assert!(is_excluded("game.exe", &["Game.exe".into()]));
        assert_eq!(exe_base_name("C:/a/B.exe"), "b.exe");
    }

    #[test]
    fn em_getsel() {
        assert_eq!(selection_from_em_getsel(5 | (5 << 16)), Selection::Empty);
        assert_eq!(selection_from_em_getsel(2 | (7 << 16)), Selection::NonEmpty);
        assert_eq!(selection_from_em_getsel(0xFFFF | (0xFFFF << 16)), Selection::Unknown);
    }

    #[test]
    fn line_copy() {
        let u = Selection::Unknown;
        assert!(line_copy_suspect("abc\r\n", u, "Chrome_WidgetWin_1"));
        assert!(line_copy_suspect("abc\n", u, "EXCEL7"));
        assert!(!line_copy_suspect("a\r\nb", u, ""));
        assert!(!line_copy_suspect("a\r\nb\r\n", u, ""));
        assert!(!line_copy_suspect("abc", u, ""));
        assert!(!line_copy_suspect("\n", u, ""));
        assert!(!line_copy_suspect("abc\r\n", Selection::NonEmpty, ""));
        assert!(!line_copy_suspect("abc\r\n", u, "_WwG")); // Word: a selected paragraph ends with \r\n
        // Excel grid: ranges of several cells or lines are never pasted back
        assert!(line_copy_suspect("a\r\nb\r\n", u, "EXCEL7"));
        assert!(line_copy_suspect("a\tb", u, "EXCEL7"));
        assert!(!line_copy_suspect("akuo", u, "EXCEL7"));
        assert!(is_excluded("C:\\Users\\x\\AppData\\Local\\Programs\\Microsoft VS Code\\Code.exe", &[]));
    }

    #[test]
    fn console_rules() {
        let cmd = vec!["C:\\Windows\\System32\\cmd.exe".to_string()];
        let cooked = ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT | 0x80;
        assert_eq!(console_reader(&cmd, cooked), Some(ConsoleReader::CmdLine));
        assert_eq!(console_reader(&cmd, cooked | ENABLE_VIRTUAL_TERMINAL_INPUT), None);
        assert_eq!(console_reader(&cmd, ENABLE_PROCESSED_INPUT), None);
        let py = vec!["cmd.exe".to_string(), "python.exe".to_string()];
        assert_eq!(console_reader(&py, cooked), None);
        let ps = vec!["powershell.exe".to_string()];
        assert_eq!(console_reader(&ps, 0x80 | 0x10), Some(ConsoleReader::PsReadLine));
        assert_eq!(console_reader(&ps, cooked), None); // Read-Host / command running
        assert_eq!(console_reader(&[], cooked), None);
        assert!(console_text_ok("dir akuo", 100));
        assert!(console_text_ok("שלום", 100));
        assert!(!console_text_ok("a\tb", 100));
        assert!(!console_text_ok("中文", 100));
        assert!(!console_text_ok("", 100));
    }

    #[test]
    fn caps_lock_is_pressed_only_when_known_to_be_still_on() {
        let c = Some(5);
        // tests of the 1.3.7 audit: known off and unknown never send a key
        assert!(caps_lock_press_needed(Some(true), Some(true), c, c));
        assert!(!caps_lock_press_needed(Some(true), Some(false), c, c), "known off is kept off");
        assert!(!caps_lock_press_needed(Some(true), None, c, c), "unknown state must not turn Caps Lock on");
        assert!(!caps_lock_press_needed(Some(false), Some(true), c, c));
        assert!(!caps_lock_press_needed(None, Some(true), c, c), "unknown at the hotkey");
        // the user pressed Caps Lock in between (twice: reading says on again) -> left alone
        assert!(!caps_lock_press_needed(Some(true), Some(true), c, Some(7)));
        // keyboard hook missing -> never a press
        assert!(!caps_lock_press_needed(Some(true), Some(true), None, None));
        assert!(!caps_lock_press_needed(Some(true), Some(true), c, None));
    }
}

