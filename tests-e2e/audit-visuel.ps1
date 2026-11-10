# Essai Windows (CI) 1.9.0 : AUDIT VISUEL. Après bout-en-bout.ps1 et captures-et-paragraphe.ps1
# (programme installé). Photographie toutes les fenêtres et tous les états, dans les 3 langues,
# dans le dossier « captures/audit » (artefact « captures ») :
#  bienvenue, menu de l'icône, bulle F10 en haut et en bas, bulle « dernière version » (bouton
#  Rechercher une mise à jour), réglages à 150 % et 200 % (KLF_TEST_DPI), contraste élevé, et la
#  question de la désinstallation. Rien n'échoue pour une capture manquée : chaque manque est ÉCRIT
#  (« MANQUE ») pour être vu, l'essai ne s'arrête que si le programme ne démarre plus.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices; using System.Text;
public static class A {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
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
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, string l);
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h, uint m, IntPtr w, IntPtr l, uint f, uint t, out IntPtr r);
  public static void Ask(IntPtr h, uint m, IntPtr w, IntPtr l) { IntPtr r; SendMessageTimeout(h, m, w, l, 2, 3000, out r); }   // SMTO_ABORTIFHUNG, 3 s
  [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  // first visible top-level window of class `cls` (any process when pid = 0)
  public static IntPtr Visible(string cls, uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => { var sb = new StringBuilder(64); GetClassName(h, sb, 64); uint p; GetWindowThreadProcessId(h, out p);
      if (sb.ToString() == cls && IsWindowVisible(h) && (pid == 0 || p == pid)) { found = h; return false; } return true; }, IntPtr.Zero);
    return found;
  }
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct HIGHCONTRAST { public uint cbSize; public uint dwFlags; public string lpszDefaultScheme; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern bool SystemParametersInfo(uint a, uint b, ref HIGHCONTRAST c, uint f);
  [StructLayout(LayoutKind.Sequential)] public struct HIGHCONTRASTR { public uint cbSize; public uint dwFlags; public IntPtr lpszDefaultScheme; }
  [DllImport("user32.dll")] public static extern bool SystemParametersInfo(uint a, uint b, ref HIGHCONTRASTR c, uint f);
  // current flags (to put back exactly what was there)
  public static uint HighContrastFlags() { var hc = new HIGHCONTRASTR(); hc.cbSize = (uint)Marshal.SizeOf(typeof(HIGHCONTRASTR)); SystemParametersInfo(0x0042, hc.cbSize, ref hc, 0); return hc.dwFlags; }
  public static bool HighContrast(bool on) {
    var hc = new HIGHCONTRAST(); hc.cbSize = (uint)Marshal.SizeOf(typeof(HIGHCONTRAST)); hc.dwFlags = on ? 1u : 0u; hc.lpszDefaultScheme = on ? "High Contrast Black" : null;
    return SystemParametersInfo(0x0043, hc.cbSize, ref hc, 2);   // SPI_SETHIGHCONTRAST, told to the windows, NOT written to the profile
  }
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
}
'@
$out = Join-Path (Get-Location) 'captures\audit'; New-Item -ItemType Directory -Force -Path $out | Out-Null
$dir = Join-Path $env:APPDATA 'KeyboardLayoutFixer'
$exe = Join-Path $env:LOCALAPPDATA 'Programs\KeyboardLayoutFixer\KeyboardLayoutFixer.exe'
$manques = New-Object System.Collections.ArrayList
function Manque($m) { Write-Host "MANQUE : $m"; [void]$manques.Add($m) }
function Photo($hw, $nom) {
  try {
    $r = New-Object A+RECT; [void][A]::GetWindowRect($hw, [ref]$r)
    if (($r.R - $r.L) -le 0 -or ($r.B - $r.T) -le 0) { Manque "taille nulle pour $nom"; return }
    $bmp = New-Object Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
    $g = [Drawing.Graphics]::FromImage($bmp)
    # PrintWindow draws the whole window even if it goes past the screen (200 %); screen copy otherwise
    $dc = $g.GetHdc(); $ok = [A]::PrintWindow($hw, $dc, 2); $g.ReleaseHdc($dc)
    if (-not $ok) { $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size) }
    $g.Dispose()
    $bmp.Save((Join-Path $out $nom), [Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    Write-Host "OK    $nom"
  } catch { Manque "capture $nom : $($_.Exception.Message)" }
}
function Attendre([scriptblock]$cond, $ms) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ($sw.ElapsedMilliseconds -lt $ms) { $v = & $cond; if ($v -and $v -ne [IntPtr]::Zero) { return $v }; Start-Sleep -Milliseconds 150 }
  return [IntPtr]::Zero
}
function Demarrer($json, $dpi) {
  Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep 1
  $json | Set-Content -Encoding UTF8 (Join-Path $dir 'settings.json')
  if ($dpi) { $env:KLF_TEST_DPI = "$dpi" } else { Remove-Item Env:\KLF_TEST_DPI -ErrorAction SilentlyContinue }
  $p = Start-Process -FilePath $exe -PassThru
  $m = Attendre { [A]::FindWindow('KBLayoutFixerMain', [NullString]::Value) } 10000
  if ($m -eq [IntPtr]::Zero) { Write-Host "ECHEC : le programme ne démarre pas"; exit 1 }
  Start-Sleep 1
  return $p
}
function Reglages($lg, $suffixe, $pages) {
  Start-Process -FilePath $exe -ArgumentList '--settings' | Out-Null
  $w = Attendre { [A]::FindWindow('KBLayoutFixerSettings', [NullString]::Value) } 10000
  if ($w -eq [IntPtr]::Zero) { Manque "réglages ($lg $suffixe)"; return [IntPtr]::Zero }
  [void][A]::Front($w); Start-Sleep -Milliseconds 700
  $tc = [A]::GetDlgItem($w, 240)
  foreach ($tab in $pages) {
    [A]::Ask($tc, 0x1330, [IntPtr]($tab - 1), [IntPtr]::Zero)   # TCM_SETCURFOCUS (3 s at most)
    Start-Sleep -Milliseconds 600
    Photo $w "reglages-$lg-$suffixe-onglet$tab.png"
  }
  return $w
}
function Bloc-notes($texte) {
  $n = Start-Process notepad -PassThru
  $h = Attendre { $n.Refresh(); $n.MainWindowHandle } 10000
  if ($h -eq [IntPtr]::Zero) { return $null }
  [void][A]::Front($h); Start-Sleep -Milliseconds 500
  [Windows.Forms.SendKeys]::SendWait($texte); Start-Sleep -Milliseconds 400
  return $n
}

