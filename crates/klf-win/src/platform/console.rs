//! Classic console (conhost) reading through the console API: we attach to the
//! console of the window, read its input mode, selection state, cursor and line, then detach.

use super::sys;
use crate::util::wide;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::Console::*;
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

pub struct Session {
    conin: HANDLE,
    conout: HANDLE,
}

fn open(name: &str) -> HANDLE {
    let w = wide(name);
    // SAFETY: valid name; handle checked by the caller.
    unsafe {
        CreateFileW(
            w.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            0,
            null_mut(),
        )
    }
}

/// While attached, our process belongs to the console: Ctrl+C / Ctrl+Break typed by the user
/// would otherwise end it (default handler = ExitProcess). Returning TRUE ignores them.
/// CTRL_CLOSE_EVENT (the user closes the console during the ~0.5 s we are attached) cannot be
/// refused: Windows ends our process after the handler. We start a new copy first (it waits up
/// to 10 s for this one to quit). The console path never touches the clipboard: nothing to restore.
unsafe extern "system" fn ignore_ctrl(event: u32) -> windows_sys::core::BOOL {
    if event == CTRL_CLOSE_EVENT {
        crate::util::log("Console closed while attached: restarting");
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        let _ = std::process::Command::new(super::sys::exe_path()).arg("--restart").creation_flags(DETACHED_PROCESS).spawn();
    }
    1
}

impl Session {
    pub fn attach(hwnd: HWND) -> Option<Session> {
        let mut pid = 0u32;
        // SAFETY: pid written. For console windows Windows reports the client process (e.g. cmd.exe).
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if pid == 0 {
            return None;
        }
        // SAFETY: plain calls. The handler is set before attaching (a Ctrl+C right after
        // AttachConsole) AND again after, checked: attaching may reset the handlers (audit; the
        // documentation is not explicit). Without it a Ctrl+C / Ctrl+Break would end us.
        unsafe {
            SetConsoleCtrlHandler(Some(ignore_ctrl), 1);
            FreeConsole();
            if AttachConsole(pid) == 0 {
                return None;
            }
            // the handler (Ctrl+C, Ctrl+Break, close) and the "ignore Ctrl+C" flag of the process
            if SetConsoleCtrlHandler(Some(ignore_ctrl), 1) == 0 || SetConsoleCtrlHandler(None, 1) == 0 {
                crate::util::log("Console: Ctrl handler could not be set, detached");
                SetConsoleCtrlHandler(Some(ignore_ctrl), 0);
                SetConsoleCtrlHandler(Some(ignore_ctrl), 0);
                SetConsoleCtrlHandler(None, 0);
                FreeConsole();
                return None;
            }
        }
        let conin = open("CONIN$");
        let conout = open("CONOUT$");
        if conin == INVALID_HANDLE_VALUE || conout == INVALID_HANDLE_VALUE {
            let s = Session { conin, conout };
            drop(s);
            return None;
        }
        Some(Session { conin, conout })
    }

    /// Exe names of the processes attached to the console, except ours.
    pub fn processes(&self) -> Vec<String> {
        let mut pids = [0u32; 64];
        // SAFETY: buffer size passed.
        let n = unsafe { GetConsoleProcessList(pids.as_mut_ptr(), pids.len() as u32) } as usize;
        if n == 0 || n > pids.len() {
            return vec!["?".into()]; // unknown -> the rules refuse
        }
        // SAFETY: no arguments.
        let me = unsafe { windows_sys::Win32::System::Threading::GetCurrentProcessId() };
        pids.iter()
            .take(n)
            .filter(|p| **p != me)
            .map(|p| sys::process_path(*p).unwrap_or_else(|| "?".into()))
            .collect()
    }

    pub fn input_mode(&self) -> Option<u32> {
        let mut m = 0u32;
        // SAFETY: valid handle, m written.
        if unsafe { GetConsoleMode(self.conin, &mut m) } != 0 {
            Some(m)
        } else {
            None
        }
    }

    pub fn selection_active(&self) -> bool {
        // SAFETY: structure written.
        unsafe {
            let mut si: CONSOLE_SELECTION_INFO = std::mem::zeroed();
            if GetConsoleSelectionInfo(&mut si) == 0 {
                return true; // unknown -> treat as "do not touch"
            }
            si.dwFlags & (CONSOLE_SELECTION_IN_PROGRESS | CONSOLE_SELECTION_NOT_EMPTY) != 0
        }
    }

    pub fn cursor(&self) -> Option<(i32, i32, i32)> {
        // SAFETY: structure written.
        unsafe {
            let mut bi: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            if GetConsoleScreenBufferInfo(self.conout, &mut bi) == 0 {
                return None;
            }
            Some((bi.dwCursorPosition.X as i32, bi.dwCursorPosition.Y as i32, bi.dwSize.X as i32))
        }
    }

    pub fn read(&self, x: i32, y: i32, count: usize) -> Option<String> {
        if count == 0 || count > 4096 {
            return None;
        }
        let mut buf = vec![0u16; count];
        let mut read = 0u32;
        // SAFETY: buffer of `count` units; reading wraps over lines.
        let ok = unsafe {
            ReadConsoleOutputCharacterW(self.conout, buf.as_mut_ptr(), count as u32, COORD { X: x as i16, Y: y as i16 }, &mut read)
        };
        if ok == 0 || read as usize != count {
            return None;
        }
        String::from_utf16(&buf).ok()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: handles we opened; FreeConsole detaches us.
        unsafe {
            if self.conin != INVALID_HANDLE_VALUE {
                CloseHandle(self.conin);
            }
            if self.conout != INVALID_HANDLE_VALUE {
                CloseHandle(self.conout);
            }
            FreeConsole();
            // removed after detaching (a Ctrl+C typed meanwhile still finds it); never piles up
            // (registered twice per attach); the "ignore Ctrl+C" flag is not inherited by children
            SetConsoleCtrlHandler(Some(ignore_ctrl), 0);
            SetConsoleCtrlHandler(Some(ignore_ctrl), 0);
            SetConsoleCtrlHandler(None, 0);
        }
    }
}
