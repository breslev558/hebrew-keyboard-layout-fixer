# Essai Windows (CI) 1.8.5, après bout-en-bout.ps1 (programme installé, en marche) :
#  1) F10 sans sélection dans une zone multi-ligne ÉTROITE qui passe à la ligne toute seule : tout le
#     paragraphe doit être corrigé, pas seulement la dernière ligne affichée ;
#  2) captures de la fenêtre de réglages (3 onglets) en hébreu, français et anglais (artefact « captures »).
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices; using System.Text;
public static class V {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr p);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  public static void Key(byte vk) { keybd_event(vk, 0, 0, UIntPtr.Zero); keybd_event(vk, 0, 2, UIntPtr.Zero); }
  public static bool Front(IntPtr h) {
    uint me = GetCurrentThreadId(), fg = GetWindowThreadProcessId(GetForegroundWindow(), IntPtr.Zero);
    AttachThreadInput(me, fg, true); keybd_event(0x12, 0, 0, UIntPtr.Zero); keybd_event(0x12, 0, 2, UIntPtr.Zero);
    bool ok = SetForegroundWindow(h); AttachThreadInput(me, fg, false); return ok && GetForegroundWindow() == h;
  }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string t);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr a, string c, string t);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, StringBuilder l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, string l);
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
}
'@
function Fail($m) { Write-Host "ECHEC : $m"; exit 1 }
$out = Join-Path (Get-Location) 'captures'; New-Item -ItemType Directory -Force -Path $out | Out-Null
$dir = Join-Path $env:APPDATA 'KeyboardLayoutFixer'
$exe = Join-Path $env:LOCALAPPDATA 'Programs\KeyboardLayoutFixer\KeyboardLayoutFixer.exe'

# --- 1) paragraphe à retour automatique (zone de texte WinForms = EDIT Win32 multi-ligne)
$form = @'
Add-Type -AssemblyName System.Windows.Forms
$f = New-Object Windows.Forms.Form; $f.Text = 'KLF-paragraphe'; $f.Width = 260; $f.Height = 260
$t = New-Object Windows.Forms.TextBox; $t.Multiline = $true; $t.WordWrap = $true; $t.Dock = 'Fill'; $t.Font = New-Object Drawing.Font('Segoe UI', 11)
$f.Controls.Add($t); [void]$f.ShowDialog()
'@
$fp = Join-Path $env:TEMP 'klf-form.ps1'; Set-Content -Path $fp -Value $form -Encoding UTF8
$pf = Start-Process powershell -ArgumentList '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $fp -PassThru
$h = [IntPtr]::Zero; $t = 0; while ($h -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 300; $h = [V]::FindWindow($null, 'KLF-paragraphe'); if (++$t -gt 60) { Fail 'fenêtre de test introuvable' } }
$e = [V]::FindWindowEx($h, [IntPtr]::Zero, $null, $null)
$gib = 'akuo kfuko nv akunfo vhuo tbh fu,c napy trul ntus fsh kcsue av,heui kuej t, fk vpxev ukt re t, vaurv vtjrubv'
[void][V]::SendMessage($e, 0x000C, [IntPtr]::Zero, $gib)                       # WM_SETTEXT
$t = 0; while (-not [V]::Front($h)) { Start-Sleep -Milliseconds 300; if (++$t -gt 20) { Fail 'fenêtre de test pas au premier plan' } }
[void][V]::SendMessage($e, 0x00B1, [IntPtr]$gib.Length, [IntPtr]$gib.Length)    # EM_SETSEL : curseur à la fin
Start-Sleep 1
[V]::Key(0x79)   # F10
$cible = -join (@(1513,1500,1493,1501,32,1500,1499,1493,1500,1501,32,1502,1492,32,1513,1500,1493,1502,1499,1501,32,1492,1497,1493,1501,32,1488,1504,1497,32,1499,1493,1514,1489,32,1502,1513,1508,1496,32,1488,1512,1493,1498,32,1502,1488,1493,1491,32,1499,1491,1497,32,1500,1489,1491,1493,1511,32,1513,1492,1514,1497,1511,1493,1503,32,1500,1493,1511,1495,32,1488,1514,32,1499,1500,32,1492,1508,1505,1511,1492,32,1493,1500,1488,32,1512,1511,32,1488,1514,32,1492,1513,1493,1512,1492,32,1492,1488,1495,1512,1493,1504,1492) | ForEach-Object { [char]$_ })
function Texte { $sb = New-Object System.Text.StringBuilder 4096; [void][V]::SendMessage($e, 0x000D, [IntPtr]4096, $sb); $sb.ToString() }
$sw = [Diagnostics.Stopwatch]::StartNew(); while ($sw.ElapsedMilliseconds -lt 6000 -and (Texte) -ne $cible) { Start-Sleep -Milliseconds 50 }
$got = Texte
Stop-Process -Id $pf.Id -Force -ErrorAction SilentlyContinue
"Paragraphe obtenu : '$got'"
if ($got -ne $cible) { Fail 'F10 sans sélection : tout le paragraphe à retour automatique devait être corrigé' }
"OK    paragraphe sur plusieurs lignes affichées corrigé en entier ($($sw.ElapsedMilliseconds) ms)"

