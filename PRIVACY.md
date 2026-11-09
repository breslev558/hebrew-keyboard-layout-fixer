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

## Files on your PC
`%APPDATA%\KeyboardLayoutFixer`: `settings.json` (your settings), `debug.log` (technical steps and
program names, never your text), `my-words.txt` and the word lists if you use *fix while typing*,
`replacements.txt` if you use *text replacements*.

Questions or problems: see [SECURITY.md](SECURITY.md) and the [issues](https://github.com/breslev558/hebrew-keyboard-layout-fixer/issues).
