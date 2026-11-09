//! Keyboard Layout Fixer (NOAM) — Rust 1.7.0. Fixes text typed with the wrong keyboard layout
//! (Hebrew <-> English) with one key (F10). Based on RavidSaadia/Keyboard_Layout_Fixer (MIT).
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod bubble;
#[cfg(windows)]
mod diagnostic;
#[cfg(windows)]
mod host;
#[cfg(windows)]
mod indicator;
#[cfg(windows)]
mod platform;
#[cfg(windows)]
mod report;
#[cfg(windows)]
mod settings_ui;
#[cfg(windows)]
mod update;
#[cfg(windows)]
mod welcome;
#[cfg(windows)]
mod util;

#[cfg(windows)]
fn main() {
    use windows_sys::Win32::UI::Controls::{InitCommonControlsEx, ICC_HOTKEY_CLASS, ICC_STANDARD_CLASSES, ICC_WIN95_CLASSES, INITCOMMONCONTROLSEX};

    // panic = "abort": write the reason in debug.log before the program stops
    std::panic::set_hook(Box::new(|info| {
        util::log(&format!("PANIC: {info}"));
        // 1.6.1: where it stopped, for a report offered at the next start (public install)
        report::note_crash(info.location());
    }));

    let arg = |name: &str| std::env::args_os().any(|a| a == name);
    // one copy per session; "--restart" waits up to 10 s for the old copy to quit
    // "--update": the old copy quits once a conversion in progress is over (up to 30 s)
    let Some(instance) = platform::sys::SingleInstance::acquire(if arg("--update") { 30_000 } else if arg("--restart") { 10_000 } else { 0 }) else {
        // "--settings" (Start menu shortcut): the running copy opens its settings window,
        // even when its tray icon is missing
        if arg("--settings") {
            app::open_settings_in_running_copy();
        }
        return;
    };

    // SAFETY: structure fully initialised.
    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_HOTKEY_CLASS | ICC_STANDARD_CLASSES | ICC_WIN95_CLASSES,
        };
        InitCommonControlsEx(&icc);
    }

    util::log(&format!("Application starting v{}-noam (Rust)", klf_core::VERSION));
    let s = app::load_settings();
    app::STATE.with(|st| *st.settings.borrow_mut() = s);
    // what the administrator imposes (registry), before the keys are registered
    app::reload_policy();

    let Some(hwnd) = app::create_main_window() else {
        util::log("Main window could not be created, exit");
        return;
    };
    if arg("--update") {
        // 1.6.4: the window exists, the program runs: the old copy may quit for good (signalled
        // before anything that could show a message)
        update::signal_started();
    }
    // the hotkey is reserved BEFORE the tray icon (the icon can take ~45 s at logon)
    if !app::register_hotkey() {
        app::announce_hotkey_unavailable();
    }
    app::ensure_tray();
    indicator::configure();
    app::post_welcome();
    report::at_start(hwnd);
    if arg("--settings") {
        settings_ui::open();
    }
    app::run_message_loop();
    util::log("Application exit");
    // 1.6.4: after an update, the lock is released so the new copy can start, then this process
    // watches it and puts the old program back if it does not start
    drop(instance);
    update::watch_after_exit();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Keyboard Layout Fixer is a Windows program. Tests: cargo test -p klf-core");
}
