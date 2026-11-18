# Technical details

Back to the [README](../README.md). This page keeps the details for people who want to check, install in a special way, manage many PCs or build the program. In version 2.0.0 the options that used to be in the icon menu are in the settings window (tabs **While typing**, **Programs**, **Advanced**, **Help**).

## How F10 works

- **F10** converts the selected text to the other layout. With nothing selected, it converts from the cursor back to the start of the line (the whole paragraph in chat boxes and browsers).
- **F10 again** within 5 seconds, in the same field and with nothing typed in between, puts the original text back; a third press converts it again.
- **F9** fixes text typed with Caps Lock on (`hELLO wORLD` → `Hello World`) and turns Caps Lock off.
- A small bubble shows what changed (`akuo → שלום`).
- F10 works by copy and paste: your clipboard is saved first and put back afterwards. Password fields, administrator windows and consoles are left alone.
- It lives in the tray (the icon near the clock). Once installed, it is a single executable under 1 MB, written in Rust, with nothing else to install (no .NET). Native builds for x64 and ARM64.
- Supported keyboards: Hebrew (including Hebrew Standard 2018) with any Latin layout: US, UK, French AZERTY; German or Swiss QWERTZ and Belgian AZERTY use a table read from Windows. With three layouts or more, Hebrew is converted to the Latin layout you used last. Other layouts (Russian, Arabic...) are refused rather than guessed.

## Install

