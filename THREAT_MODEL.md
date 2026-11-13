# Threat model

What Keyboard Layout Fixer can see and do, what it keeps, what an attacker could try, and how each
risk is handled. Short on purpose; the details are in the code and in [PRIVACY.md](PRIVACY.md).

## What the program can see
| Feature | What it sees | When |
|---|---|---|
| F10 / F9 | the selected text (or the line before the caret), through the clipboard | only when you press the key |
| Caps Lock count | how many times Caps Lock, other keys and mouse buttons were pressed (counts only, never which key) | always, to switch Caps Lock off safely and to toggle a quick second F10 |
| Fix while typing, text replacements, double Shift, Ctrl taps, chat guard (options, off by default, public version only) | the keys typed, through a low-level keyboard hook | only while one of these options is on |
| F10 in Windows Terminal and the classic console (1.9.5/1.9.6, on by default in the public version, never on a managed PC) | the key codes typed in Windows Terminal since the last Enter / special key / click (at most 200, memory only), and the few characters just before the cursor (Windows accessibility) | only while Windows Terminal is in front; the screen only when you press F10 |

## What it keeps
- **In memory only**: the last few words typed (fix while typing), the text of the last F10 for 5 s
  (quick second F10), the saved clipboard during a conversion. All forgotten on a click, another
  window, sleep, lock or a pause.
- **On disk** (`%APPDATA%\KeyboardLayoutFixer`): `settings.json`; `debug.log` (steps, program names,
  durations, never text); `my-words.txt` (words you taught or asked never to fix, readable and
  editable); `replacements.txt` and `one-language-programs.txt` (your own lists); the word lists.
- **Never**: passwords, window titles, file paths of your documents, the clipboard history (the
  converted text is marked so that Windows keeps it out of Win+V and the cloud clipboard).

## Network
Only GitHub (updates, word lists) and, with your consent, an anonymous problem report. No typed text
is ever sent. Everything downloaded is signed with the project's key and checked before use.

## Threats and answers
| Threat | Answer |
|---|---|
| The program logs what you type (keylogger) | No text is written anywhere; the hook keeps a few words in memory; options are off by default and unavailable when an administrator manages the PC. |
| A fake update or word list | ECDSA P-256 signature (key kept offline) checked before use; SHA-256 of every file; a started update must report back within 60 s or the previous version is put back. |
| Typing into the wrong place (another window, a password field, an administrator window, a remote PC) | Window and field checked again just before acting; password fields (`ES_PASSWORD`), administrator windows, consoles, password managers, crypto wallets and remote desktop clients are left alone; text read back where possible. |
| Losing the user's clipboard | Saved in full before, put back after, only if it is still ours (sequence number); kept in memory 10 min if Windows refuses. |
| Another program faking our messages | No pointer is ever passed in a window message; results go through internal slots. |
| F10 in a terminal runs a command or erases the wrong text | Only Backspaces and printable one-column characters are sent, in one SendInput (never Enter, Tab, Ctrl+C, the clipboard); the typed text must be shown exactly before the cursor (read on the screen) just before the keys; any key or click of the user, a selection, another window or the command palette stops it; nothing is done when the screen cannot be read. Residual risk: in a one-key prompt (`[y/N]`) or a full-screen program, a typed character can be a command; the screen check makes it unlikely. |
| The test switch of the Windows test (`KLF_TEST_TERMINAL=1` in the environment of the program) | It only lets keys injected with one fixed mark count as typed, for the automatic Windows test. A program able to set it can already inject keys as the same user; it never logs or sends anything. |
| Attaching to a classic console (1.9.6) | Only to the console of the window in front (checked after attaching), only when this program has no console of its own, for a fraction of a second; Ctrl+C and Ctrl+Break are ignored meanwhile; closing that console at that very moment ends this program, which starts again by itself. |
| A terminal password kept by the program | The followed keys are never turned into text before F10, are locked in memory and left out of crash dumps, and are wiped on Enter, a click, another window, a lock or within 20 s; a password is not shown as typed, so F10 never touches it. The journal never says how many characters a refused text had. |
| A slow hook removed by Windows | The time spent in the hook is measured; a removed hook is detected and put back (at most every 30 s), with a warning on the tray icon if it fails. |
| Settings changed by a user on a managed PC | The administrator's values (`HKLM\SOFTWARE\Policies\...`) win; a settings file never turns on key watching or sending reports. |

## Not covered
- Password fields that are not standard Windows controls (some browsers) are not always detected.
- Remote desktop shown in a browser tab (Chrome Remote Desktop) is not recognised.
- A program run as administrator cannot be fixed (Windows blocks our keys; a message says so).

Report a security problem privately: see [SECURITY.md](SECURITY.md).
