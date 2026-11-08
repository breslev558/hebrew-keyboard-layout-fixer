# Keyboard Layout Fixer (NOAM version)

<p align="center"><img src="docs/logo.png" alt="Keyboard Layout Fixer logo: keys A and aleph linked by arrows" width="160"></p>

<p align="center"><a href="https://github.com/breslev558/hebrew-keyboard-layout-fixer/releases/latest/download/KeyboardLayoutFixer.zip"><b>⬇ Download for Windows (zip)</b></a> · <a href="https://github.com/breslev558/hebrew-keyboard-layout-fixer/releases/latest">all versions and SHA256</a></p>

[English](#english) · [Français](#français) · [עברית](#עברית)

## English

Typed `akuo` instead of `שלום` (or the other way round)? Select the text and press **F10**: it is fixed.
**F9** fixes text typed with Caps Lock on (`hELLO` → `Hello`). Works with Hebrew ⇄ English, also French AZERTY.
Small Windows 10/11 tray app written in Rust: one `.exe` of about 600 KB, nothing else to install, no internet
connection, nothing you type is recorded (only a small technical `debug.log` in `%APPDATA%`, without the text).

**Install (no administrator rights needed)**
1. Download the zip above, right click it > **Extract All**.
2. Double-click **Installer.cmd**. The program is copied to your user folder, starts with Windows and appears near the clock.
3. When you open Installer.cmd, Windows may show “Windows protected your PC” (SmartScreen), because the program is new and not signed by a
   paid certificate: click **More info**, then **Run anyway**. You can check the file with the SHA256 given on the
   release page (`Get-FileHash KeyboardLayoutFixer.zip` in PowerShell).

To remove it: Windows **Settings > Apps > Installed apps > Keyboard Layout Fixer > Uninstall** (or Uninstall.cmd from the zip); your settings are kept. Settings: click the A/א icon near the clock.

Other features: the clipboard is saved and put back; password fields, administrator windows and consoles are
protected; settings window in Hebrew / French / English (keys, direction, careful mode, per-program rules, bubble
position, diagnostic, export/import). An administrator can impose settings on a whole PC in
`HKLM\SOFTWARE\Policies\NOAM\KeyboardLayoutFixer` (same names as `settings.json`).

**Guarantees and limits**
- Passwords: classic Windows password fields (Edit/RichEdit with `ES_PASSWORD`) are recognised in advance; elsewhere the protection relies on the application refusing to copy. Administrator windows are left alone, unless the program itself runs as administrator.
- Clipboard: text, images, files and most formats are saved and put back. Some embedded Office object formats (OLE: Embed Source, Object Descriptor…) are not: an Office object copied before a fix may come back as text or picture. A new copy made in the same window during the one-second restore may not be told apart from the application's copy if it is identical.
- If the clipboard could not be put back, it is kept **in memory only** for 10 minutes: tray menu “Put back the previous clipboard”.
- F10 fixes the keys' layout, it does not guess the intended language; punctuation alone is not always reversible.
- The diagnostic never contains your text: program names, window classes, sizes, settings and steps only.

**Build**: `bash tools/construire.sh` (Rust toolchain from `rust-toolchain.toml`, MinGW target `x86_64-pc-windows-gnu`):
tests, licence check, `KeyboardLayoutFixer.exe` and `BUILD-INFO.txt`. The same checks run on GitHub for every change (`.github/workflows/ci.yml`). Manual tests on Windows: `TESTS.md`. Changes: `CHANGELOG.md`. Security reports: `SECURITY.md`.

## Français

Vous avez tapé `akuo` au lieu de `שלום` ? Sélectionnez le texte et appuyez sur **F10** : il est corrigé.
**F9** corrige un texte tapé avec Verr. Maj allumé. Hébreu ⇄ anglais, et clavier français AZERTY.

**Installation (sans droits administrateur)** : téléchargez le zip (lien en haut), clic droit > **Extraire tout**,
double-clic sur **Installer.cmd**. Si Windows affiche « Windows a protégé votre ordinateur » : **Informations
complémentaires**, puis **Exécuter quand même** (programme récent, sans certificat payant ; l'empreinte SHA256 est
sur la page de la version). Pour le retirer : **Uninstall.cmd** (vos réglages sont gardés).

## עברית

הקלדתם `akuo` במקום `שלום`? סמנו את הטקסט ולחצו **F10** והוא יתוקן. **F9** מתקן טקסט שהוקלד כש-Caps Lock היה דולק.
התקנה ללא הרשאות מנהל: הורידו את קובץ ה-zip (הקישור למעלה), לחצו עליו קליק ימני ובחרו **חלץ הכל**, ואז לחצו פעמיים על **Installer.cmd**.
אם מופיעה ההודעה "Windows הגן על המחשב שלך": לחצו **מידע נוסף** ואז **הפעל בכל זאת**. להסרה: **Uninstall.cmd**.

## Credits and licence

Based on [Keyboard Layout Fixer](https://github.com/RavidSaadia/Keyboard_Layout_Fixer) by Ravid Saadia
(MIT licence; original documentation in `README-original.md` and `USAGE-original.md`).
NOAM version (Rust rewrite and changes) by breslev558, also under the MIT licence: see `LICENSE`.
