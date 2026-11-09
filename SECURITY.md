# Security

Please report a security problem **privately**: open a
[private security advisory](https://github.com/breslev558/hebrew-keyboard-layout-fixer/security/advisories/new)
on GitHub (never paste text you typed or confidential content). You will get a first answer within
14 days, usually much sooner. A confirmed problem is fixed in a new release as quickly as possible
(at most 60 days), and named in [CHANGELOG.md](CHANGELOG.md) once users can update.

Supported versions: only the latest release receives fixes (the program updates itself in one click).

The program never sends or writes to disk the text it converts. Its only network uses are optional:
the daily update check (GitHub) and anonymous problem reports, sent only after the user's click
(or if they chose "Send reports without asking"), with technical fields only (see README, Privacy).
The optional "fix while typing" (off by default) installs a low-level keyboard hook only while it
is switched on, keeps only the last few words in memory (never written or sent) and downloads its
signed word lists once from this repository.
It keeps the user's
clipboard in memory only (at most 10 minutes when it could not be put back, see README).
