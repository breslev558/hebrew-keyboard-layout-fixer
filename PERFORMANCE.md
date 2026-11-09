# Performance

Keyboard Layout Fixer is a single native executable written in Rust, with no runtime to install
(no .NET, no Java) and no background polling.

| | x64 | ARM64 |
|---|---|---|
| Executable | about 740 KB | about 660 KB |
| Installer | about 470 KB | about 430 KB |

## Measured automatically on every change
The end-to-end test on Windows (GitHub Actions, `tests-e2e/bout-en-bout.ps1`) installs the real
program, types `akuo` in Notepad, presses F10 and measures:

- the time from the F10 key to the fixed text (`שלום`) on the screen;
- the memory of the running program (working set and private memory);
- the processor time it used since it started.

The latest values are in the summary of each [CI run](https://github.com/breslev558/hebrew-keyboard-layout-fixer/actions/workflows/ci.yml)
(job *bout-en-bout*, section "Mesures"). GitHub's machines are shared and slower than a usual PC,
so these numbers are an upper bound.

## What it costs when idle
- No timer runs while nothing happens, except a policy check once a minute and the optional daily
  update check.
- The optional *fix while typing* adds a low-level keyboard hook that only looks keys up in tables
  prepared beforehand (a few microseconds per key); without that option, no hook is installed.
- The conversion itself takes well under a millisecond; most of the time of an F10 fix is waiting
  for the target program to copy and paste (clipboard), which the program keeps as short as the
  target allows.
