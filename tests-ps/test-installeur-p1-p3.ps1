# Tests de l'installeur (audit 1.3.7, P1 et P3), executables avec PowerShell 5.1 ou 7.
# Extrait les fonctions et le bloc de sauvegarde AutoHotkey de install/Installer.ps1, sans rien installer.
$ErrorActionPreference = 'Stop'
$src = Get-Content -LiteralPath (Join-Path $PSScriptRoot '..\install\Installer.ps1') -Raw
$ok = 0; $ko = 0
function Verifie([string]$nom, [bool]$cond) { if ($cond) { $script:ok++; Write-Host "OK   $nom" } else { $script:ko++; Write-Host "ECHEC $nom" } }

# --- P3 : regle de version (meme expression que dans Installer.ps1)
$m = [regex]::Match($src, "\`$v -match '([^']+)'")
Verifie 'P3 regle trouvee dans Installer.ps1' $m.Success
$regle = $m.Groups[1].Value
$cas = [ordered]@{ '1.0.0+da9aa150c86cbd90' = $true; '1.0.0.0' = $true; '1.2.9.5' = $true; '1.2.8-noam4' = $true;
    '1.3.7-noam' = $false; '1.3.7' = $false; '1.4.0-noam' = $false; '2.0.0-noam' = $false; '2.0' = $false;
    'unknown' = $false; '' = $false; '10.2.1' = $false; '1.20.0' = $false; '11.0.0' = $false;
    '1.2' = $true; '1.1' = $true; '1.3' = $false; '1.2-noam' = $true }
foreach ($v in $cas.Keys) { Verifie "P3 '$v' force=$($cas[$v])" ((("$v") -match $regle) -eq $cas[$v]) }

# --- P1 : nom de compte
$usersRoot = 'C:\Users'
Verifie 'P1 regle de compte basee sur $usersRoot' ($src.Contains('-match "^$([regex]::Escape($usersRoot))'))
$rc = "^$([regex]::Escape($usersRoot))\\([^\\]+)\\"
foreach ($t in @(@('C:\Users\Alice\AppData\x.ahk','Alice'), @('C:\Users\Jean Paul\AppData\x.ahk','Jean Paul'), @('c:\users\bob\x.ahk','bob'), @('C:\ProgramData\Microsoft\x.ahk',$null))) {
    $got = if ($t[0] -match $rc) { $Matches[1] } else { $null }
    Verifie "P1 compte de '$($t[0])' = '$($t[1])'" ($got -eq $t[1])
}

# --- P1 : Alice et Bob ont chacun un f10-klf.ahk ; les deux sauvegardes doivent exister
$base = Join-Path ([IO.Path]::GetTempPath()) ("klf-p1-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path (Join-Path $base 'alice'), (Join-Path $base 'bob'), (Join-Path $base 'pd') -Force | Out-Null
[IO.File]::WriteAllText((Join-Path $base 'alice/f10-klf.ahk'), 'ALICE: F10::^q')
[IO.File]::WriteAllText((Join-Path $base 'bob/f10-klf.ahk'), 'BOB: F10::^x')
$ahk = @((Get-Item (Join-Path $base 'alice/f10-klf.ahk')), (Get-Item (Join-Path $base 'bob/f10-klf.ahk')))
$fonctions = [regex]::Match($src, '(?s)function Hash\(.*?\r?\nfunction Pareils[^\r\n]*').Value
$bloc = [regex]::Match($src, '(?s)        \$garde = .*?\r?\n        \}\r?\n').Value
Verifie 'P1 bloc de sauvegarde trouve' ($bloc.Length -gt 200)
$env:ProgramData = Join-Path $base 'pd'
function Get-CimInstance { }   # pas de processus AutoHotkey dans le test
. ([scriptblock]::Create($fonctions))
. ([scriptblock]::Create($bloc))
$copies = @(Get-ChildItem -LiteralPath $env:ProgramData -Recurse -File -Filter '*.ahk')
Verifie 'P1 deux sauvegardes' ($copies.Count -eq 2)
$contenus = @($copies | ForEach-Object { [IO.File]::ReadAllText($_.FullName) } | Sort-Object)
Verifie 'P1 contenu ALICE garde' ($contenus -contains 'ALICE: F10::^q')
Verifie 'P1 contenu BOB garde' ($contenus -contains 'BOB: F10::^x')
Verifie 'P1 originaux retires' (-not (Test-Path (Join-Path $base 'alice/f10-klf.ahk')) -and -not (Test-Path (Join-Path $base 'bob/f10-klf.ahk')))
$inv = @(Get-ChildItem -LiteralPath $env:ProgramData -Recurse -File -Filter 'inventaire.txt')
Verifie 'P1 inventaire avec 2 lignes' ($inv.Count -eq 1 -and @(Get-Content -LiteralPath $inv[0].FullName).Count -eq 2)


# --- P1 : copie impossible => original laisse en place
[IO.File]::WriteAllText((Join-Path $base 'alice/f10-klf.ahk'), 'ALICE 2')
$ahk = @(Get-Item (Join-Path $base 'alice/f10-klf.ahk'))
function Copy-Item { throw 'disque plein (simule)' }
Start-Sleep -Seconds 1
. ([scriptblock]::Create($bloc))
Verifie 'P1 copie ratee : original garde' (Test-Path (Join-Path $base 'alice/f10-klf.ahk'))

# --- P1 : copie faite mais original impossible a retirer => inventaire "copie seule"
Remove-Item Function:\Copy-Item
function Remove-Item { param([string]$LiteralPath, [switch]$Force) throw 'fichier verrouille (simule)' }
Start-Sleep -Seconds 1
. ([scriptblock]::Create($bloc))
Microsoft.PowerShell.Management\Remove-Item Function:\Remove-Item
$dernier = @(Get-ChildItem -LiteralPath $env:ProgramData -Recurse -File -Filter 'inventaire.txt' | Sort-Object FullName)[-1]
Verifie 'P1 retrait rate : original garde' (Test-Path (Join-Path $base 'alice/f10-klf.ahk'))
Verifie 'P1 retrait rate : inventaire "copie seule"' ((Get-Content -LiteralPath $dernier.FullName -Raw) -match 'copie seule')
Remove-Item -LiteralPath $base -Recurse -Force
Write-Host "$ok reussis, $ko echecs"
if ($ko) { exit 1 }
