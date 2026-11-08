# Keyboard Layout Fixer (version NOAM) - desinstallation. Lance par Desinstaller.cmd.
# Option : -NonInteractive (aucune question, rien n'est jamais force).
# Codes : 0 = tout est retire ; 1 = il reste quelque chose (liste affichee) ; 2 = programme
# encore en marche (rien retire) ; 5 = pas de droits admin.
param([switch]$NonInteractive, [switch]$Eleve)
$ErrorActionPreference = 'Stop'

function Fin([int]$code, [string]$msg) {
    Write-Host ''
    Write-Host $msg
    if (-not $NonInteractive) { Write-Host ''; Read-Host 'Appuyez sur Entree pour fermer' | Out-Null }
    exit $code
}
function Question([string]$texte) {
    if ($NonInteractive) { return $false }
    return ((Read-Host "$texte (O/N)") -match '^[oOyY]')
}

$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) {
    if ($Eleve -or $NonInteractive) { Fin 5 'ERREUR : il faut les droits administrateur.' }
    Write-Host 'Demande des droits administrateur...'
    try {
        $p = Start-Process powershell -Verb RunAs -Wait -PassThru -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"", '-Eleve')
        exit $p.ExitCode
    } catch {
        Write-Host 'Droits administrateur refuses : rien n''a ete retire.'
        exit 5
    }
}

$Dest = Join-Path $env:ProgramFiles 'KeyboardLayoutFixer'
$Exe = Join-Path $Dest 'KeyboardLayoutFixer.exe'
$Lnk = Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs\Keyboard Layout Fixer.lnk'
$RunKey = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run'
$Hkcu = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'

# encore en marche : seulement si le PID est toujours CE programme (un PID libere peut etre reattribue)
function Vivants($ids) { @(Get-Process -Id $ids -ErrorAction SilentlyContinue | Where-Object { $_.Path -ieq $Exe }) }

# taskkill sans /f n'atteint pas la fenetre cachee du programme (1.3+) :
# message de fermeture poste directement aux fenetres KBLayoutFixerMain de ces PID (session courante)
function Fermer-Fenetres($ids) {
    try {
        if (-not ('KLFClose.W' -as [type])) {
            Add-Type -Namespace KLFClose -Name W -MemberDefinition @"
[DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr p,IntPtr a,string c,string t);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
[DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h,uint m,IntPtr w,IntPtr l);
"@
        }
        $h = [IntPtr]::Zero
        while ($true) {
            $h = [KLFClose.W]::FindWindowExW([IntPtr]::Zero, $h, 'KBLayoutFixerMain', [NullString]::Value)
            if ($h -eq [IntPtr]::Zero) { break }
            $wp = [uint32]0; [void][KLFClose.W]::GetWindowThreadProcessId($h, [ref]$wp)
            if ($ids -contains [int]$wp) { [void][KLFClose.W]::PostMessageW($h, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) }
        }
    } catch { }
}

# arret propre des seules copies installees (toutes sessions), 30 s, puis question
$nos = @(Get-CimInstance Win32_Process -Filter "Name='KeyboardLayoutFixer.exe'" | Where-Object { $_.ExecutablePath -ieq $Exe })
if ($nos.Count -gt 0) {
    foreach ($p in $nos) { cmd.exe /c "taskkill /pid $($p.ProcessId) >nul 2>&1" }
    $ids = @($nos | ForEach-Object { $_.ProcessId })
    Fermer-Fenetres $ids
    for ($i = 0; $i -lt 30 -and (Vivants $ids).Count; $i++) { Start-Sleep -Seconds 1 }
    if ((Vivants $ids).Count) {
        if (-not (Question "Le programme ne s'est pas arrete en 30 s (une correction est peut-etre en cours). Forcer l'arret ?")) { Fin 2 'Le programme tourne encore : rien n''a ete retire.' }
        Vivants $ids | Stop-Process -Force
        Start-Sleep -Seconds 2
    }
}

$restes = @()
function Etape([string]$quoi, [scriptblock]$faire, [scriptblock]$encore) {
    try { & $faire } catch { }
    if (& $encore) { $script:restes += $quoi }
}
Etape 'demarrage automatique (tous les comptes)' { Remove-ItemProperty -Path $RunKey -Name 'KeyboardLayoutFixer' -ErrorAction SilentlyContinue } { [bool](Get-ItemProperty -Path $RunKey -Name 'KeyboardLayoutFixer' -ErrorAction SilentlyContinue) }
# demarrage personnel du compte qui desinstalle, seulement s'il vise ce programme
$perso = (Get-ItemProperty -Path $Hkcu -Name 'KeyboardLayoutFixer' -ErrorAction SilentlyContinue).KeyboardLayoutFixer
if ($perso -and $perso -like '*KeyboardLayoutFixer.exe*') {
    Etape "demarrage automatique personnel ($perso)" { Remove-ItemProperty -Path $Hkcu -Name 'KeyboardLayoutFixer' } { [bool](Get-ItemProperty -Path $Hkcu -Name 'KeyboardLayoutFixer' -ErrorAction SilentlyContinue) }
}
Etape 'raccourci du menu Demarrer' { Remove-Item -LiteralPath $Lnk -Force -ErrorAction SilentlyContinue } { Test-Path -LiteralPath $Lnk }
Etape "dossier $Dest" { Remove-Item -LiteralPath $Dest -Recurse -Force -ErrorAction SilentlyContinue } { Test-Path -LiteralPath $Dest }
if ($nos.Count -gt 0 -and (Vivants @($nos | ForEach-Object { $_.ProcessId })).Count) { $restes += 'programme encore en marche' }

if ($restes.Count -gt 0) { Fin 1 ("Desinstallation INCOMPLETE. Reste :`n  - " + ($restes -join "`n  - ")) }
Fin 0 "Keyboard Layout Fixer est desinstalle.`n(Les reglages personnels restent dans %APPDATA%\KeyboardLayoutFixer ; on peut effacer ce dossier.`nLes demarrages automatiques personnels des AUTRES comptes ne sont pas touches.)"
