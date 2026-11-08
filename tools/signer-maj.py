"""Signe le fichier .sha256 d'une version PUBLIQUE avec la clé des mises à jour (ECDSA P-256).
Usage : python3 tools/signer-maj.py <KeyboardLayoutFixer.exe.sha256> [clé=/root/.secrets/klf-maj/maj.key]
Écrit <fichier>.sig : r||s en hexadécimal (128 caractères), le format vérifié par Windows (CNG)."""
import subprocess, sys
f = sys.argv[1]
key = sys.argv[2] if len(sys.argv) > 2 else '/root/.secrets/klf-maj/maj.key'
der = subprocess.run(['openssl', 'dgst', '-sha256', '-sign', key, f], capture_output=True, check=True).stdout
# DER : 30 len 02 lr r 02 ls s
assert der[0] == 0x30
i = 2 if der[1] < 0x80 else 2 + (der[1] & 0x7f)
def entier(i):
    assert der[i] == 0x02
    n = der[i + 1]
    v = der[i + 2:i + 2 + n].lstrip(b'\0')
    assert len(v) <= 32
    return v.rjust(32, b'\0'), i + 2 + n
r, i = entier(i)
s, _ = entier(i)
open(f + '.sig', 'w').write((r + s).hex())
# contrôle : on refait le DER depuis r||s et openssl le vérifie avec la clé publique
def d(v):
    v = v.lstrip(b'\0') or b'\0'
    if v[0] & 0x80: v = b'\0' + v
    return b'\x02' + bytes([len(v)]) + v
body = d(r) + d(s)
open(f + '.sig.der', 'wb').write(b'\x30' + bytes([len(body)]) + body)
pub = subprocess.run(['openssl', 'ec', '-in', key, '-pubout'], capture_output=True, check=True).stdout
open(f + '.pub.pem', 'wb').write(pub)
ok = subprocess.run(['openssl', 'dgst', '-sha256', '-verify', f + '.pub.pem', '-signature', f + '.sig.der', f], capture_output=True)
import os; os.remove(f + '.sig.der'); os.remove(f + '.pub.pem')
print('signature', 'OK' if ok.returncode == 0 else 'ECHEC', f + '.sig')
sys.exit(ok.returncode)
