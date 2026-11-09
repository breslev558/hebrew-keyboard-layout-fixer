# Changelog

## 1.8.2 (November 2026)
- Fix while typing (option) now recognises **words that are in no list**: letter models (character trigrams) built on your PC from the same word lists, and **Hebrew prefixes** before a common word (וכשהלכתי). Measured on 4,874 Hebrew and 4,466 English rare words never seen by the program (ranks 45,001–50,000): 65 % and 69 % fixed when typed on the wrong layout (before: none), false proposals unchanged at 0.06 % and 0.40 %. On typos of real words (a letter dropped, two letters swapped): 0.06 % (Hebrew) and 0.26 % (English) false proposals.
- The keys `, . ; / '` count as letters inside a word (`ac,` = שבת, `tr.` = ארץ); a known word followed by a sign (`hello,`) is left alone. Words in CAPITALS (acronyms, Caps Lock) are never fixed while typing. Short words such as `kt` (לא) or `ak` (של) are fixed together with the wrong word that follows them.
- New options, off by default: a **short Windows sound** after a fix (standard system sound, silent when Windows sounds are off) and **text replacements** (your own list, `@@ = name@example.com`; empty until you write it; Backspace right after undoes).
- The tray icon turns grey, with the reason in its tooltip, while fixing while typing sleeps in the current window (code editor, excluded program, full screen).
- Latin layouts whose letters are not on the US keys (German or Swiss QWERTZ, Belgian AZERTY) now convert with their own table read from Windows (before: the US table, so y/z or a/q/m were wrong). US, UK, US-International and French AZERTY are unchanged.
- With only the "Hebrew (Standard)" SI-1452 layout installed, the keyboard now switches to Hebrew after a fix (it stayed on English).
- FAQ: quit the program before playing games with anti-cheat.

## 1.8.1 (November 2026)
- Fix while typing (option): **learns from F10** (a single word you fix with F10 is fixed at once next time) and from **Backspace** (pressed right after a fix, it undoes it and the word is never fixed again). A personal list (tray menu › My words…) has priority over the word lists.
- Never in **code editors** (VS Code, Visual Studio, JetBrains IDEs, Sublime Text, Notepad++…) by default, never on code-like words (camelCase), and the words kept are forgotten on sleep or lock.
- Fewer false proposals: short words (3–4 letters) must be common. Measured on words absent from the lists: 0.06 % (Hebrew) and 0.22 % (English) false proposals.
- New option: **double-tap Shift** to convert, like F10 (handy on laptops where F10 needs Fn); never in JetBrains IDEs, where double Shift opens their search.
- Repository: a "wrong fix" issue form, `tests/typing-cases.tsv` (real cases checked by the tests), PRIVACY.md and PERFORMANCE.md (F10 time and memory measured on every change).

## 1.8.0 (November 2026)
- New option, off by default: **suggest fixes while typing**. When the keys of the last words spell a real word on the other layout (and not on the current one), a small bubble shows the fix; Tab fixes the whole run of wrong words and switches the keyboard, any other key ignores it. Also fixes a "TWo capitals" slip (SHalom → Shalom). Never in password fields, consoles, administrator windows, full-screen games or presentations, programs the fixer is off for, or when a third keyboard layout is active. Measured on the word lists: 99 % of words typed on the wrong layout are fixed, no false proposal on correctly typed words.
- Privacy: the keys are watched (low-level keyboard hook) only while this option is on; only the last few words are kept, in memory, and forgotten on a click, arrows, Enter, a shortcut, another window or a pause. The Hebrew and English word lists (FrequencyWords, CC BY-SA 4.0, separate files) are downloaded once from this repository when the option is switched on, signed with the update key and checked each time they are read. Not available when an administrator manages the PC; importing a settings file never switches it on.
- New presentation: cover image, animated demo and FAQ in the README.

## 1.7.1 (November 2026)
- After F10, the keyboard now switches to the language of the fixed text by default, so you can keep typing straight away. It can be switched off in the settings ("Switch the keyboard language after the fix"). Settings files from earlier versions get the new default once; a choice made from 1.7.1 on is kept.

## 1.7.0 (November 2026)
- Native build for Windows on ARM (Snapdragon / Copilot+ PCs): KeyboardLayoutFixer-Setup-arm64.exe. It is built and tested on an ARM64 machine by GitHub Actions, and updates itself with its own ARM64 files. The x64 version runs as before.
- Presentation page: https://breslev558.github.io/hebrew-keyboard-layout-fixer/ (short animation of F10).
- Available with the Windows Package Manager once accepted by Microsoft: `winget install breslev558.KeyboardLayoutFixer`.

