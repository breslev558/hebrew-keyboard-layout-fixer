# Test (Windows, CI) de l'installeur unique : installation silencieuse, contrôles, désinstallation.
# Vérifie aussi qu'un AUTRE programme de démarrage (valeur de test) reste intact.
# Sans KeyboardLayoutFixer-Setup.exe (téléchargé par la CI), le test est sauté.
$ErrorActionPreference = 'Stop'
# (Start-Process -Wait attendrait aussi le programme lancé par l'installeur, qui ne s'arrête jamais)
$setup = Join-Path $PSScriptRoot '..\KeyboardLayoutFixer-Setup.exe'
if (-not (Test-Path -LiteralPath $setup)) { 'SAUTE : KeyboardLayoutFixer-Setup.exe absent'; exit 0 }
$run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$arp = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\KeyboardLayoutFixer'
$dest = Join-Path $env:LOCALAPPDATA 'Programs\KeyboardLayoutFixer'
$ok = 0; $ko = 0
function Verifie([string]$n, [bool]$c) { if ($c) { $script:ok++; "OK    $n" } else { $script:ko++; "ECHEC $n" } }
if (-not (Test-Path -LiteralPath $run)) { New-Item -Path $run | Out-Null }
New-ItemProperty -Path $run -Name 'KLF-Test-Autre' -Value '"C:\x\autre.exe"' -PropertyType String -Force | Out-Null
try {
    $p = Start-Process -FilePath $setup -ArgumentList '/S' -PassThru
    if (-not $p.WaitForExit(120000)) { throw "installeur bloqué" }
    Verifie 'installeur : code 0' ($p.ExitCode -eq 0)
    Verifie 'programme copié' (Test-Path -LiteralPath (Join-Path $dest 'KeyboardLayoutFixer.exe'))
    Verifie 'désinstalleur copié' (Test-Path -LiteralPath (Join-Path $dest 'Uninstall.exe'))
    $v = Get-ItemProperty -LiteralPath $run
    Verifie 'démarrage avec Windows posé' ($v.KeyboardLayoutFixer -eq ('"' + (Join-Path $dest 'KeyboardLayoutFixer.exe') + '"'))
    Verifie 'autre programme de démarrage intact' ($v.'KLF-Test-Autre' -eq '"C:\x\autre.exe"')
    $a = Get-ItemProperty -LiteralPath $arp -ErrorAction SilentlyContinue
    Verifie 'entrée Applications installées' ($a -and $a.DisplayName -eq 'Keyboard Layout Fixer' -and $a.UninstallString)
    Verifie 'raccourci menu Démarrer' (Test-Path -LiteralPath (Join-Path ([Environment]::GetFolderPath('Programs')) 'Keyboard Layout Fixer.lnk'))
    Start-Sleep -Seconds 3
    Verifie 'programme lancé' ([bool](Get-Process -Name KeyboardLayoutFixer -ErrorAction SilentlyContinue))
    # mise à jour par-dessus, programme en marche : elle le ferme et réussit ; le démarrage que
    # l'utilisateur a coupé reste coupé
    Remove-ItemProperty -Path $run -Name 'KeyboardLayoutFixer'
    $p = Start-Process -FilePath $setup -ArgumentList '/S' -PassThru
    if (-not $p.WaitForExit(120000)) { throw "installeur bloqué" }
    Verifie 'réinstallation par-dessus : code 0' ($p.ExitCode -eq 0)
    Verifie 'mise à jour : démarrage coupé reste coupé' ($null -eq (Get-ItemProperty -LiteralPath $run).KeyboardLayoutFixer)
    New-ItemProperty -Path $run -Name 'KeyboardLayoutFixer' -Value ('"' + (Join-Path $dest 'KeyboardLayoutFixer.exe') + '"') -PropertyType String -Force | Out-Null
    # désinstallation (le désinstalleur NSIS se relance depuis le dossier temporaire : on attend)
    $u = Start-Process -FilePath (Join-Path $dest 'Uninstall.exe') -ArgumentList '/S' -PassThru
    [void]$u.WaitForExit(60000)
    for ($i = 0; $i -lt 60 -and (Test-Path -LiteralPath $arp); $i++) { Start-Sleep -Milliseconds 500 }
    Start-Sleep -Seconds 2
    Verifie 'entrée Applications installées retirée' (-not (Test-Path -LiteralPath $arp))
    Verifie 'programme retiré' (-not (Test-Path -LiteralPath (Join-Path $dest 'KeyboardLayoutFixer.exe')))
    for ($i = 0; $i -lt 20 -and (Test-Path -LiteralPath $dest); $i++) { Start-Sleep -Milliseconds 500 }
    Verifie 'dossier supprimé' (-not (Test-Path -LiteralPath $dest))
    $v = Get-ItemProperty -LiteralPath $run
    Verifie 'démarrage retiré' ($null -eq $v.KeyboardLayoutFixer)
    Verifie 'autre programme de démarrage toujours intact' ($v.'KLF-Test-Autre' -eq '"C:\x\autre.exe"')
    Verifie 'programme arrêté' (-not (Get-Process -Name KeyboardLayoutFixer -ErrorAction SilentlyContinue))
} finally {
    Remove-ItemProperty -Path $run -Name 'KLF-Test-Autre' -ErrorAction SilentlyContinue
    Get-Process -Name KeyboardLayoutFixer -ErrorAction SilentlyContinue | Stop-Process -Force
}
"$ok OK, $ko ECHEC"
exit $ko
