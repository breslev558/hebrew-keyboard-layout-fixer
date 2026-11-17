"""1.9.9: the method numbers written by hand for raw COM calls (comments « // vtable Interface::Method »
next to a `const NAME: usize = N;`) are checked against the Windows header of mingw-w64 (installed in
the CI for the Windows build). Exit code 1 if one differs, or if the header or a method is not found."""
import glob, re, sys

SOURCES = ["crates/klf-win/src/settings_ui.rs"]
HEADERS = {"IAccPropServices": "oleacc.h"}
found = glob.glob("/usr/x86_64-w64-mingw32/include/")
if not found:
    sys.exit("ARRET : en-têtes mingw-w64 introuvables (paquet gcc-mingw-w64-x86-64)")
inc = found[0]
order = {}
for itf, h in HEADERS.items():
    text = open(inc + h, encoding="utf-8", errors="replace").read()
    m = re.search(r"typedef struct %sVtbl\s*\{(.*?)END_INTERFACE" % itf, text, re.S)
    if not m:
        sys.exit(f"ARRET : {itf}Vtbl introuvable dans {h}")
    order[itf] = re.findall(r"\(\s*(?:STDMETHODCALLTYPE|WINAPI)\s*\*\s*(\w+)\s*\)", m.group(1))
bad, n = [], 0
for src in SOURCES:
    for line in open(src, encoding="utf-8"):
        m = re.search(r"const (\w+): usize = (\d+); // vtable (\w+)::(\w+)", line)
        if not m:
            continue
        name, idx, itf, meth = m.group(1), int(m.group(2)), m.group(3), m.group(4)
        n += 1
        methods = order.get(itf)
        real = methods.index(meth) if methods and meth in methods else None
        print(f"{'ok ' if real == idx else 'NON'} {src}: {name} = {idx}, {itf}::{meth} est la méthode {real}")
        if real != idx:
            bad.append(name)
if n == 0:
    sys.exit("ARRET : aucun numéro de méthode trouvé (commentaires « // vtable » changés ?)")
if bad:
    sys.exit("Numéros de méthode faux : " + ", ".join(bad))
print(f"{n} numéros de méthode COM conformes aux en-têtes Windows.")
