# Essai Windows (CI) 1.9.6 : F10 dans la CONSOLE CLASSIQUE (conhost), après bout-en-bout.ps1.
# conhost.exe est lancé directement (la machine peut avoir Windows Terminal comme terminal par défaut).
# Le programme est lancé avec KLF_TEST_TERMINAL=1 : seules les touches injectées par CET essai avec la
# marque 0x4B4C4654 comptent comme tapées par l'utilisateur.
# Cas : correction (Read-Host) + aller-retour, mot de passe en étoiles (rien), Tab avant F10 (rien),
# F8 (rien), clic pendant (annulé), cmd « set /p » (correction). Le texte final est relu dans un fichier.
$ErrorActionPreference = 'Stop'
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class T {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr p);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code, uint type);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr LoadKeyboardLayout(string id, uint f);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
  public static readonly UIntPtr Mark = new UIntPtr(0x4B4C4654);
  // a key typed "by the user" for the program (test mark), key-down then key-up
  public static void Typed(byte vk) { byte sc = (byte)MapVirtualKey(vk, 0); keybd_event(vk, sc, 0, Mark); keybd_event(vk, sc, 2, Mark); System.Threading.Thread.Sleep(40); }
  public static void Word(string s) { foreach (char c in s.ToUpper()) Typed((byte)c); }
  public static void Key(byte vk) { keybd_event(vk, 0, 0, UIntPtr.Zero); keybd_event(vk, 0, 2, UIntPtr.Zero); }
  public static void English(IntPtr h) { PostMessage(h, 0x0050, IntPtr.Zero, LoadKeyboardLayout("00000409", 1)); }
  public static void F10ThenClick(IntPtr h) { RECT r; GetWindowRect(h, out r); keybd_event(0x79, 0, 0, UIntPtr.Zero); System.Threading.Thread.Sleep(10); SetCursorPos((r.L + r.R) / 2, (r.T + r.B) / 2); mouse_event(2, 0, 0, 0, UIntPtr.Zero); mouse_event(4, 0, 0, 0, UIntPtr.Zero); System.Threading.Thread.Sleep(150); keybd_event(0x79, 0, 2, UIntPtr.Zero); }
  // marked too (an unmarked injected key would only break the run: the palette check must be the reason, avis Fable)
  public static void CtrlShiftP() { keybd_event(0x11, 0, 0, Mark); keybd_event(0x10, 0, 0, Mark); keybd_event(0x50, 0, 0, Mark); keybd_event(0x50, 0, 2, Mark); keybd_event(0x10, 0, 2, Mark); keybd_event(0x11, 0, 2, Mark); }
  public static bool Front(IntPtr h) {
    if (GetForegroundWindow() == h) return true;   // no Alt sent for nothing (avis Fable)
    uint me = GetCurrentThreadId(), fg = GetWindowThreadProcessId(GetForegroundWindow(), IntPtr.Zero);
    AttachThreadInput(me, fg, true); keybd_event(0x12, 0, 0, UIntPtr.Zero); keybd_event(0x12, 0, 2, UIntPtr.Zero);
    bool ok = SetForegroundWindow(h); AttachThreadInput(me, fg, false); return ok && GetForegroundWindow() == h;
  }
}
'@
function Fail($m) {
  Write-Host "ECHEC : $m"
  # (nettoyage, avis Fable : la console de l'essai fermée, le programme relancé sans la marque d'essai)
  if ($script:proc) { Stop-Process -Id $script:proc.Id -Force -ErrorAction SilentlyContinue }
  Remove-Item Env:KLF_TEST_TERMINAL -ErrorAction SilentlyContinue
  Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Process (Join-Path $env:LOCALAPPDATA 'Programs\KeyboardLayoutFixer\KeyboardLayoutFixer.exe')
  exit 1
}
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$log = Join-Path $env:APPDATA 'KeyboardLayoutFixer\debug.log'
$exe = Join-Path $env:LOCALAPPDATA 'Programs\KeyboardLayoutFixer\KeyboardLayoutFixer.exe'
# le programme : réglages de l'essai, terminal activé, marque d'essai acceptée
Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 1
'{ "WelcomeDone": true, "ShowBubble": false, "SwitchLanguageAfterConvert": false, "PasteDelayMs": 100, "SettingsVersion": 4, "UiLanguage": "en", "TerminalSupport": true }' |
  Set-Content -Encoding UTF8 (Join-Path $env:APPDATA 'KeyboardLayoutFixer\settings.json')
$env:KLF_TEST_TERMINAL = '1'
Start-Process $exe; Start-Sleep 3
if (-not (Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue)) { Fail 'le programme ne démarre pas' }
$out = Join-Path $env:TEMP 'klf-console-out.txt'
$results = @()

