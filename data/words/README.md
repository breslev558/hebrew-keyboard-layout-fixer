# Word lists for the optional "fix while typing" feature

- `KeyboardLayoutFixer-words-he.txt`: the 40,000 most frequent Hebrew words of `content/2018/he/he_50k.txt`
- `KeyboardLayoutFixer-words-en.txt`: the 40,000 most frequent English words of `content/2018/en/en_50k.txt`

Source: **FrequencyWords** by Hermit Dave (https://github.com/hermitdave/FrequencyWords), word
frequency lists derived from OpenSubtitles data. Changes: only the word column is kept, words with
other characters than letters and apostrophes (and gershayim for Hebrew) are left out.

These two files are licensed under the **Creative Commons Attribution-ShareAlike 4.0
International** licence (https://creativecommons.org/licenses/by-sa/4.0/), like their source.
The MIT licence of this project does **not** apply to them. They are separate data files: the
program does not contain them; it downloads them (signed, from the `words-1` release of this
repository) only when the user switches the feature on.
