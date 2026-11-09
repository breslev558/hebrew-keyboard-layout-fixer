# Third-party notices

## Word lists (data/words/)

`data/words/KeyboardLayoutFixer-words-he.txt` and `data/words/KeyboardLayoutFixer-words-en.txt`
are derived from **FrequencyWords** by Hermit Dave (https://github.com/hermitdave/FrequencyWords),
files `content/2018/he/he_50k.txt` and `content/2018/en/en_50k.txt` (OpenSubtitles data), and are
licensed under the Creative Commons Attribution-ShareAlike 4.0 International licence
(https://creativecommons.org/licenses/by-sa/4.0/). See `data/words/README.md`. The MIT licence of
this project does not apply to these files. They are not compiled into the program; they are
downloaded only when the optional "fix while typing" feature is switched on.

## Rust crates

The program's Rust dependencies and their licences are listed by `cargo deny check licenses`
(all permissive: MIT, Apache-2.0, Unicode-3.0 and similar).
