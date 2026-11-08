# Keyboard Layout Fixer - installation for the CURRENT USER only (no administrator rights).
# Started by Installer.cmd. Copies KeyboardLayoutFixer.exe to %LOCALAPPDATA%\Programs\KeyboardLayoutFixer,
# starts it with Windows for this user (HKCU Run), adds a Start menu shortcut, then starts it.
# An older copy in that folder is closed cleanly and replaced; nothing is put on the desktop.
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

$Src = Join-Path $PSScriptRoot 'KeyboardLayoutFixer.exe'
$Dest = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Programs\KeyboardLayoutFixer'))
$Exe = Join-Path $Dest 'KeyboardLayoutFixer.exe'
$RunKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$Lnk = Join-Path ([Environment]::GetFolderPath('Programs')) 'Keyboard Layout Fixer.lnk'
if (-not (Test-Path -LiteralPath $Src) -or -not (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'Uninstall.ps1'))) { Fin 1 'ERROR: KeyboardLayoutFixer.exe and Uninstall.ps1 must be in the same folder. Unzip the file first (right click > Extract All).' }
Unblock-File -LiteralPath $Src -ErrorAction SilentlyContinue
if ($Dest.StartsWith('\\')) { Write-Host 'Note: your local application folder is on the network; Windows may ask for confirmation at each logon.' }

# already installed for the whole PC by an administrator: do not add a second copy
$Machine = Join-Path $env:ProgramFiles 'KeyboardLayoutFixer\KeyboardLayoutFixer.exe'
if (Test-Path -LiteralPath $Machine) { Fin 3 "Keyboard Layout Fixer is already installed for the whole PC ($Machine). Nothing was changed." }

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

# copies of THIS installation running in this session, closed cleanly (a conversion in progress ends first)
$sid = (Get-Process -Id $PID).SessionId
function Copies { @(Get-CimInstance Win32_Process -Filter "Name='KeyboardLayoutFixer.exe'" | Where-Object { $_.SessionId -eq $sid }) }
$autres = @(Copies | Where-Object { $_.ExecutablePath -and [IO.Path]::GetFullPath($_.ExecutablePath) -ine $Exe })
if ($autres.Count) { Fin 2 "Another copy of Keyboard Layout Fixer is running ($($autres[0].ExecutablePath)). Close it from its tray icon (right click > Exit), then run Installer.cmd again." }
$miennes = @(Copies | Where-Object { $_.ExecutablePath -and [IO.Path]::GetFullPath($_.ExecutablePath) -ieq $Exe })
if ($miennes.Count) {
    $ids = @($miennes | ForEach-Object { [int]$_.ProcessId })
    foreach ($i in $ids) { & "$env:SystemRoot\System32\taskkill.exe" /pid $i 2>$null | Out-Null }
    Fermer-Fenetres $ids
    for ($n = 0; $n -lt 30 -and @(Get-Process -Id $ids -ErrorAction SilentlyContinue).Count; $n++) { Start-Sleep -Seconds 1 }
    if (@(Get-Process -Id $ids -ErrorAction SilentlyContinue).Count) {
        if (-not (Question 'The old copy did not close within 30 s. Force it to close?')) { Fin 2 'The old copy is still running: nothing was changed.' }
        Get-Process -Id $ids -ErrorAction SilentlyContinue | Stop-Process -Force
        Start-Sleep -Seconds 2
    }
}

