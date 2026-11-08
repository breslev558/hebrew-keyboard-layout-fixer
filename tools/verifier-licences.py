#!/usr/bin/env python3
"""Checks that every dependency has a permissive licence compatible with MIT (idea 12, 1.4.0).
Reads `cargo metadata --locked`; exit code 1 if a licence is missing or not on the list."""
import json, re, subprocess, sys

ALLOWED = {"MIT", "Apache-2.0", "Unlicense", "Unicode-3.0", "BSD-2-Clause", "BSD-3-Clause", "Zlib", "ISC"}

meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1", "--locked"], check=True, capture_output=True, text=True).stdout)
bad = []
for p in sorted(meta["packages"], key=lambda p: p["name"]):
    lic = p.get("license") or ""
    # "(MIT OR Apache-2.0) AND Unicode-3.0": every AND part needs one allowed licence
    parts = [x for x in re.split(r"\bAND\b", lic.replace("(", "").replace(")", "")) if x.strip()]
    ok = bool(parts) and all(any(o.strip() in ALLOWED for o in part.split("OR")) for part in parts)
    print(f"{'ok ' if ok else 'NON'} {p['name']} {p['version']} : {lic or '(aucune)'}")
    if not ok:
        bad.append(p["name"])
if bad:
    print("Licences à vérifier : " + ", ".join(bad))
    sys.exit(1)
print(f"{len(meta['packages'])} paquets, toutes les licences sont compatibles MIT.")