# --- 1b) temps passé dans le crochet clavier (option « correction pendant la frappe » allumée)
Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep 1
'{ "WelcomeDone": true, "ShowBubble": false, "TypingSuggestions": true }' | Set-Content -Encoding UTF8 (Join-Path $dir 'settings.json')
Start-Process -FilePath $exe | Out-Null
Start-Sleep 4
$pf = Start-Process powershell -ArgumentList '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $fp -PassThru
$h = [IntPtr]::Zero; $t = 0; while ($h -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 300; $h = [V]::FindWindow($null, 'KLF-paragraphe'); if (++$t -gt 60) { Fail 'fenêtre de test introuvable (2)' } }
$t = 0; while (-not [V]::Front($h)) { Start-Sleep -Milliseconds 300; if (++$t -gt 20) { Fail 'fenêtre de test pas au premier plan (2)' } }
for ($i = 0; $i -lt 130; $i++) { foreach ($vk in 0x48, 0x45, 0x4C, 0x4F, 0x20) { [V]::Key([byte]$vk); Start-Sleep -Milliseconds 15 } }   # « helo » x 130
Start-Sleep 2
Stop-Process -Id $pf.Id -Force -ErrorAction SilentlyContinue
$lg = Join-Path $dir 'debug.log'
$tim = Select-String -Path $lg -Pattern 'Hook timing' | Select-Object -Last 1
if (-not $tim) { Fail 'aucune mesure du temps du crochet dans debug.log' }
"MESURE crochet clavier : $($tim.Line)"
if ($env:GITHUB_STEP_SUMMARY) { @("", "### Crochet clavier (correction pendant la frappe allumée)", "", "``$($tim.Line)``") | Out-File -Append -Encoding utf8 $env:GITHUB_STEP_SUMMARY }
if ($tim.Line -match 'max (\d+) us' -and [int]$Matches[1] -gt 100000) { Fail "le crochet a pris plus de 100 ms sur une touche" }

# --- 2) captures de la fenêtre de réglages dans les 3 langues
foreach ($lg in 'he', 'fr', 'en') {
  Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep 1
  ('{ "WelcomeDone": true, "ShowBubble": false, "UiLanguage": "' + $lg + '" }') | Set-Content -Encoding UTF8 (Join-Path $dir 'settings.json')
  Start-Process -FilePath $exe -ArgumentList '--settings' | Out-Null
  $w = [IntPtr]::Zero; $t = 0
  while ($w -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 400; $w = [V]::FindWindow('KBLayoutFixerSettings', $null); if (++$t -gt 40) { break } }
  if ($w -eq [IntPtr]::Zero) { $p = Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1; if ($p) { $w = $p.MainWindowHandle } }
  if ($w -eq [IntPtr]::Zero) { Fail "fenêtre de réglages introuvable ($lg)" }
  [void][V]::Front($w); Start-Sleep 1
  for ($tab = 1; $tab -le 3; $tab++) {
    $r = New-Object V+RECT; [void][V]::GetWindowRect($w, [ref]$r)
    $bmp = New-Object Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
    $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size); $g.Dispose()
    $bmp.Save((Join-Path $out "reglages-$lg-onglet$tab.png"), [Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    [V]::keybd_event(0x11, 0, 0, [UIntPtr]::Zero); [V]::Key(0x09); [V]::keybd_event(0x11, 0, 2, [UIntPtr]::Zero)   # Ctrl+Tab
    Start-Sleep -Milliseconds 700
  }
  "OK    captures des réglages ($lg)"
}
exit 0