## 1.6.4 (November 2026)
- Safer updates: the update is described by a signed file that includes its version, size and SHA-256, so an older signed release can never be passed off as a newer one. The new version is kept only once it has really started (window and keys ready); if it does not start within 60 seconds, the previous version is put back and started again automatically. No more fixed waiting time.
- Copying from programs that offer dozens of private clipboard formats (Excel ranges) can no longer make F10 slow: past 300 ms, the remaining private formats are not kept (text, pictures and files always are).
- Repository: issue templates (bug, idea), code of conduct, and an end-to-end test on Windows in CI (installs the program, types "akuo" in Notepad, presses F10 and expects "שלום").

## 1.6.3 (November 2026)
- Security review (with an independent second review): a clipboard copied from a password manager (marked "do not keep") is never kept in memory for later when it cannot be put back at once; a clipboard kept for later is forgotten after 10 minutes even if the tray menu is never opened; clipboard formats holding something other than a memory block are checked before being read; the clipboard is never held open while the log is written.
- Updates: a local file problem (disk full, antivirus holding the file) is tried again at the next check instead of blocking that version for 7 days; an older copy still running is set aside instead of blocking the update.
- Repository: CodeQL code scanning, OpenSSF Scorecard, Dependabot, cargo-audit and cargo-deny (known vulnerabilities, licences, sources), strict clippy and fuzzing of the conversion core on every change; GitHub Actions pinned by commit and with minimal permissions.

## 1.6.2 (November 2026)
- All texts reviewed against Windows conventions, in Hebrew, French and English: standard button words (the settings button is now "Apply", it saves without closing the window), Hebrew commands in the imperative like Windows, French "Paramètres" and "Presse-papiers", consistent terms and agreements, clearer installer message.
- The converter choices in Hebrew no longer use arrows (they could be shown reversed).

## 1.6.1 (November 2026)
- Optional, consent-based anonymous problem reports (per-user install only). After a real error (clipboard not put back, paste failed) or a crash (offered at most once a day), a Windows notification asks "Send an anonymous report to help fix it?"; nothing is sent without that click. Tray menu › Problem reports: "Report a problem…" (optional comment, the exact content shown before sending), "Send reports without asking" (off by default) and "Never offer a report".
- A report contains only technical fields: report type, program version, Windows build, result, action, failed step, step timings, program name and window classes, kind of selection, number of characters, and the optional comment (shown before sending). Importing a settings file never turns on "Send reports without asking". Never the text you type or the clipboard, no identifier, no file path. The receiver (a small Cloudflare Worker) does not store IP addresses and keeps reports 90 days.
- Never active when an administrator manages the PC (same rule as updates).
- "About" no longer says "no internet connection" (updates and reports are optional network uses).

## 1.6.0 (November 2026)
- Single installer: download **KeyboardLayoutFixer-Setup.exe** and double-click it (per-user, no administrator rights). It closes a running copy cleanly, adds the program to the Start menu, to the startup programs and to Windows "Installed apps"; the zip stays available.
- Welcome window on first start: F10, F9, where the icon is, and "Try it now" (opens the test box); "Do not show again".
- The confirmation bubble shows what was converted (`akuo → שלום`). Shown from memory only, never logged; can be switched off in the tray menu ("Show the corrected text in the bubble") or by an administrator (`ShowPreview`).

## 1.5.5 (November 2026)
- Update in one click (per-user install only): once a day the program asks GitHub for the latest release; if a newer version exists, a bubble says so and the tray menu offers "Install version X". The exe is downloaded directly, its SHA-256 and an ECDSA P-256 signature (key kept outside GitHub) are checked by Windows, the file version must match, then it is swapped in place (old one kept until the new one starts). Can be switched off in the tray menu; never active when an administrator manages the PC. No other network access, no telemetry.

## 1.5.4 (November 2026)
- New tray icon: white A and turquoise Aleph side by side, filling the square, drawn from a vector source and adjusted pixel by pixel (16-48 px); grey when paused.
- Clipboard that could not be put back is kept in memory for 10 minutes: tray menu "Put back the previous clipboard" (only if our text is still there).
- The target window is the one in the foreground when the key is pressed; holding an arrow key during the copy stops the conversion.
- Settings: Win+F9 is kept when saving; profile import shows ReplaceCaps changes, refuses newer profile formats, and its errors are translated.
- Administrator rules: a machine rule is never lost when the user already has 200 rules.
- Public installer: exact rollback, the old version kept until the new one has started, uninstaller copied and listed in Windows "Installed apps"; the startup (Run) key is never emptied.
- Automatic checks on GitHub (tests, Windows build, PowerShell scripts).

## 1.5.3 (November 2026)
- Neutral wording ("administrator"), per-user installer without administrator rights, download link.
