# Changelog

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