Add-Type @'
using System; using System.Runtime.InteropServices;
public static class W { [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string t); }
'@
# ouvre une console conhost (titre fixe) sur un script ou une commande ; ferme seulement la précédente de l'essai
$titre = 'KLF-CONSOLE-ESSAI'
function Ouvrir($script, $cmd = $false) {
  if ($script:proc) { Stop-Process -Id $script:proc.Id -Force -ErrorAction SilentlyContinue; Start-Sleep 1 }
  Write-Host "  (ouverture de la console)"
  Remove-Item $out -ErrorAction SilentlyContinue
  if ($cmd) {
    $bat = Join-Path $env:TEMP 'klf-console.cmd'
    Set-Content -Encoding ASCII $bat ("@echo off`r`ntitle $titre`r`n" + $script)
    $script:proc = Start-Process conhost.exe -ArgumentList "cmd.exe /d /c `"$bat`"" -PassThru
  } else {
    $ps1 = Join-Path $env:TEMP 'klf-console.ps1'
    Set-Content -Encoding UTF8 $ps1 ("`$Host.UI.RawUI.WindowTitle = '$titre'; " + $script)
    $script:proc = Start-Process conhost.exe -ArgumentList "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$ps1`"" -PassThru
  }
  $script:h = [IntPtr]::Zero; $t = 0
  while ($script:h -eq [IntPtr]::Zero) {
    Start-Sleep -Milliseconds 500
    $script:h = [W]::FindWindow('ConsoleWindowClass', $titre)
    if (++$t -gt 40) { Fail 'fenêtre de console (ConsoleWindowClass) introuvable' }
  }
  Start-Sleep 3   # invite affichée
  $t = 0; while (-not [T]::Front($script:h)) { Start-Sleep -Milliseconds 300; if (++$t -gt 20) { Fail 'console pas au premier plan' } }
  [T]::English($script:h); Start-Sleep -Milliseconds 800
  [T]::Typed(0x11); Start-Sleep -Milliseconds 300   # Ctrl seul, marqué : départ propre
}
function Journal($n0) { @(Get-Content $log -ErrorAction SilentlyContinue) | Select-Object -Skip $n0 }
function Lignes($n0) { Journal $n0 | Where-Object { $_ -match 'Terminal|User input|changed|nothing done|Console' } | ForEach-Object { Write-Host "       $(($_ -split '\] ', 2)[1])" } }
function F10() { [T]::Key(0x79); Start-Sleep -Milliseconds 1500 }
function Entree() { [T]::Key(0x0D); Start-Sleep 2 }
function Lu() { if (Test-Path $out) { (Get-Content -Encoding UTF8 $out -Raw).Trim() } else { '(rien écrit)' } }
$shalom = [string][char]0x05E9 + [char]0x05DC + [char]0x05D5 + [char]0x05DD
$lire = '$x = Read-Host "T"; Set-Content -Encoding UTF8 "' + $out + '" $x'

Write-Host "Programme lancé (KLF_TEST_TERMINAL=1)"
# 1. correction, puis aller-retour (2e et 3e F10)
$n0 = @(Get-Content $log).Count   # (from before the window opens: « keys followed » must appear)
Ouvrir $lire
[T]::Word('akuo'); Start-Sleep -Milliseconds 500
F10
$j = Journal $n0
Lignes $n0
if (-not ($j -match 'Console: 4 characters replaced by 4')) { Fail '1er F10 : pas de remplacement dans la console' }
if ($j -match 'not confirmed') { Fail '1er F10 : remplacement non confirmé à l''écran' }
$n1 = @(Get-Content $log).Count; F10; F10
if (@(Journal $n1 | Where-Object { $_ -match 'Console: 4 characters replaced by 4' }).Count -ne 2) { Lignes $n1; Fail '2e/3e F10 : pas d''aller-retour' }
Entree
$v = Lu; Write-Host "MESURE console, correction + aller-retour : texte lu '$v'"
if ($v -ne $shalom) { Fail "texte attendu « $shalom », obtenu « $v »" }
if ($j -match 'Ctrl\+C|Ctrl\+V|clipboard saved') { Fail 'le presse-papiers ou Ctrl+C a servi' }
$results += 'OK    console : correction + aller-retour'

# 2. mot de passe caché (Read-Host -AsSecureString affiche des *) : rien n'est fait
Ouvrir ('$s = Read-Host "P" -AsSecureString; $p = [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($s)); Set-Content -Encoding UTF8 "' + $out + '" $p')
$n0 = @(Get-Content $log).Count
[T]::Word('akuo'); Start-Sleep -Milliseconds 500; F10
$j = Journal $n0; Lignes $n0
if ($j -match 'characters replaced') { Fail 'mot de passe : du texte a été remplacé' }
if (-not ($j -match 'not seen before the cursor|without echo')) { Fail 'mot de passe : refus attendu « not seen before the cursor »' }
Entree
if ((Lu) -ne 'akuo') { Fail "mot de passe modifié : '$(Lu)'" }
$results += 'OK    console : mot de passe caché, rien fait'