# copy: .new first, checked, then swapped (the old file is kept as .old until the end)
[void][IO.Directory]::CreateDirectory($Dest)
$New = "$Exe.new"; $Old = "$Exe.old"
$premiere = -not (Test-Path -LiteralPath $Exe)
$Arp = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\KeyboardLayoutFixer'
$arpAvant = Test-Path -LiteralPath $Arp
$arpValeurs = if ($arpAvant) { Get-ItemProperty -LiteralPath $Arp } else { $null }
# every step done is noted, to undo exactly those steps if a later one fails (audit A04)
$fait = @{ moved = $false; swapped = $false; run = $false; arp = $false; files = @() }
[IO.File]::Copy($Src, $New, $true)
try {
    if (Test-Path -LiteralPath $Old) { [IO.File]::Delete($Old) }
    if (-not $premiere) { [IO.File]::Move($Exe, $Old); $fait.moved = $true }
    [IO.File]::Move($New, $Exe); $fait.swapped = $true
    # start with Windows: set at the first installation only (an update keeps the user's choice)
    if ($premiere) {
        # never New-Item -Force on an existing key: it would empty it (all the user's startup programs)
        if (-not (Test-Path -LiteralPath $RunKey)) { New-Item -Path $RunKey | Out-Null }
        New-ItemProperty -Path $RunKey -Name 'KeyboardLayoutFixer' -Value "`"$Exe`"" -PropertyType String -Force | Out-Null
        $fait.run = $true
        # an old "disabled" mark left by a previous install would keep it from starting
        Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run' -Name 'KeyboardLayoutFixer' -ErrorAction SilentlyContinue
    }
    # uninstaller kept next to the program + entry in Windows "Installed apps" (audit A13)
    foreach ($f in 'Uninstall.ps1', 'Uninstall.cmd') {
        $from = Join-Path $PSScriptRoot $f
        if (Test-Path -LiteralPath $from) { [IO.File]::Copy($from, (Join-Path $Dest $f), $true); $fait.files += (Join-Path $Dest $f) }
    }
    $ver = (Get-Item -LiteralPath $Exe).VersionInfo.FileVersion
    if (-not $arpAvant) { New-Item -Path $Arp | Out-Null }
    $fait.arp = $true
    $ps = "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe"
    $vals = @{ DisplayName = 'Keyboard Layout Fixer'; DisplayVersion = "$ver"; Publisher = 'breslev558 (MIT, based on Ravid Saadia)'
        DisplayIcon = "$Exe,0"; InstallLocation = $Dest
        UninstallString = "`"$ps`" -NoProfile -ExecutionPolicy Bypass -File `"$(Join-Path $Dest 'Uninstall.ps1')`""
        QuietUninstallString = "`"$ps`" -NoProfile -ExecutionPolicy Bypass -File `"$(Join-Path $Dest 'Uninstall.ps1')`" -NonInteractive"
        URLInfoAbout = 'https://github.com/breslev558/hebrew-keyboard-layout-fixer' }
    foreach ($k in $vals.Keys) { New-ItemProperty -Path $Arp -Name $k -Value $vals[$k] -PropertyType String -Force | Out-Null }
    foreach ($k in 'NoModify', 'NoRepair') { New-ItemProperty -Path $Arp -Name $k -Value 1 -PropertyType DWord -Force | Out-Null }
    New-ItemProperty -Path $Arp -Name 'EstimatedSize' -Value ([int]((Get-Item -LiteralPath $Exe).Length / 1KB)) -PropertyType DWord -Force | Out-Null
    # the program is started BEFORE the old file is deleted: a failed start is undone too
    Start-Process -FilePath $Exe -WorkingDirectory $Dest
} catch {
    $err = $_.Exception.Message
    if ($fait.arp -and -not $arpAvant) { Remove-Item -LiteralPath $Arp -Recurse -Force -ErrorAction SilentlyContinue }
    elseif ($fait.arp -and $arpValeurs) { foreach ($k in 'DisplayVersion', 'EstimatedSize') { if ($null -ne $arpValeurs.$k) { Set-ItemProperty -LiteralPath $Arp -Name $k -Value $arpValeurs.$k -ErrorAction SilentlyContinue } } }
    if ($fait.run) { Remove-ItemProperty -Path $RunKey -Name 'KeyboardLayoutFixer' -ErrorAction SilentlyContinue }
    if ($premiere) { foreach ($f in $fait.files) { Remove-Item -LiteralPath $f -Force -ErrorAction SilentlyContinue } }
    if ($fait.swapped -and (Test-Path -LiteralPath $Exe)) { [IO.File]::Delete($Exe) }
    if (Test-Path -LiteralPath $New) { [IO.File]::Delete($New) }
    if (-not $premiere -and (Test-Path -LiteralPath $Old) -and -not (Test-Path -LiteralPath $Exe)) {
        [IO.File]::Move($Old, $Exe)
        Start-Process -FilePath $Exe -WorkingDirectory $Dest -ErrorAction SilentlyContinue
        Fin 1 "ERROR: $err. The previous version was put back."
    }
    if ($premiere) {
        if (-not (Get-ChildItem -LiteralPath $Dest -Force -ErrorAction SilentlyContinue)) { Remove-Item -LiteralPath $Dest -Force -ErrorAction SilentlyContinue }
        Fin 1 "ERROR: $err. Nothing was installed."
    }
    if (-not $fait.moved -and -not $fait.swapped) { Fin 1 "ERROR: $err. Nothing was changed." }
    Fin 1 "ERROR: $err. The installation is incomplete: run Installer.cmd again."
}
try { if (Test-Path -LiteralPath $Old) { [IO.File]::Delete($Old) } } catch { Write-Host "Note: the old file $Old could not be deleted (harmless)." }
# Start menu shortcut: optional (may be refused in a locked-down PowerShell)
try {
    $s = (New-Object -ComObject WScript.Shell).CreateShortcut($Lnk)
    $s.TargetPath = $Exe; $s.Arguments = '--settings'; $s.WorkingDirectory = $Dest; $s.Description = 'Keyboard Layout Fixer'
    $s.Save()
} catch { Write-Host 'Note: the Start menu shortcut could not be created (the program works anyway).' }
Fin 0 "Installed: $Exe`nF10 fixes the keyboard layout, F9 fixes Caps Lock. Settings: click the keyboard icon near the clock.`nIt starts with Windows (can be switched off in the settings). To remove it: Windows Settings > Apps > Installed apps > Keyboard Layout Fixer, or Uninstall.cmd from the program folder."
