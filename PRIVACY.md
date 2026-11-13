# Privacy

Keyboard Layout Fixer fixes text typed with the wrong keyboard layout. It is built so that what you
type stays on your PC.

## What the program never does
- It never sends what you type or copy anywhere, and never writes it to disk.
- It has no account, no advertising, no tracking and no telemetry.

## What happens when you press F10 or F9
The selected text (or the line before the cursor) is copied, converted in memory and pasted back.
Your clipboard is saved beforehand and put back afterwards. If it cannot be put back at once, it is
kept **in memory only** for 10 minutes (never if the copy came from a password manager that marks
it as "do not keep").

## Optional features and the network
All of these are optional and disabled when an administrator manages the PC.

| Feature | Default | What it sends or downloads |
|---|---|---|
| Update check | on | Once a day asks GitHub for the latest release of this project; downloads the new version only when you click Install. No identifier. GitHub sees your IP address, as for any download. |
| Anonymous problem reports | asks each time | After a real error, only if you click: report type, program version, Windows build, result, failed step and timings, program name and window classes, kind of selection, number of characters, and the comment you may write. Never your text, no identifier, no file path. The receiver (a Cloudflare Worker) stores no IP address and deletes reports after 90 days. |
| Fix while typing | off | Downloads the signed word lists once from this repository when you switch it on. |

## Fix while typing (off by default)
To suggest fixes while you type, the program watches the keys typed (a low-level keyboard hook)
**only while this option is on**. It keeps only the last few words, in memory, and forgets them on
a click, arrow keys, Enter, a shortcut, another window, sleep or lock, or a pause of 15 seconds. A
word is checked only when you type a space, so a password without spaces is never checked. It
never works in password fields it can detect, consoles, administrator windows, full-screen games or
presentations, code editors (by default), password managers, crypto wallets, remote desktop windows
or programs you turned the fixer off for.

Words you teach it (F10 on a single word, or Backspace to undo a fix) are written to
`%APPDATA%\KeyboardLayoutFixer\my-words.txt`, a plain text file you can read, edit or delete.
Words that are in no list are judged by letter models computed on your PC from the downloaded
lists: nothing extra is downloaded or sent.

## Text replacements (off and empty by default)
Same watching rules as *fix while typing*, only while this option is on: what was typed since the
last space is kept in memory, and when it is one of YOUR shortcuts it is replaced after the space.
The list is yours alone (`%APPDATA%\KeyboardLayoutFixer\replacements.txt`, empty until you write
it); it is never sent anywhere. Never in password fields, consoles, administrator windows or
programs you turned the fixer off for.

## Counting keys (1.8.5)
To switch Caps Lock off safely and to let a quick second F10 toggle the text back, the program
counts how many keys and mouse buttons are pressed (a number only, never which key or what text).
The text of the last F10 is kept in memory for 5 seconds for that toggle, then forgotten.

## F10 in Windows Terminal (1.9.5, on by default in the public version)
A terminal has no selection to copy, and Ctrl+C would stop the program running in it. So, **only
while a Windows Terminal window is in front**, the program follows the keys you type there (letters,
digits, space and signs: the key codes, never written anywhere), so that F10 knows what to fix.
- **Memory only**: locked in memory (never in the page file) and left out of crash dumps; never in
  `debug.log` (the journal only says how many characters were replaced, or why nothing was done).
- **Forgotten** at once on Enter, Tab, Esc, arrows, any other special key, Ctrl/Alt/Win (except the modifier of your F10 shortcut), a mouse
  click, another window or a locked session, and within 20 seconds when you stop typing. The text
  read on the screen and the converted text are overwritten in memory right after use.
- The program follows only the Windows Terminal window in front; if another window comes in front,
  the keys are not followed and what was kept is forgotten.
- **Passwords**: before changing anything, F10 checks on the screen (Windows accessibility) that
  exactly what you typed is shown just before the cursor. A password typed blind (sudo, ssh) or
  shown as `****` is never shown as typed: nothing is done, and it is forgotten on Enter.
- **What F10 sends**: Backspaces, then the converted text, in one go. Never Ctrl+C, never the
  clipboard, never Enter, Tab or any control character.
- **Off**: untick *Also fix in Windows Terminal* in the settings (Advanced page). On a PC managed by
  an administrator it is always off and the option is hidden.
- Not available in administrator (elevated) terminal windows.

## Files on your PC
`%APPDATA%\KeyboardLayoutFixer`: `settings.json` (your settings), `debug.log` (technical steps and
program names, never your text), `my-words.txt` and the word lists if you use *fix while typing*,
`replacements.txt` if you use *text replacements*, `one-language-programs.txt` if you use that option.

Questions or problems: see [SECURITY.md](SECURITY.md) and the [issues](https://github.com/breslev558/hebrew-keyboard-layout-fixer/issues).