# 3. Tab avant F10 : le suivi est effacé, rien n'est fait
Ouvrir $lire
$n0 = @(Get-Content $log).Count
[T]::Word('akuo'); [T]::Typed(0x09); Start-Sleep -Milliseconds 500; F10
$j = Journal $n0; Lignes $n0
if ($j -match 'characters replaced') { Fail 'Tab : du texte a été remplacé' }
if (-not ($j -match 'Console: nothing followed')) { Fail 'Tab : refus attendu « nothing followed »' }
Entree
$results += 'OK    console : Tab avant F10, rien fait'

# 4b. F8 (PowerShell remplace la ligne par l'historique) avant F10 : le suivi est effacé, rien
Ouvrir $lire
$n0 = @(Get-Content $log).Count
[T]::Word('akuo'); [T]::Typed(0x77); Start-Sleep -Milliseconds 500; F10
$j = Journal $n0; Lignes $n0
if ($j -match 'characters replaced') { Fail 'F8 : du texte a été remplacé' }
if (-not ($j -match 'Console: nothing followed')) { Fail 'F8 : refus attendu « nothing followed »' }
Entree
$results += 'OK    console : F8 avant F10, rien fait'

# 5. clic pendant la correction : annulée (3 essais : jamais de remplacement après un clic vu)
$annule = $false
foreach ($e in 1..3) {
  Ouvrir $lire
  $n0 = @(Get-Content $log).Count
  [T]::Word('akuo'); Start-Sleep -Milliseconds 500
  [T]::F10ThenClick($script:h); Start-Sleep 2
  $j = Journal $n0; Lignes $n0
  if ($j -match 'characters replaced') { Fail "clic pendant : texte remplacé malgré le clic (essai $e)" }
  if ($j -match 'User input during the fix|nothing followed') { $annule = $true; Entree; break }
  Entree
}
if (-not $annule) { Fail 'clic pendant la correction : jamais annulée (3 essais)' }
$results += 'OK    console : clic pendant la correction, annulée'

# 6. cmd : « set /p » lit une ligne (mode ligne avec écho) : correction
Ouvrir ("set /p x=T: `r`n> `"$out`" echo %x%") $true
$n0 = @(Get-Content $log).Count
[T]::Word('akuo'); Start-Sleep -Milliseconds 500; F10
$j = Journal $n0; Lignes $n0
if (-not ($j -match 'Console: 4 characters replaced by 4')) { Fail 'cmd : pas de remplacement' }
Entree
$v = Lu; Write-Host "MESURE console cmd : texte lu '$v' (page de code OEM : l'hébreu peut s'écrire « ? » dans le fichier ; seul le journal compte ici)"
if ($v -eq 'akuo') { Fail 'cmd : le texte n''a pas changé' }
$results += 'OK    console : cmd set /p, corrigé'

# 7. cmd : du texte après le curseur (le texte tapé l'écraserait) : refus, la ligne reste intacte
Ouvrir ("set /p x=T: `r`n> `"$out`" echo %x%") $true
$n0 = @(Get-Content $log).Count
[T]::Word('zz'); [T]::Typed(0x25); [T]::Word('akuo'); Start-Sleep -Milliseconds 500; F10
$j = Journal $n0; Lignes $n0
if ($j -match 'characters replaced') { Fail 'texte après le curseur : texte remplacé' }
if (-not ($j -match 'text after the cursor')) { Fail 'texte après le curseur : refus attendu « text after the cursor »' }
Entree
if ((Lu) -ne 'zakuoz') { Fail "texte après le curseur : ligne modifiée '$(Lu)'" }
$results += 'OK    console : texte après le curseur, rien fait'

# 8. bord droit : « akuo » à cheval sur deux lignes : corrigé et confirmé
Ouvrir ('$x = Read-Host ("T" * ($Host.UI.RawUI.BufferSize.Width - 5)); Set-Content -Encoding UTF8 "' + $out + '" $x')
$n0 = @(Get-Content $log).Count
[T]::Word('akuo'); Start-Sleep -Milliseconds 500; F10
$j = Journal $n0; Lignes $n0
if (-not ($j -match 'Console: 4 characters replaced by 4')) { Fail 'bord droit : pas de remplacement' }
if ($j -match 'not confirmed') { Fail 'bord droit : non confirmé' }
Entree
if ((Lu) -ne $shalom) { Fail "bord droit : texte attendu « $shalom », obtenu « $(Lu) »" }
$results += 'OK    console : texte à cheval sur deux lignes, corrigé'

if ($script:proc) { Stop-Process -Id $script:proc.Id -Force -ErrorAction SilentlyContinue }
Remove-Item Env:KLF_TEST_TERMINAL
Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Process $exe
$results
exit 0