1. [Download **KeyboardLayoutFixer-Setup.exe**](https://github.com/breslev558/hebrew-keyboard-layout-fixer/releases/latest/download/KeyboardLayoutFixer-Setup.exe) and double-click it. On a Windows on ARM PC (Snapdragon, Copilot+), use [KeyboardLayoutFixer-Setup-arm64.exe](https://github.com/breslev558/hebrew-keyboard-layout-fixer/releases/latest/download/KeyboardLayoutFixer-Setup-arm64.exe).
2. If Windows shows **"Windows protected your PC"**, click **More info**, then **Run anyway**. (The program is not yet signed with a recognised certificate; a free [SignPath](https://signpath.org) signature has been requested.)
3. No administrator rights are needed: the program is installed for your account only, starts with Windows, and shows a short welcome window with a **Try it now** box.

To update by hand, run a newer installer over the old one: the running copy is closed cleanly and your settings are kept.

### Check the download

- **SHA-256**: in PowerShell, run `Get-FileHash KeyboardLayoutFixer-Setup.exe` and compare the result with the `.sha256` file on the [release page](https://github.com/breslev558/hebrew-keyboard-layout-fixer/releases/latest).
- **Build provenance**: the x64 files are built by GitHub Actions from this repository, with a provenance attestation. With the GitHub CLI: `gh attestation verify KeyboardLayoutFixer-Setup.exe --repo breslev558/hebrew-keyboard-layout-fixer`.
- **Program alone**: download [KeyboardLayoutFixer.exe](https://github.com/breslev558/hebrew-keyboard-layout-fixer/releases/latest/download/KeyboardLayoutFixer.exe), put it in a folder of your choice (for example `%LOCALAPPDATA%\Programs\KeyboardLayoutFixer`) and double-click it. To start it with Windows, tick **Start when the computer starts** in the **General** tab. Some antivirus programs wrongly flag the small installer; the program itself has never been flagged.
- **Without the installer**: download the [zip](https://github.com/breslev558/hebrew-keyboard-layout-fixer/releases/latest/download/KeyboardLayoutFixer.zip) (x64), extract it and double-click **Installer.cmd**.
- **Where it goes**: the program in `%LOCALAPPDATA%\Programs\KeyboardLayoutFixer`, your settings in `%APPDATA%\KeyboardLayoutFixer`.
- **ARM64**: the ARM64 version updates itself with its own ARM64 files.

## Uninstall

**Windows Settings › Apps › Installed apps** (on Windows 10: **Apps & features**) › **Keyboard Layout Fixer** › **Uninstall**. The program, its startup entry and its shortcut are removed. The uninstaller asks whether to also delete your settings and the words the program learned (default: no).

## Updates

Once a day the program asks GitHub whether a newer version exists. No identifier is sent; GitHub sees your IP address, as with any download. If there is a new version, a bubble tells you and the menu offers to install it. Nothing is installed without your click. To check at once, use **Check for updates** in the **Help** tab: a newer version is installed right away, otherwise it says you have the latest one.

Before switching, the program checks the new file's SHA-256 and a digital signature (ECDSA P-256) made with a key kept offline, using Windows cryptography. If anything fails, or the new version does not start within 60 seconds, the previous version is put back automatically. To turn the daily check off, untick **Check automatically** in the **Help** tab.

## Optional features

All are off by default. They are not available when settings are enforced by an administrator (see [For administrators](#for-administrators)).

- **Suggest fixes while typing (Tab)**: when the last words you typed are real words on the *other* layout, a small bubble offers the fix. **Tab** fixes them and switches the keyboard; any other key dismisses it. It learns from you (**My words...**).
- **F10 fixes only the wrong words**: you started in the right language and went on in the wrong one? One F10 fixes only the words typed on the wrong layout. It uses the word lists; if no word looks wrong, everything is converted as usual.
- **Chat apps: hold back a sentence in the wrong language**: in WhatsApp, Telegram, Teams, Signal, Discord, Slack and similar apps, the first Enter is held back with a warning when the whole sentence is in the wrong language. Press Enter again to send it anyway. The program never presses Enter itself.
- **Left Ctrl = English, right Ctrl = Hebrew**: a short tap on Ctrl alone switches the keyboard. Any other key or a click cancels it.
- **Programs that always use one language (e.g. Terminal in English)**: Windows Terminal, PowerShell, cmd and the Run box (Win+R) switch to English when they come to the front. Edit the list in **Choose the programs...**: one `program.exe` per line, or `program.exe = he` for Hebrew.
- **Double-tap Shift to convert (like F10)**: handy on laptops where F10 needs the Fn key. Never with Ctrl, Alt or Win held, in full-screen games, or in JetBrains IDEs (where double Shift opens their search).
- **Text shortcuts (e.g. @@ → email address)**: your own shortcuts in **My shortcuts...** (`%APPDATA%\KeyboardLayoutFixer\replacements.txt`), one `shortcut = text` per line, for example `@@ = name@example.com`. Type the shortcut, then Space, and it is replaced; **Backspace** right after undoes it. The list is empty until you write it.
- **Short sound when a word is fixed**: a soft sound of the program's own.

### More about fixing while typing

- **How it decides**: a word is offered only when its keys spell a known word on the other layout and not on the current one. Names, brands, typos and words in CAPITALS are left alone. Rare words and Hebrew words with prefixes (וכשהלכתי) are recognised by letter models built on your PC from the same word lists. Short words such as `kt` (לא) are fixed together with the wrong word after them. The keys `, . ; / '` count as letters inside a word (`ac,` = שבת).
- **Learning**: a word you fix with F10 is noted the first time and learned the second time, on another day; a note not confirmed within 30 days is forgotten. **Backspace** right after a fix undoes it, and that word is never fixed again. Both lists are in **My words...** (a plain text file on your PC that you can edit).
- **Where it never acts**: password fields (including those of web pages and Electron apps), consoles, administrator windows, full-screen games and presentations, code editors (VS Code, Visual Studio, JetBrains..., by default), password managers, crypto wallets, remote desktop windows, and programs you disabled it in. The tray icon turns grey while it sleeps, and its tooltip says why.
- **What it reads**: while this option is on (or **Text shortcuts**, **Double-tap Shift**, **Left Ctrl = English, right Ctrl = Hebrew** or **Chat apps: hold back...**), the program watches the keys typed through a low-level keyboard hook. It keeps only the last few words, in memory, and forgets them on a click, arrow keys, Enter, a shortcut, another window, sleep, lock or a pause. What you type is never sent and never written to disk (only the words you teach it go to **My words...**).
- **Word lists**: the Hebrew and English lists (40,000 words each, see [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)) are downloaded once from this repository when you switch the option on. They are signed and checked before use.
- **Wrong or missing fix?** Use the [wrong fix form](https://github.com/breslev558/hebrew-keyboard-layout-fixer/issues/new?template=wrong-fix.yml). Real cases are added to `tests/typing-cases.tsv` and checked by the tests.

## Does it watch my keyboard?

With only F10 and F9, a minimal hook **counts** key presses (a number, never which key) so that Caps Lock can be turned off safely and a quick second F10 can put the text back. The options that read the keys (**Suggest fixes while typing**, **Text shortcuts**, **Double-tap Shift**, **Left Ctrl = English, right Ctrl = Hebrew**, **Chat apps: hold back...**) do so only while they are on, keep the last few words in memory only, and never send them. See [THREAT_MODEL.md](../THREAT_MODEL.md).

## Privacy in detail

- What you type is never sent anywhere. Nothing is written to disk except the words you teach it (**My words...**) and your own shortcuts, which stay on your PC. No account, no ads, no telemetry.
- The network is used in three optional cases only: the **daily update check** (GitHub), the **word lists** for fixing while typing (downloaded once), and **anonymous problem reports**, sent only if you click.
- All network use is turned off when settings are enforced by an administrator.

Details: [PRIVACY.md](../PRIVACY.md) and [THREAT_MODEL.md](../THREAT_MODEL.md).

### What a problem report contains

After a real error, a Windows notification asks *"Send an anonymous report to help fix it?"*. Nothing is sent unless you click it. A report contains only: report type, program version, Windows build, result, action (F10 or F9), the step that failed and step timings, the program name and window classes (for example `winword.exe`, `OpusApp`), the kind of selection, the number of characters, and the comment you may write in **Report a problem...** (you see the exact content before sending).

Never your text or clipboard, no file path, no identifier. The receiver (a small Cloudflare Worker) does not store IP addresses and deletes reports after 90 days. In the **Help** tab, **When something goes wrong** lets you choose: ask me (default), send reports without asking, or never offer a report.

The technical report (**Help** tab) shows program names, window classes, text sizes, settings and steps, never your text. It stays on your PC unless you copy it.

## Security

- Report a security problem **privately**: see [SECURITY.md](../SECURITY.md).
- Every change is checked on GitHub: tests on Windows (including a real F10 in Notepad), CodeQL, cargo-audit and cargo-deny, strict Clippy and fuzzing. OpenSSF Scorecard runs every week.
- Updates and word lists are signed, and the program checks them before use.
- What the program can see and keep, and how each risk is handled: [THREAT_MODEL.md](../THREAT_MODEL.md).

## Limitations

- F10 recognises password fields in advance only in standard Windows controls. Elsewhere, protection relies on the program blocking copy. (Fixing while typing also recognises the password fields of web pages and Electron apps.)
- Text in programs running as administrator cannot be converted: Windows blocks the simulated keys.
- Most clipboard contents (text, pictures, files) are put back. Some embedded Office objects (OLE) may come back as text or a picture.
- If the clipboard cannot be put back at once (another program keeps it busy), it is kept **in memory only** for 10 minutes: use **Put back the previous clipboard** in the icon menu.
- F10 converts key positions: it does not guess the intended language, and punctuation alone is not always reversible.
- Only Hebrew with English or French AZERTY is fully supported (see supported keyboards above).

## More FAQ

- **Windows or my antivirus warns about the program**: until the program is signed with a recognised certificate, SmartScreen shows *Windows protected your PC*, and a few antivirus programs based on machine learning may flag the unsigned installer. Check the SHA-256 against the release page, or build it yourself from source. The program also uses a low-level keyboard hook (see above), which some antivirus programs dislike.
- **F10 does nothing in one program**: in a remote desktop window (Remote Desktop, AnyDesk, TeamViewer, RustDesk...), F10 is left to the other PC: install Keyboard Layout Fixer there too. Some programs (administrator windows, games) do not accept simulated keys. Check whether the fixer is paused or disabled in that program, and see the technical report in the **Help** tab.
- **Online games with anti-cheat**: quit the fixer (icon menu › **Exit**, not just pause) before playing and start it again afterwards.

## For administrators

Settings can be enforced for every account under `HKLM\SOFTWARE\Policies\KeyboardLayoutFixer`, which standard users cannot change. The names are the same as in `settings.json`:

- `REG_DWORD` 0/1: `RequireSelection`, `TurnOffCapsLock`, `ConsoleSupport`, `LastWordOnly`, `Indicator`, `ShowBubble`, `ShowPreview`, `PlaySound`, `SwitchLanguageAfterConvert`, `CheckUpdates`.
- `REG_DWORD` values: `MaxCharacterLimit`, `PasteDelayMs` (100–10000), `MixedTextMode` (0–2), `BubblePosition` (0 top, 1 bottom, 2 cursor), `HotkeyModifiers` and `HotkeyKey`, `CaseHotkeyModifiers` and `CaseHotkeyKey`.
- `REG_SZ`: `UiLanguage` (`auto`, `he`, `fr`, `en`).
- Subkeys: `AppRules` (one value per program: `Disabled` or `SelectionOnly`) and `ExcludedProcesses`.

Enforced settings are greyed out in the settings window and never written to the user's file. When this key exists, updates, problem reports and the optional features are not available.

## Build from source

Requirements: Rust (the version is pinned in `rust-toolchain.toml`), the MinGW-w64 linker (`x86_64-w64-mingw32-gcc`), Bash and Python 3.

```
rustup target add x86_64-pc-windows-gnu
bash tools/construire.sh
```

The script runs the tests, checks the dependency licences and builds `target/x86_64-pc-windows-gnu/release/KeyboardLayoutFixer.exe` with a `BUILD-INFO.txt`. The installer is built with NSIS:

```
makensis -DVERSION=x.y.z -DEXE=<path to the exe> -DICON=crates/klf-win/res/klf.ico installer/KeyboardLayoutFixer.nsi
```

Releases are built by [`.github/workflows/release.yml`](../.github/workflows/release.yml). Manual checks on Windows are listed in [TESTS.md](../TESTS.md).

## Project files

[CHANGELOG.md](../CHANGELOG.md) · [PRIVACY.md](../PRIVACY.md) · [THREAT_MODEL.md](../THREAT_MODEL.md) · [SECURITY.md](../SECURITY.md) · [PERFORMANCE.md](../PERFORMANCE.md) · [CONTRIBUTING.md](../CONTRIBUTING.md) · [CODE_SIGNING_POLICY.md](../CODE_SIGNING_POLICY.md) · [audits/](../audits) · [original project (Ravid Saadia)](https://github.com/RavidSaadia/Keyboard_Layout_Fixer), documentation archived in [docs/archive](archive)
