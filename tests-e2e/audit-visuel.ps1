# Essai Windows (CI) 1.9.0 : AUDIT VISUEL. Après bout-en-bout.ps1 et captures-et-paragraphe.ps1
# (programme installé). Photographie toutes les fenêtres et tous les états, dans les 3 langues,
# dans le dossier « captures/audit » (artefact « captures ») :
#  bienvenue, menu de l'icône, bulle F10 en haut et en bas, bulle « dernière version » (bouton
#  Rechercher une mise à jour), réglages à 150 % et 200 % (KLF_TEST_DPI), contraste élevé, et la
#  question de la désinstallation. Rien n'échoue pour une capture manquée : chaque manque est ÉCRIT
#  (« MANQUE ») pour être vu, l'essai ne s'arrête que si le programme ne démarre plus.
# 1.9.9 (audit du 17/11) : trois VERIFICATIONS qui font echouer l'essai (« DEFAUT », sortie 1 a la fin) :
#  texte coupe ou controle hors de la fenetre dans les reglages (3 langues, 100/150/200 %), controle
#  sans nom pour un lecteur d'ecran (UI Automation), bulle qui prend le premier plan ou le focus.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
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
  // 1.9.9: text cut in a single-line control, or a control outside the window's client area
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr p, EnumProc f, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int i);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)] public struct SIZE { public int cx, cy; }
  [DllImport("gdi32.dll")] public static extern IntPtr SelectObject(IntPtr dc, IntPtr o);
  [DllImport("gdi32.dll", CharSet=CharSet.Unicode)] public static extern bool GetTextExtentPoint32(IntPtr dc, string s, int n, out SIZE z);
  [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
  [DllImport("user32.dll")] public static extern int MapWindowPoints(IntPtr from, IntPtr to, ref RECT r, int n);
  [DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr o);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int DrawText(IntPtr dc, string s, int n, ref RECT r, uint f);
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct LOGFONT { public int lfHeight, lfWidth, lfEscapement, lfOrientation, lfWeight; public byte lfItalic, lfUnderline, lfStrikeOut, lfCharSet, lfOutPrecision, lfClipPrecision, lfQuality, lfPitchAndFamily; [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string lfFaceName; }
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct NONCLIENTMETRICS { public uint cbSize; public int iBorderWidth, iScrollWidth, iScrollHeight, iCaptionWidth, iCaptionHeight; public LOGFONT lfCaptionFont; public int iSmCaptionWidth, iSmCaptionHeight; public LOGFONT lfSmCaptionFont; public int iMenuWidth, iMenuHeight; public LOGFONT lfMenuFont, lfStatusFont, lfMessageFont; public int iPaddedBorderWidth; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern bool SystemParametersInfoForDpi(uint a, uint b, ref NONCLIENTMETRICS m, uint f, uint dpi);
  [DllImport("gdi32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr CreateFontIndirect(ref LOGFONT lf);
  // the font of the settings window, made again HERE (a font handle of the program means nothing in
  // this process, avis Fable): the message font for `dpi`, as settings_ui.rs make_fonts does
  public static IntPtr SettingsFont(uint dpi) {
    var m = new NONCLIENTMETRICS(); m.cbSize = (uint)Marshal.SizeOf(typeof(NONCLIENTMETRICS));
    if (!SystemParametersInfoForDpi(0x0029, m.cbSize, ref m, 0, dpi)) return IntPtr.Zero;
    return CreateFontIndirect(ref m.lfMessageFont);
  }
  public static string Cut(IntPtr w, uint dpi) {
    var bad = new StringBuilder();
    IntPtr font = SettingsFont(dpi);
    if (font == IntPtr.Zero) return "police des reglages non recreee; ";
    // the client area in screen coordinates; a mirrored (Hebrew) window gives its corners swapped
    RECT pc; GetClientRect(w, out pc); MapWindowPoints(w, IntPtr.Zero, ref pc, 2);
    int cl = Math.Min(pc.L, pc.R), cr2 = Math.Max(pc.L, pc.R), ct = Math.Min(pc.T, pc.B), cb = Math.Max(pc.T, pc.B);
    EnumChildWindows(w, (h, l) => {
      if (!IsWindowVisible(h)) return true;
      var cn = new StringBuilder(64); GetClassName(h, cn, 64); string cls = cn.ToString();
      var tb = new StringBuilder(512); GetWindowText(h, tb, 512); string txt = tb.ToString().Replace("&", "");
      RECT r; GetWindowRect(h, out r);
      if (r.R - r.L > 0 && (r.L < cl - 1 || r.T < ct - 1 || r.R > cr2 + 1 || r.B > cb + 1))
        bad.AppendFormat("{0} '{1}' hors de la fenetre ({2},{3},{4},{5}); ", cls, txt, r.L - cl, r.T - ct, r.R - cl, r.B - ct);
      if (txt.Length == 0) return true;
      int st = GetWindowLong(h, -16), extra = -1;
      if (cls == "Static" && (st & 0x4000) == 0 && (st & 0x2000) != 0) {          // SS_EDITCONTROL: wraps; lines below the box are cut
        IntPtr dc2 = GetDC(h); IntPtr old2 = SelectObject(dc2, font); RECT cr0; GetClientRect(h, out cr0);
        RECT need = new RECT(); need.R = cr0.R; DrawText(dc2, txt, txt.Length, ref need, 0x0400 | 0x0010 | 0x2000 | 0x0800);   // DT_CALCRECT | DT_WORDBREAK | DT_EDITCONTROL | DT_NOPREFIX
        SelectObject(dc2, old2); ReleaseDC(h, dc2);
        if (need.B > cr0.B + 1 || need.R > cr0.R + 1) bad.AppendFormat("{0} '{1}' coupe ({2} px de haut pour {3}); ", cls, txt, need.B, cr0.B);
        return true;
      }
      if (cls == "Static" && (st & 0x4000) != 0) extra = 0;                       // SS_ENDELLIPSIS: cut silently
      else if (cls == "Button" && (st & 0x2000) == 0) {                            // not BS_MULTILINE
        int t = st & 0xF;
        if (t == 2 || t == 3 || t == 4 || t == 5 || t == 6 || t == 9) extra = GetSystemMetrics(71) + 4;   // box + gap
        else if (t == 0 || t == 1) extra = 6;
      }
      if (extra < 0) return true;
      IntPtr dc = GetDC(h); IntPtr old = SelectObject(dc, font); SIZE z; GetTextExtentPoint32(dc, txt, txt.Length, out z); SelectObject(dc, old); ReleaseDC(h, dc);
      RECT cr; GetClientRect(h, out cr);
      if (z.cx + extra > cr.R + 2) bad.AppendFormat("{0} '{1}' coupe ({2}+{3} px pour {4}); ", cls, txt, z.cx, extra, cr.R);
      return true;
    }, IntPtr.Zero);
    DeleteObject(font);
    return bad.ToString();
  }
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int ht, uint f);
  [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr h);
  [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr h, IntPtr dc);
  [DllImport("gdi32.dll")] public static extern bool BitBlt(IntPtr d, int x, int y, int w, int h, IntPtr s, int sx, int sy, uint rop);
  // what is on the screen, layered (transparent) windows included: SRCCOPY | CAPTUREBLT
  // (.NET's CopyFromScreen refuses that combination in PowerShell 5.1, seen in the 2nd 1.9.0 trial)
  public static System.Drawing.Bitmap Screen(int x, int y, int w, int h) {
    var bmp = new System.Drawing.Bitmap(w, h);
    using (var g = System.Drawing.Graphics.FromImage(bmp)) {
      IntPtr dst = g.GetHdc(); IntPtr src = GetDC(IntPtr.Zero);
      BitBlt(dst, 0, 0, w, h, src, x, y, 0x00CC0020 | 0x40000000);
      ReleaseDC(IntPtr.Zero, src); g.ReleaseHdc(dst);
    }
    return bmp;
  }
}
'@
$out = Join-Path (Get-Location) 'captures\audit'; New-Item -ItemType Directory -Force -Path $out | Out-Null
# a bigger screen, so that the 200 % window fits (Windows Server cmdlet; the default is often 1024x768)
try { Set-DisplayResolution -Width 1920 -Height 1080 -Force -ErrorAction Stop; Start-Sleep 3 } catch { Write-Host "écran : résolution non changée ($($_.Exception.Message))" }
$sc = [Windows.Forms.Screen]::PrimaryScreen.Bounds; Write-Host "écran : $($sc.Width) x $($sc.Height)"
$dir = Join-Path $env:APPDATA 'KeyboardLayoutFixer'
$env:KLF_TEST_CAPTURE = '1'   # the bubbles are normally excluded from screenshots (privacy): shown here for the audit
$exe = Join-Path $env:LOCALAPPDATA 'Programs\KeyboardLayoutFixer\KeyboardLayoutFixer.exe'
$manques = New-Object System.Collections.ArrayList
function Manque($m) { Write-Host "MANQUE : $m"; [void]$manques.Add($m) }
$defauts = New-Object System.Collections.ArrayList
function Defaut($m) { Write-Host "DEFAUT : $m"; [void]$defauts.Add($m) }
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
# 1.9.9: every control reachable from the keyboard has a name for a screen reader
function Noms($w, $quoi) {
  try {
    $el = [Windows.Automation.AutomationElement]::FromHandle($w)
    $tous = $el.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    $types = @('ControlType.Edit', 'ControlType.ComboBox', 'ControlType.CheckBox', 'ControlType.Button', 'ControlType.RadioButton', 'ControlType.TabItem', 'ControlType.Spinner', 'ControlType.Slider', 'ControlType.Hyperlink')
    $vus = 0; $d0 = $defauts.Count
    # (1st 1.9.9 trial: nothing seen on any page: what UI Automation gives is written down)
    Write-Host "       UIA $quoi : $($tous.Count) element(s) sous la fenetre"
    $tous | Select-Object -First 12 | ForEach-Object { $q = $_.Current; Write-Host "         $($q.ControlType.ProgrammaticName) | focus $($q.IsKeyboardFocusable) | hors ecran $($q.IsOffscreen) | classe $($q.ClassName) | nom '$($q.Name)'" }
    $horsEcran = 0; $inactif = 0; $autreType = 0
    foreach ($e in $tous) {
      $c = $e.Current
      if ($c.IsOffscreen) { $horsEcran++; continue }
      if (-not $c.IsEnabled) { $inactif++; continue }
      if ($types -notcontains $c.ControlType.ProgrammaticName) { $autreType++; continue }
      $vus++
      if ([string]::IsNullOrWhiteSpace($c.Name)) { Defaut "$quoi : controle sans nom pour un lecteur d'ecran ($($c.ControlType.ProgrammaticName), classe $($c.ClassName), id $($c.AutomationId))" }
    }
    Write-Host "       UIA $quoi : $vus verifie(s), $horsEcran hors ecran, $inactif inactif(s), $autreType d'un autre type ; premier plan = fenetre : $([A]::GetForegroundWindow() -eq $w)"
    if ($defauts.Count -eq $d0 -and $vus -gt 0) { Write-Host "OK    $quoi : $vus controles nommes verifies" }
    if ($vus -eq 0) { Defaut "$quoi : aucun controle vu par UI Automation" }
  } catch { Manque "noms UI Automation ($quoi) : $($_.Exception.Message)" }
}
function Photo($hw, $nom) {
  try {
    $r = New-Object A+RECT; [void][A]::GetWindowRect($hw, [ref]$r)
    if (($r.R - $r.L) -le 0 -or ($r.B - $r.T) -le 0) { Manque "taille nulle pour $nom"; return }
    # what is really on the screen (PrintWindow drew Hebrew windows mirrored, 1st 1.9.0 trial), layered
    # windows included (the bubbles came out black without CAPTUREBLT)
    $bmp = [A]::Screen($r.L, $r.T, ($r.R - $r.L), ($r.B - $r.T))
    $bmp.Save((Join-Path $out $nom), [Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    Write-Host "OK    $nom"
  } catch {
    Manque "capture $nom : $($_.Exception.Message)"
    # backup: the whole screen, so that something is always there to look at
    try {
      $sc = [Windows.Forms.Screen]::PrimaryScreen.Bounds
      $g2 = New-Object Drawing.Bitmap $sc.Width, $sc.Height
      $gg = [Drawing.Graphics]::FromImage($g2); $gg.CopyFromScreen(0, 0, 0, 0, $g2.Size); $gg.Dispose()
      $g2.Save((Join-Path $out ("ecran-" + $nom)), [Drawing.Imaging.ImageFormat]::Png); $g2.Dispose()
      Write-Host "      (écran entier gardé : ecran-$nom)"
    } catch { Write-Host "      (écran entier impossible aussi : $($_.Exception.Message))" }
  }
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
function Reglages($lg, $suffixe, $pages, [switch]$noms, $dpi = 96) {
  Start-Process -FilePath $exe -ArgumentList '--settings' | Out-Null
  $w = Attendre { [A]::FindWindow('KBLayoutFixerSettings', [NullString]::Value) } 10000
  if ($w -eq [IntPtr]::Zero) { Manque "réglages ($lg $suffixe)"; return [IntPtr]::Zero }
  [void][A]::SetWindowPos($w, [IntPtr]::Zero, 0, 0, 0, 0, 0x0015)   # top left corner (SWP_NOSIZE|NOZORDER|NOACTIVATE): all of it on the screen
  [void][A]::Front($w); Start-Sleep -Milliseconds 700
  $tc = [A]::GetDlgItem($w, 240)
  foreach ($tab in $pages) {
    [A]::Ask($tc, 0x1330, [IntPtr]($tab - 1), [IntPtr]::Zero)   # TCM_SETCURFOCUS (3 s at most)
    Start-Sleep -Milliseconds 600
    Photo $w "reglages-$lg-$suffixe-onglet$tab.png"
    $c = [A]::Cut($w, [uint32]$dpi)
    if ($c) { Defaut "reglages $lg $suffixe onglet $tab : $c" } else { Write-Host "OK    reglages $lg $suffixe onglet $tab : aucun texte coupe" }
    if ($noms) { Noms $w "reglages $lg onglet $tab" }
  }
  return $w
}
function Bloc-notes($texte) {
  $n = Start-Process notepad -PassThru
  $h = Attendre { $n.Refresh(); $n.MainWindowHandle } 10000
  if ($h -eq [IntPtr]::Zero) { return $null }
  [void][A]::Front($h); Start-Sleep -Milliseconds 1500   # 1.9.1: the first letter was lost at 500 ms (« kuo » instead of « akuo »)
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
    $avantF10 = [A]::GetForegroundWindow()   # (Notepad of Windows 11: its MainWindowHandle may be stale, avis Fable)
    [A]::Key(0x79)   # F10
    $b = Attendre { [A]::Visible('KBLayoutFixerBubble', 0) } 4000
    if ($b -ne [IntPtr]::Zero) {
      Start-Sleep -Milliseconds 250; Photo $b "bulle-f10-$($pos[1])-$lg.png"
      # 1.9.9: the bubble never takes the foreground nor the focus (the user keeps typing)
      $fg = [A]::GetForegroundWindow(); $ex = [A]::GetWindowLong($b, -20)
      if ($fg -ne $avantF10) { Defaut "bulle F10 $($pos[1]) ($lg) : Bloc-notes n'est plus au premier plan" }
      if (($ex -band 0x08000000) -eq 0) { Defaut "bulle F10 $($pos[1]) ($lg) : style sans WS_EX_NOACTIVATE" }
      if (($ex -band 0x20) -eq 0) { Defaut "bulle F10 $($pos[1]) ($lg) : style sans WS_EX_TRANSPARENT (les clics ne la traversent pas)" }
      if ($fg -eq $avantF10 -and ($ex -band 0x08000020) -eq 0x08000020) { Write-Host "OK    bulle F10 $($pos[1]) ($lg) : ne prend ni le premier plan ni les clics" }
    } else { Manque "bulle F10 $($pos[1]) ($lg)" }
    Stop-Process -Id $n.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep 1
  }

  # --- bouton « Rechercher une mise à jour » (page Diagnostic) : bulle de réponse
  $p = Demarrer ('{ "WelcomeDone": true, "ShowBubble": true, "UiLanguage": "' + $lg + '" }') $null
  $w = Reglages $lg 'normal' @(1, 2, 3) -noms
  if ($w -ne [IntPtr]::Zero) {
    [void][A]::PostMessage($w, 0x0111, [IntPtr]264, [A]::GetDlgItem($w, 264))   # WM_COMMAND, bouton 264
    $b = Attendre { [A]::Visible('KBLayoutFixerBubble', 0) } 15000
    if ($b -ne [IntPtr]::Zero) { Start-Sleep -Milliseconds 250; Photo $b "bulle-mise-a-jour-$lg.png" } else { Manque "bulle de mise à jour ($lg)" }
    [void][A]::PostMessage($w, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
  }

  # --- réglages à 150 % et 200 % (tous les onglets)
  foreach ($dpi in 144, 192) {
    $p = Demarrer ('{ "WelcomeDone": true, "ShowBubble": false, "UiLanguage": "' + $lg + '" }') $dpi
    $w = Reglages $lg "$([int]($dpi * 100 / 96))pc" @(1, 2, 3) -dpi $dpi
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
  cmd.exe /c "taskkill /T /F /PID $($pu.Id) >nul 2>&1"   # the uninstaller and the copy it started, only them (already gone = fine)
} else { Manque "désinstalleur introuvable" }

"Captures de l'audit : $((Get-ChildItem $out -Filter *.png).Count) ; manques : $($manques.Count)"
$manques | ForEach-Object { "  - $_"; Write-Host "::warning::audit visuel : $_" }
"Defauts verifies : $($defauts.Count)"
$defauts | ForEach-Object { "  - $_"; Write-Host "::error::audit visuel : $_" }
if ($defauts.Count -gt 0) { Write-Host "ECHEC : $($defauts.Count) defaut(s) dans l'audit visuel"; exit 1 }
exit 0
