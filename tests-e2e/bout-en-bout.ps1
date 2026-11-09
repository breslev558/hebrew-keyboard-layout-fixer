# Essai de bout en bout sous Windows (CI) : installe le vrai programme avec l'installeur, ouvre le
# Bloc-notes, tape « akuo » avec le clavier anglais, appuie sur F10 et vérifie que le texte devient
# « שלום ». Code de sortie 0 = réussi.
$ErrorActionPreference = 'Stop'
Add-Type @'
using System; using System.Runtime.InteropServices; using System.Text;
public static class W {
  [DllImport("user32.dll")] public static extern IntPtr LoadKeyboardLayout(string id, uint flags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr p);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  public static bool Front(IntPtr h) {
    uint me = GetCurrentThreadId(), fg = GetWindowThreadProcessId(GetForegroundWindow(), IntPtr.Zero);
    AttachThreadInput(me, fg, true); keybd_event(0x12, 0, 0, UIntPtr.Zero); keybd_event(0x12, 0, 2, UIntPtr.Zero);
    bool ok = SetForegroundWindow(h); AttachThreadInput(me, fg, false); return ok && GetForegroundWindow() == h;
  }
  [DllImport("user32.dll")] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr a, string c, string t);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, StringBuilder l);
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  public static void Key(byte vk) { keybd_event(vk, 0, 0, UIntPtr.Zero); keybd_event(vk, 0, 2, UIntPtr.Zero); }
}
'@
function Fail($m) { Write-Host "ECHEC : $m"; if (Test-Path $log) { Write-Host '--- debug.log'; Get-Content $log -Tail 30 }; exit 1 }
$dir = Join-Path $env:APPDATA 'KeyboardLayoutFixer'
$log = Join-Path $dir 'debug.log'

# 1. claviers hébreu + anglais (US) pour cet utilisateur, et pas de fenêtre de bienvenue
$l = New-WinUserLanguageList 'en-US'; $l.Add('he-IL'); Set-WinUserLanguageList $l -Force
[void][W]::LoadKeyboardLayout('0000040d', 0); [void][W]::LoadKeyboardLayout('00000409', 1)
New-Item -ItemType Directory -Force -Path $dir | Out-Null
'{ "WelcomeDone": true, "ShowBubble": false }' | Set-Content -Encoding UTF8 (Join-Path $dir 'settings.json')

# 2. installation silencieuse (le programme est lancé par l'installeur)
$p = Start-Process -FilePath '.\KeyboardLayoutFixer-Setup.exe' -ArgumentList '/S' -PassThru
if (-not $p.WaitForExit(120000)) { Fail "l'installeur ne s'est pas terminé" }
if ($p.ExitCode -ne 0) { Fail "installeur : code $($p.ExitCode)" }
$t = 0; while (-not ((Test-Path $log) -and (Select-String -Path $log -Pattern 'Hotkey registered: F10' -Quiet))) { Start-Sleep 1; if (++$t -gt 30) { Fail 'F10 non enregistrée après 30 s' } }
"OK    programme installé et lancé, F10 enregistrée"

# 3. Bloc-notes, « akuo », F10
$n = Start-Process notepad -PassThru
$t = 0; while ($n.MainWindowHandle -eq 0) { Start-Sleep -Milliseconds 300; $n.Refresh(); if (++$t -gt 50) { Fail 'Bloc-notes introuvable' } }
$h = $n.MainWindowHandle
$t = 0; while (-not [W]::Front($h)) { Start-Sleep -Milliseconds 300; if (++$t -gt 20) { Fail 'le Bloc-notes ne passe pas au premier plan' } }
# clavier anglais (US) dans le Bloc-notes lui-même : WM_INPUTLANGCHANGEREQUEST
$hkl = [W]::LoadKeyboardLayout('00000409', 0)
[void][W]::PostMessage($h, 0x0050, [IntPtr]::Zero, $hkl); Start-Sleep 1
$edit = [W]::FindWindowEx($h, [IntPtr]::Zero, 'Edit', $null)
if ($edit -eq [IntPtr]::Zero) { $edit = [W]::FindWindowEx($h, [IntPtr]::Zero, 'RichEditD2DPT', $null) }
if ($edit -eq [IntPtr]::Zero) { Fail "contrôle d'édition du Bloc-notes introuvable (Bloc-notes moderne ?)" }
function Texte { $sb = New-Object System.Text.StringBuilder 256; [void][W]::SendMessage($edit, 0x000D, [IntPtr]256, $sb); $sb.ToString() }
# la 1re touche après le passage au premier plan peut se perdre : on vérifie « akuo » avant F10
$ok = $false
for ($essai = 1; $essai -le 3 -and -not $ok; $essai++) {
  [void][W]::SendMessage($edit, 0x000C, [IntPtr]::Zero, [IntPtr]::Zero)   # WM_SETTEXT vide
  Start-Sleep -Milliseconds 500
  foreach ($vk in 0x41, 0x4B, 0x55, 0x4F) { [W]::Key([byte]$vk); Start-Sleep -Milliseconds 120 }   # a k u o
  Start-Sleep -Milliseconds 500
  $avant = Texte
  "Essai $essai, texte tapé : '$avant'"
  $ok = ($avant -eq 'akuo')
}
if (-not $ok) { Fail "impossible de taper « akuo » dans le Bloc-notes" }
Start-Sleep 1
$sw = [Diagnostics.Stopwatch]::StartNew()
[W]::Key(0x79)   # F10
# temps jusqu'au texte corrigé (mesure de performance), 4 s au plus
$cible = [string][char]0x05E9 + [char]0x05DC + [char]0x05D5 + [char]0x05DD
while ($sw.ElapsedMilliseconds -lt 4000 -and (Texte) -ne $cible) { Start-Sleep -Milliseconds 20 }
$ms = $sw.ElapsedMilliseconds
Start-Sleep 1

# 4. lecture du texte du Bloc-notes
$txt = Texte
Stop-Process -Id $n.Id -Force -ErrorAction SilentlyContinue
"Texte obtenu : '$txt'"
if ($txt -ne ([string][char]0x05E9 + [char]0x05DC + [char]0x05D5 + [char]0x05DD)) { Fail "attendu « שלום »" }
"OK    akuo + F10 = shalom (bout en bout), en $ms ms"
$pr = Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Select-Object -First 1
if ($pr) {
  $ws = [math]::Round($pr.WorkingSet64 / 1MB, 1); $pv = [math]::Round($pr.PrivateMemorySize64 / 1MB, 1); $cpu = [math]::Round($pr.TotalProcessorTime.TotalMilliseconds)
  "MESURE mémoire de travail $ws Mo ; mémoire privée $pv Mo ; processeur depuis le démarrage $cpu ms ; correction F10 $ms ms"
  if ($env:GITHUB_STEP_SUMMARY) {
    @("### Mesures (Windows, machine GitHub)", "", "| Mesure | Valeur |", "|---|---|", "| Correction F10 (akuo -> shalom), de la touche au texte corrigé | $ms ms |", "| Mémoire de travail | $ws Mo |", "| Mémoire privée | $pv Mo |", "| Temps processeur depuis le démarrage | $cpu ms |") | Out-File -Append -Encoding utf8 $env:GITHUB_STEP_SUMMARY
  }
}
exit 0
