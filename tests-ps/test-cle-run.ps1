# Test (Windows) : l'installeur ne doit JAMAIS vider la clé des programmes qui démarrent avec Windows.
# Travaille dans une clé de TEST (HKCU:\Software\KLF-Test-Run), jamais dans la vraie clé Run.
# Lancer : powershell -NoProfile -ExecutionPolicy Bypass -File tests-ps\test-cle-run.ps1
$ErrorActionPreference = 'Stop'
$k = 'HKCU:\Software\KLF-Test-Run'
$ok = 0; $ko = 0
function Verifie([string]$nom, [bool]$c) { if ($c) { $script:ok++; "OK    $nom" } else { $script:ko++; "ECHEC $nom" } }
try {
    if (Test-Path -LiteralPath $k) { Remove-Item -LiteralPath $k -Recurse -Force }
    New-Item -Path $k | Out-Null
    New-ItemProperty -Path $k -Name 'OneDrive' -Value '"C:\x\OneDrive.exe" /background' -PropertyType String | Out-Null
    New-ItemProperty -Path $k -Name 'Teams' -Value '"C:\x\Teams.exe"' -PropertyType String | Out-Null
    # le code exact de l'installeur (première installation), sur la clé de test
    $RunKey = $k; $Exe = 'C:\Users\Test\AppData\Local\Programs\KeyboardLayoutFixer\KeyboardLayoutFixer.exe'
    if (-not (Test-Path -LiteralPath $RunKey)) { New-Item -Path $RunKey | Out-Null }
    New-ItemProperty -Path $RunKey -Name 'KeyboardLayoutFixer' -Value "`"$Exe`"" -PropertyType String -Force | Out-Null
    $v = Get-ItemProperty -LiteralPath $k
    Verifie 'les autres programmes restent (OneDrive)' ($v.OneDrive -eq '"C:\x\OneDrive.exe" /background')
    Verifie 'les autres programmes restent (Teams)' ($v.Teams -eq '"C:\x\Teams.exe"')
    Verifie 'notre valeur est posée' ($v.KeyboardLayoutFixer -eq "`"$Exe`"")
    # pour mémoire : ce que faisait l'installeur public 1.5.3 (New-Item -Force sur la clé existante)
    New-Item -Path $k -Force | Out-Null
    $apres = Get-ItemProperty -LiteralPath $k -ErrorAction SilentlyContinue
    "INFO  New-Item -Force sur une clé existante : OneDrive " + $(if ($apres.OneDrive) { 'gardé' } else { 'EFFACÉ' })
} finally {
    if (Test-Path -LiteralPath $k) { Remove-Item -LiteralPath $k -Recurse -Force }
}
"$ok OK, $ko ECHEC"
exit $ko
