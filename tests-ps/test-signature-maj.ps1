# Test (Windows) : une signature de mise à jour faite par tools/signer-maj.py (ECDSA P-256, r||s en hex)
# est acceptée par la cryptographie de Windows avec la clé publique intégrée au programme, et un
# fichier modifié est refusé. Même format que klf-win/src/update.rs (CNG, BCryptVerifySignature).
$ErrorActionPreference = 'Stop'
function Octets([string]$h) { [byte[]](0..($h.Length / 2 - 1) | ForEach-Object { [Convert]::ToByte($h.Substring(2 * $_, 2), 16) }) }
$pub = Octets '8b7ed4cdbc072e6e9a0ba10fd1e02e0124f1b1d7bbab08c442dec555dd930b2d5a63f8c8d2fa21d1635ea25525f7df24caf89543fc69a339cbf906741b26fe77'
$p = New-Object System.Security.Cryptography.ECParameters
$p.Curve = [System.Security.Cryptography.ECCurve]::CreateFromValue('1.2.840.10045.3.1.7')
$q = New-Object System.Security.Cryptography.ECPoint
$q.X = $pub[0..31]; $q.Y = $pub[32..63]; $p.Q = $q
$ec = [System.Security.Cryptography.ECDsa]::Create($p)
$msg = [Text.Encoding]::ASCII.GetBytes("7e606417c18284fb4ad3ecd8a28499e25b016c15caa95a74bbc5cdae5ed1ccf2  KeyboardLayoutFixer.exe`n")
$sig = Octets '4453e11e9b7f8c4eb228f5799ac44e626478f185ef6137879b08950ab44a954d2dda788894e9341779713075b573ee8454691ce4a633e1e43af1ea7c76753aad'
$ok = 0; $ko = 0
function Verifie([string]$n, [bool]$c) { if ($c) { $script:ok++; "OK    $n" } else { $script:ko++; "ECHEC $n" } }
Verifie 'signature valide acceptée' ($ec.VerifyData($msg, $sig, [System.Security.Cryptography.HashAlgorithmName]::SHA256))
$faux = [byte[]]$msg.Clone(); $faux[0] = [byte]($faux[0] -bxor 1)
Verifie 'fichier modifié refusé' (-not $ec.VerifyData($faux, $sig, [System.Security.Cryptography.HashAlgorithmName]::SHA256))
"$ok OK, $ko ECHEC"
exit $ko
