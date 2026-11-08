# Keyboard Layout Fixer (NOAM version)

<p align="center"><img src="docs/logo.png" alt="Keyboard Layout Fixer logo: keys A and aleph linked by arrows" width="160"></p>

[English](#english) · [Français](#français) · [עברית](#עברית)

## English

Fixes text typed with the wrong keyboard layout (Hebrew ⇄ English) with one key: **F10**.
A small Windows tray program written in Rust: one `.exe` of about 600 KB, nothing else to install
(no .NET, no runtime), a few MB of memory. No internet connection; nothing you type is recorded (only a small technical `debug.log` in `%APPDATA%`, without the text).

- **F10**: select the text (or put the cursor at the end of the line) and press F10, the text is converted.
- **F9**: fixes text typed with Caps Lock on (`hELLO` → `Hello`).
- Your clipboard is saved and put back; password fields, administrator windows and consoles are protected.
- Settings window (Hebrew / French / English): keys, direction, careful mode, per-program rules,
  small confirmation bubble (top centre, bottom centre or near the cursor), diagnostic, export/import.
- Settings can be imposed on a whole PC by an administrator in
  `HKLM\SOFTWARE\Policies\NOAM\KeyboardLayoutFixer` (same names as `settings.json`; greyed out in the window).

**Build** (Linux or Windows, Rust toolchain from `rust-toolchain.toml`, MinGW target):
`bash tools/construire.sh` runs the tests (`cargo test -p klf-core`), checks licences and builds
`target/x86_64-pc-windows-gnu/release/KeyboardLayoutFixer.exe` with a `BUILD-INFO.txt`.

**Install**: put `KeyboardLayoutFixer.exe` next to the files of `install/`, then double-click `Installer.cmd`
(administrator rights; an older copy is closed cleanly and replaced). `Desinstaller.cmd` removes it.
Manual tests on Windows: `TESTS.md`.

## Français

Corrige un texte tapé avec le mauvais clavier (hébreu ⇄ anglais) avec une seule touche : **F10**.
Petit programme Windows (icône près de l'horloge) écrit en Rust : un seul `.exe` d'environ 600 Ko, rien
d'autre à installer. Ne va pas sur internet et n'enregistre pas ce que vous tapez.
**F9** corrige un texte tapé avec Verr. Maj allumé. Le presse-papiers est gardé et remis.
Compilation : `bash tools/construire.sh`. Installation : `install/Installer.cmd` (droits administrateur).
Réglages imposés par un administrateur : `HKLM\SOFTWARE\Policies\NOAM\KeyboardLayoutFixer`.

## עברית

תוכנה קטנה ל-Windows שמתקנת טקסט שהוקלד בשפת מקלדת שגויה (עברית ⇄ אנגלית) בלחיצה על **F10**.
**F9** מתקן טקסט שהוקלד כש-Caps Lock היה דולק. התוכנה אינה מתחברת לאינטרנט ואינה שומרת את מה שמקלידים.
התקנה: `install/Installer.cmd` (הרשאות מנהל).

## Credits and licence

Based on [Keyboard Layout Fixer](https://github.com/RavidSaadia/Keyboard_Layout_Fixer) by Ravid Saadia
(MIT licence; original documentation in `README-original.md` and `USAGE-original.md`).
NOAM version (Rust rewrite and changes) by breslev558, also under the MIT licence: see `LICENSE`.
