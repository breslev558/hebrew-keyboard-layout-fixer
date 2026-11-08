#!/usr/bin/env bash
# Traceable build of the NOAM version (idea 12, 1.4.0): fixed compiler (rust-toolchain.toml),
# locked dependencies, tests, licence check, SHA-256 and a BUILD-INFO.txt next to the exe.
set -euo pipefail
cd "$(dirname "$0")/.."
# the Windows version (Properties, installer) must follow Cargo.toml (forgotten in 1.5.2)
V=$(grep -m1 '^version' crates/klf-win/Cargo.toml | cut -d'"' -f2)
for want in "\"ProductVersion\", \"$V-noam\"" "FILEVERSION ${V//./,},0" "version=\"$V.0\""; do
  grep -qF "$want" crates/klf-win/res/app.rc crates/klf-win/res/app.manifest || { echo "ARRET : res/app.rc ou app.manifest pas en $V ($want)"; exit 1; }
done
cargo test --locked -q -p klf-core
python3 tools/verifier-licences.py
cargo build --locked --release --target x86_64-pc-windows-gnu -p KeyboardLayoutFixer
EXE=target/x86_64-pc-windows-gnu/release/KeyboardLayoutFixer.exe
{
  echo "Keyboard Layout Fixer NOAM - informations de compilation"
  echo "version : $(grep -m1 '^version' crates/klf-win/Cargo.toml | cut -d'"' -f2)"
  echo "commit : $(git rev-parse HEAD 2>/dev/null || echo inconnu)$(git diff --quiet 2>/dev/null || echo ' (modifié)')"
  echo "compilateur : $(rustc --version)"
  echo "cible : x86_64-pc-windows-gnu ; dépendances : Cargo.lock (--locked)"
  echo "SHA256 de l'exe : $(sha256sum "$EXE" | cut -d' ' -f1)"
} > target/BUILD-INFO.txt
cat target/BUILD-INFO.txt
