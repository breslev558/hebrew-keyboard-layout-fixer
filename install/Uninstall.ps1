# Keyboard Layout Fixer - removal for the CURRENT USER (installed by Installer.cmd, no administrator rights).
# Your settings (%APPDATA%\KeyboardLayoutFixer) are kept.
param([switch]$NonInteractive)
$ErrorActionPreference = 'Stop'
function Fin([int]$code, [string]$msg) {
    Write-Host ''; Write-Host $msg
    if (-not $NonInteractive) { Write-Host ''; Read-Host 'Press Enter to close' | Out-Null }
    exit $code
}
function Question([string]$t) { if ($NonInteractive) { return $false }; return ((Read-Host "$t (Y/N)") -match '^[yYoO]') }
if (([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Fin 1 'Please do NOT use "Run as administrator": just double-click the .cmd file (the program is installed for your own account).'
}
$Dest = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Programs\KeyboardLayoutFixer'))
$Exe = Join-Path $Dest 'KeyboardLayoutFixer.exe'
$RunKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$Lnk = Join-Path ([Environment]::GetFolderPath('Programs')) 'Keyboard Layout Fixer.lnk'

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

$sid = (Get-Process -Id $PID).SessionId
$miennes = @(Get-CimInstance Win32_Process -Filter "Name='KeyboardLayoutFixer.exe'" | Where-Object { $_.SessionId -eq $sid -and $_.ExecutablePath -and [IO.Path]::GetFullPath($_.ExecutablePath) -ieq $Exe })
if ($miennes.Count) {
    $ids = @($miennes | ForEach-Object { [int]$_.ProcessId })
    foreach ($i in $ids) { & "$env:SystemRoot\System32\taskkill.exe" /pid $i 2>$null | Out-Null }
    Fermer-Fenetres $ids
    for ($n = 0; $n -lt 30 -and @(Get-Process -Id $ids -ErrorAction SilentlyContinue).Count; $n++) { Start-Sleep -Seconds 1 }
    if (@(Get-Process -Id $ids -ErrorAction SilentlyContinue).Count) {
        if (-not (Question 'The program did not close within 30 s. Force it to close?')) { Fin 2 'The program is still running: nothing was removed.' }
        Get-Process -Id $ids -ErrorAction SilentlyContinue | Stop-Process -Force
        Start-Sleep -Seconds 2
    }
}
$v = (Get-ItemProperty -Path $RunKey -Name 'KeyboardLayoutFixer' -ErrorAction SilentlyContinue).KeyboardLayoutFixer
if ($v -and $v.Trim('"') -ieq $Exe) { Remove-ItemProperty -Path $RunKey -Name 'KeyboardLayoutFixer' -ErrorAction SilentlyContinue }
Remove-Item -LiteralPath $Lnk -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run' -Name 'KeyboardLayoutFixer' -ErrorAction SilentlyContinue
try { if (Test-Path -LiteralPath $Dest) { Remove-Item -LiteralPath $Dest -Recurse -Force } }
catch { Fin 1 "The program folder could not be deleted ($Dest): $($_.Exception.Message). Close it and run Uninstall.cmd again." }
Fin 0 'Keyboard Layout Fixer was removed (your settings in %APPDATA%\KeyboardLayoutFixer are kept).'
