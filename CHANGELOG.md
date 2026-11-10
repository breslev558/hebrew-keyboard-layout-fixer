# Changelog

## 1.9.2 (November 2026)
- **F10 is much faster in web pages and Electron apps** (Chrome, Edge, Claude Code, Teams…): it took 1.5 to 3 seconds there, instant in Notepad. The program could not know whether text was selected and waited up to 1.5 s for a copy that never came. Now it asks Windows (UI Automation, 60 ms at most) whether text is selected: with nothing selected the line is taken at once; with a selection the wait ends as soon as the copy arrives. Without an answer, the wait is 0.4 s instead of 1.5 s, also after Shift+Home.
- Safety: a copy arriving late never meets the line selection (nothing is done then); in code editors with a terminal panel (Windsurf, Hyper, Tabby…), Ctrl+C is only sent on a selection seen by Windows (it could stop the program running in the terminal; VS Code, Cursor and VSCodium stay excluded).
- The journal notes the time of each step of F10 and the time until the paste (never any text). The Windows test measures F10 in Edge, with and without a selection, next to the previous version, with a limit of 300 ms.

## 1.9.1 (November 2026)
- F10 bubble: the "before → after" preview is drawn without its invisible direction marks, measured on the bubble itself, and ends with "…" if it is ever too wide (never cut at both ends).
- Settings, Diagnostics page: the explanation of the last attempt is in the language of the interface (it was always French), and the copied information is in English for the Hebrew and English interfaces (French for French).
- Download page: the program alone (KeyboardLayoutFixer.exe) is offered next to the installer, for antivirus programs that wrongly flag the small installer.

## 1.9.0 (November 2026)
- **Remote desktop**: while a remote desktop or virtual machine window is in front (Remote Desktop, AnyDesk, TeamViewer, RustDesk, VNC, Parsec, Hyper-V), F10 and F9 are left to it, so that the copy running on the other PC fixes the text. Before, F10 was taken here and nothing happened on either side.
- Fix while typing: in a one-line Windows text box (a form field), **Tab always moves to the next field**; it never accepts a proposal there.
- Settings, Advanced page: the delay label is short enough to be read whole (it was cut in the three languages).
- Full visual audit on Windows at every version: welcome, tray menu, F10 bubble at the top and at the bottom, update bubble, settings at 150 % and 200 %, high contrast and the uninstall question, in Hebrew, French and English.

## 1.8.9 (November 2026)
- **Check for updates** button in the About box and on the Diagnostic page of the settings: it checks at once and, if there is a newer version, installs it (same signature checks). Otherwise it says "You have the latest version". On a PC managed by an administrator it only shows the version: updates are managed there.
- F10 pressed many times in a row: **every press works**, even 20 in a row (the 1.8.6 brake after 8 presses in 10 s is removed; a key held down still counts once).
- Hebrew settings window: the diagnostic text, the excluded programs and the numbers are really written left to right now (1.8.5 tried, but the boxes kept the right-to-left mirroring).
- The list of keyboards on the Diagnostic page gives the same verdict as F10 (a Hebrew variant F10 accepts was marked ✗), and each layout keeps its sign in Hebrew.
- Windows test: 21 quick F10 presses must all be applied; the Diagnostic page and the About box are photographed in Hebrew, French and English.

## 1.8.8 (November 2026)
A full review of the whole code (not only the changes) found these; all fixed:
- The **automatic update check never ran** since 1.5.5 (two timers shared the same number); "Check for updates" in the menu worked. The build now refuses two timers with the same number.
- With 3 keyboard layouts, F10 now really uses the Latin layout chosen in the settings (it always used the last one).
- F10 without a selection that converts nothing (numbers only, several lines, clipboard busy...) no longer leaves the line selected: before, the next key typed erased it.
- A selection the application does not copy is never extended to the start of the line.
- When another program rewrites the clipboard during F10 (remote desktop, clipboard tools), your previous copy is no longer lost: "Restore the previous clipboard" in the menu puts it back for a few minutes.
- F10 without a selection also takes the whole paragraph in windows made by PowerShell scripts (WinForms text boxes).
- Fix while typing never waits for a frozen program (at most 200 ms), reads the window once per word, reads its files at most every 5 s, and rewrites a fix only if the text stays the same twice.
- An update is refused if the downloaded program is made for another kind of PC (x64 / ARM64), and it is written to disk for good before it replaces the program.
- An unreadable settings file is never replaced by the defaults; wrong values in three late settings no longer reset the whole file; a huge date in my-words.txt is refused.
- A message posted by another program can no longer make the program quit; update and report sending can no longer stay "busy"; the safety brake is kept when a setting changes; my-words.txt is always rewritten through a temporary file; a word typed after a Backspace into the previous word, a layout change or a very long word is never judged as a whole word.

## 1.8.7 (November 2026)
- Fix while typing never proposes anything in a **password field of a web page or an Electron app**: the program asks Windows' UI Automation (on a thread of its own, at most 300 ms; no answer = nothing proposed). Before, only standard Windows password boxes were recognised.
- F10 in a box with **autocompletion** (Run dialog, Explorer address bar): the suggested end selected by Windows is taken away first, so that F10 converts what you typed, not the suggestion. Only when the last key you pressed there typed a character (a selection you made yourself is kept).
- The uninstaller offers to also delete your settings and the words the program learned (no by default, never in a silent uninstall).
- F10 without a selection also fixes the **whole paragraph in classic Windows text boxes** (WinForms), not only the last line.
- Settings enforced by an administrator are now read from `HKLM\SOFTWARE\Policies\KeyboardLayoutFixer` (see For administrators in the README).
- New download page: a big download button, the exact menu labels in English, French and Hebrew, clearer privacy and learning explanations.