foreach ($lg in 'he', 'fr', 'en') {
  # --- bienvenue (premier démarrage)
  $p = Demarrer ('{ "WelcomeDone": false, "ShowBubble": true, "UiLanguage": "' + $lg + '" }') $null
  $d = Attendre { [A]::Visible('#32770', [uint32]$p.Id) } 8000
  if ($d -ne [IntPtr]::Zero) { Start-Sleep -Milliseconds 400; Photo $d "bienvenue-$lg.png"; [void][A]::PostMessage($d, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) } else { Manque "bienvenue ($lg)" }
  Start-Sleep 1

  # --- menu de l'icône (le clic droit sur l'icône envoie WM_CONTEXTMENU à la fenêtre cachée)
  $m = [A]::FindWindow('KBLayoutFixerMain', [NullString]::Value)
  [void][A]::Front($m)
  [void][A]::PostMessage($m, 0x8001, [IntPtr]::Zero, [IntPtr]0x007B)   # WM_APP_TRAY, WM_CONTEXTMENU
  $menu = Attendre { [A]::Visible('#32768', 0) } 5000
  if ($menu -ne [IntPtr]::Zero) { Start-Sleep -Milliseconds 400; Photo $menu "menu-icone-$lg.png"; [void][A]::PostMessage($m, 0x001F, [IntPtr]::Zero, [IntPtr]::Zero) } else { Manque "menu de l'icône ($lg)" }   # WM_CANCELMODE closes the menu
  Start-Sleep 1

  # --- bulle F10, en haut puis en bas
  foreach ($pos in @(@(0, 'haut'), @(1, 'bas'))) {
    $p = Demarrer ('{ "WelcomeDone": true, "ShowBubble": true, "ShowPreview": true, "BubblePosition": ' + $pos[0] + ', "UiLanguage": "' + $lg + '" }') $null
    $n = Bloc-notes 'akuo'
    if (-not $n) { Manque "Bloc-notes ($lg)"; continue }
    [A]::Key(0x79)   # F10
    $b = Attendre { [A]::Visible('KBLayoutFixerBubble', 0) } 4000
    if ($b -ne [IntPtr]::Zero) { Start-Sleep -Milliseconds 250; Photo $b "bulle-f10-$($pos[1])-$lg.png" } else { Manque "bulle F10 $($pos[1]) ($lg)" }
    Stop-Process -Id $n.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep 1
  }

  # --- bouton « Rechercher une mise à jour » (page Diagnostic) : bulle de réponse
  $p = Demarrer ('{ "WelcomeDone": true, "ShowBubble": true, "UiLanguage": "' + $lg + '" }') $null
  $w = Reglages $lg 'normal' @(3)
  if ($w -ne [IntPtr]::Zero) {
    [void][A]::PostMessage($w, 0x0111, [IntPtr]264, [A]::GetDlgItem($w, 264))   # WM_COMMAND, bouton 264
    $b = Attendre { [A]::Visible('KBLayoutFixerBubble', 0) } 15000
    if ($b -ne [IntPtr]::Zero) { Start-Sleep -Milliseconds 250; Photo $b "bulle-mise-a-jour-$lg.png" } else { Manque "bulle de mise à jour ($lg)" }
    [void][A]::PostMessage($w, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
  }

  # --- réglages à 150 % et 200 % (tous les onglets)
  foreach ($dpi in 144, 192) {
    $p = Demarrer ('{ "WelcomeDone": true, "ShowBubble": false, "UiLanguage": "' + $lg + '" }') $dpi
    $w = Reglages $lg "$([int]($dpi * 100 / 96))pc" @(1, 2, 3)
    if ($w -ne [IntPtr]::Zero) { [void][A]::PostMessage($w, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }
  }
  Remove-Item Env:\KLF_TEST_DPI -ErrorAction SilentlyContinue
}

# --- contraste élevé (hébreu et français, tous les onglets) ; l'état d'avant est TOUJOURS remis
$avant = [A]::HighContrastFlags()
try {
  if ([A]::HighContrast($true)) {
    Start-Sleep 3
    if (([A]::HighContrastFlags() -band 1) -eq 0) { Manque "contraste élevé demandé mais pas appliqué" }
    foreach ($lg in 'he', 'fr') {
      $p = Demarrer ('{ "WelcomeDone": true, "ShowBubble": false, "UiLanguage": "' + $lg + '" }') $null
      $w = Reglages $lg 'contraste' @(1, 2, 3)
      if ($w -ne [IntPtr]::Zero) { [void][A]::PostMessage($w, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }
    }
  } else { Manque "contraste élevé (non activable sur cette machine)" }
} finally {
  if (($avant -band 1) -eq 0) { [void][A]::HighContrast($false) }
  Start-Sleep 2
}

# --- désinstallation : la question « effacer aussi les réglages et les mots appris ? » (réponse Non)
Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Stop-Process -Force
$un = Get-ChildItem (Split-Path $exe) -Filter '*.exe' | Where-Object { $_.Name -match 'unins' } | Select-Object -First 1
if ($un) {
  $pu = Start-Process -FilePath $un.FullName -PassThru
  for ($i = 1; $i -le 4; $i++) {
    $d = Attendre { $c = Get-CimInstance Win32_Process -Filter "ParentProcessId=$($pu.Id)" -ErrorAction SilentlyContinue; $ids = @($pu.Id) + @($c | ForEach-Object { $_.ProcessId }); foreach ($x in $ids) { $h = [A]::Visible('#32770', [uint32]$x); if ($h -ne [IntPtr]::Zero) { return $h } }; [IntPtr]::Zero } 8000
    if ($d -eq [IntPtr]::Zero) { break }
    Start-Sleep -Milliseconds 400; Photo $d "desinstallation-$i.png"
    $non = [A]::GetDlgItem($d, 7)   # IDNO : the question « also delete your settings? » gets « No »
    if ($non -ne [IntPtr]::Zero) { [void][A]::PostMessage($non, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero) }   # BM_CLICK
    elseif ([A]::Front($d)) { [A]::Key(0x0D) }   # a wizard page: Enter = its default button
    else { Manque "page de désinstallation pas au premier plan"; break }
    Start-Sleep 2
  }
  Start-Sleep 3
  & taskkill.exe /T /F /PID $pu.Id 2>&1 | Out-Null   # the uninstaller and the copy it started, only them
} else { Manque "désinstalleur introuvable" }

"Captures de l'audit : $((Get-ChildItem $out -Filter *.png).Count) ; manques : $($manques.Count)"
$manques | ForEach-Object { "  - $_"; Write-Host "::warning::audit visuel : $_" }
exit 0