## 1.8.6 (November 2026)
- Fix while typing and F10 "only the wrong words" recognise more: a final letter (ך ם ן ף ץ) inside a word means English typed on the Hebrew layout; English chat words without the apostrophe (dont, youre, thats…) are never converted. Measured on real English sentences typed on the wrong layout: 71.7 % of the words fixed (61.1 % in 1.8.5), with no more false proposals (Hebrew 0.01 %, English 0.002 % of correct sentences).
- Learning is more careful: a word fixed with F10 is learned only the second time (noted `?word date` the first time, forgotten after 30 days); a round trip with F10 teaches nothing.
- A text you selected is selected again after F10 when the result is Latin text (short, one line), so that F10 again converts it back.
- Safety brakes: F10 pressed again more than 8 times in 10 s does nothing for 10 s; keyboard hooks removed by Windows 3 times in 5 minutes stop fixing while typing for 15 minutes, with a message.
- Left/right Ctrl taps do nothing in full-screen games and videos. With the one-language option, the Run dialog (Win+R) also gets the English keyboard.
- With three keyboards or more, Hebrew is converted to the last Latin layout you used (or to `PreferredLatinLayout` in settings.json).
- Tests: a precision gate on every change (fix rate and false proposals on the word lists), and a Windows test proving that a witness text typed with every option on appears in none of the program's files.

## 1.8.5 (November 2026)
- **F10 pressed again quickly** toggles at once: 1st press converted, 2nd the original text, 3rd converted again… each press is taken immediately, typed directly without the clipboard (within 5 s, same field, one line of 300 characters at most, nothing else pressed or clicked in between). Before, presses during the previous conversion were mostly lost. A round trip teaches nothing.
- **F10 without a selection takes the whole paragraph** in fields that wrap long lines (chat boxes, Electron apps, browsers), not only the last displayed line; it stops at a real line break. Not in Excel or terminals.
- New option (public version): **F10 fixes only the wrong words** in a text where you started in the right language and went on in the wrong one. Measured on real sentences half typed on the wrong layout: 90.9 % (Hebrew) and 89.8 % (English, capitals aside) fully right; correct sentences touched by mistake: 0.07 % and 0 %.
- New options (public version, off by default): **chat guard** (in WhatsApp, Telegram, Teams… a first Enter is held back with a warning when the whole sentence is in the wrong language; the next Enter sends it), **left Ctrl alone = English, right Ctrl alone = Hebrew**, **one-language programs** (Windows Terminal, PowerShell, cmd… switch to English when they come to the front; your own list).
- **Sounds**: the program now makes its own short, soft sounds (two rising notes when a text is fixed, one quieter note when it is undone or toggled back) instead of Windows sounds that sounded like errors.
- Fix while typing: technical words and brands are recognised and written their usual way (GitHub, ChatGPT, WhatsApp…); 250 ms after a fix, the result is read back where possible and the words as typed are put back if it is not exact; typed text is sent at a gentle pace (the Windows 11 Notepad lost letters).
- Keyboard hooks removed by Windows are detected and put back, with a warning on the tray icon if that fails; the time spent in the hook is measured (debug.log and the Windows test of every change).
- Hebrew interface: the diagnostic and program names are written left to right, layout names are translated, and texts mixing Hebrew and Latin keep their punctuation in place.
- The "Hebrew (Standard 2018)" layout converts with its own table. An unreadable settings file is kept as a copy before being replaced.
- New page: [THREAT_MODEL.md](THREAT_MODEL.md) (what the program sees, keeps and how risks are handled).

## 1.8.4 (November 2026)
- **Fix: the F10 preview bubble** ("before → after") could show boxes labelled FSI, LRI or PDI and put the parts in the wrong order on some PCs (since 1.6.0). It now uses only the invisible marks LRM/RLM and is always drawn left to right, each part keeping its own direction.
- Fix while typing and text replacements never act in **password managers** (KeePass, KeePassXC, Bitwarden, 1Password, LastPass, Dashlane, Proton Pass…), **crypto wallets** (Exodus, Electrum, Ledger Live…) or **remote desktop and virtual machine** windows (Remote Desktop, Windows App, AnyDesk, RustDesk, TeamViewer, Parsec, VNC, Citrix, Horizon, Hyper-V, VirtualBox, Quick Assist…; not Chrome Remote Desktop, which runs in a browser tab): their fields are not always seen as password fields, and in a remote desktop the keys go to another PC. The tray icon turns grey there; F10 still works.
- Measured on **real sentences typed correctly** (Tatoeba, used for testing only, not shipped): 212,926 Hebrew sentences (1,038,269 words): 26 with a false proposal (0.012 %); 204,056 English sentences (1,539,424 words): 5 (0.002 %). The same sentences typed on the wrong layout: 65.8 % (Hebrew) and 61.1 % (English) of the words fixed. Two fixes came from it, with no loss of fixes: a rare Hebrew word that looks like Hebrew is no longer taken for a less common English word, and "TWo capitals" is fixed only when the result is a word (`DJing` stays). Also nothing is learned from F10 in those programs, and a program whose name cannot be read is treated as unsafe.

## 1.8.3 (November 2026)
- F10 on text typed with Caps Lock on (for example `AKUO`, meant as שלום) now also switches Caps Lock off after turning it into Hebrew; before, it stayed on and the next Hebrew words came out as Latin capitals again. Only for a text entirely in Latin capitals (2 letters or more, no Hebrew in it) converted to Hebrew, and only if the setting "Turn Caps Lock off after fixing text typed in capitals" is on (default). Fixing while typing still never touches words in capitals.

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
