# Essai Windows (CI) 1.9.2 : VITESSE de F10 dans un navigateur (Edge = Chromium, comme Chrome et les
# applications Electron telles que Claude Code ou Teams). Après bout-en-bout.ps1 (programme installé).
# Deux cas : rien de sélectionné (la ligne est prise) et tout le texte sélectionné. Le temps lu est
# « until paste » du journal du programme (de l'appui sur F10 jusqu'au collage du texte corrigé ; aucun
# texte n'est jamais écrit dans le journal). Le texte obtenu est aussi relu (Ctrl+A, Ctrl+C).
# Mesure « avant » : la 1.9.1 publiée (téléchargée de GitHub) passe le même essai, à côté.
# Seuil : la nouvelle version doit coller en moins de 300 ms dans chaque cas, sinon ECHEC.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class N {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr p);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  public static void Key(byte vk) { keybd_event(vk, 0, 0, UIntPtr.Zero); keybd_event(vk, 0, 2, UIntPtr.Zero); }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr LoadKeyboardLayout(string id, uint f);
  // the window's keyboard back to English (US) before typing « akuo » (after a fix it can be Hebrew,
  // and then SendKeys' Ctrl+A and « akuo » come out wrong: seen in the 4th 1.9.2 trial)
  public static void English(IntPtr h) { PostMessage(h, 0x0050, IntPtr.Zero, LoadKeyboardLayout("00000409", 1)); }
  // Ctrl+A by key codes (independent of the keyboard language)
  public static void CtrlA() { keybd_event(0x11, 0, 0, UIntPtr.Zero); keybd_event(0x41, 0, 0, UIntPtr.Zero); keybd_event(0x41, 0, 2, UIntPtr.Zero); keybd_event(0x11, 0, 2, UIntPtr.Zero); }
  public static string Title(IntPtr h) { var sb = new System.Text.StringBuilder(512); GetWindowText(h, sb, 512); return sb.ToString(); }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
  // a click inside the page: the focus goes to the text box (Edge leaves it in the address bar)
  public static void ClickIn(IntPtr h, int dx, int dy) { RECT r; GetWindowRect(h, out r); SetCursorPos(r.L + dx, r.T + dy); mouse_event(2, 0, 0, 0, UIntPtr.Zero); mouse_event(4, 0, 0, 0, UIntPtr.Zero); }
  public static bool Front(IntPtr h) {
    uint me = GetCurrentThreadId(), fg = GetWindowThreadProcessId(GetForegroundWindow(), IntPtr.Zero);
    AttachThreadInput(me, fg, true); keybd_event(0x12, 0, 0, UIntPtr.Zero); keybd_event(0x12, 0, 2, UIntPtr.Zero);
    bool ok = SetForegroundWindow(h); AttachThreadInput(me, fg, false); return ok && GetForegroundWindow() == h;
  }
}
'@
function Fail($m) { if ($script:avant) { throw $m }; Write-Host "ECHEC : $m"; exit 1 }   # the « before » measure only informs
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$log = Join-Path $env:APPDATA 'KeyboardLayoutFixer\debug.log'
$exe = Join-Path $env:LOCALAPPDATA 'Programs\KeyboardLayoutFixer\KeyboardLayoutFixer.exe'
# the keyboard stays as it is after F10 (otherwise the next « akuo » would be typed in Hebrew)
Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 1
$reglages = '{ "WelcomeDone": true, "ShowBubble": false, "SwitchLanguageAfterConvert": false, "PasteDelayMs": 100, "SettingsVersion": 4, "UiLanguage": "en" }'
$reglages | Set-Content -Encoding UTF8 (Join-Path $env:APPDATA 'KeyboardLayoutFixer\settings.json')
Start-Process $exe; Start-Sleep 3
$edge = @("${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe", "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { Write-Host "MANQUE : Edge introuvable sur cette machine, essai sauté"; exit 0 }
$profil = Join-Path $env:TEMP 'klf-edge-profil'
# the page title shows the text of the box (« T:akuo… »): read through the window title, never through
# the clipboard (both reads came back empty in the 2nd 1.9.2 trial)
$html = '<!doctype html><title>T::F</title><body style="margin:0"><textarea id=t autofocus oninput="document.title=''T:''+this.value+'':F''" onselect="document.title=''T:''+this.value+'':F sel''" style="position:fixed;left:0;top:0;width:100%;height:100%;font-size:20px"></textarea></body>'
$page = 'data:text/html;base64,' + [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($html))   # (avis Fable : # % " cassent une URL brute)
$p = Start-Process -FilePath $edge -ArgumentList "--user-data-dir=`"$profil`"", '--inprivate', '--no-first-run', '--no-default-browser-check', '--disable-sync', '--new-window', $page -PassThru
$h = [IntPtr]::Zero; $t = 0
while ($h -eq [IntPtr]::Zero) {
  Start-Sleep -Milliseconds 500
  $w = Get-Process msedge -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
  if ($w) { $h = $w.MainWindowHandle }
  if (++$t -gt 40) { Fail 'fenêtre Edge introuvable' }
}
Start-Sleep 4   # page loaded, textarea focused
$t = 0; while (-not [N]::Front($h)) { Start-Sleep -Milliseconds 300; if (++$t -gt 20) { Fail 'Edge pas au premier plan' } }
Start-Sleep 1
# 1.9.2 (2nd trial): the focus stayed in the address bar (field Chrome_WidgetWin_1, nothing typed in the
# page): a click in the middle of the window, where the text box fills the whole page
$r = New-Object N+RECT; [void][N]::GetWindowRect($h, [ref]$r)
[N]::ClickIn($h, [int](($r.R - $r.L) / 2), [int](($r.B - $r.T) / 2))
Start-Sleep -Milliseconds 800

function Lancer($chemin) {
  Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 1
  $reglages | Set-Content -Encoding UTF8 (Join-Path $env:APPDATA 'KeyboardLayoutFixer\settings.json')   # same settings for each version (avis Fable)
  Start-Process $chemin; Start-Sleep 3
  if (-not (Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue)) { Fail "le programme ne démarre pas ($chemin)" }
  [void][N]::Front($h); Start-Sleep -Milliseconds 500
  $r = New-Object N+RECT; [void][N]::GetWindowRect($h, [ref]$r)
  [N]::ClickIn($h, [int](($r.R - $r.L) / 2), [int](($r.B - $r.T) / 2)); Start-Sleep -Milliseconds 500
}
function Texte { $m = [regex]::Match([N]::Title($h), 'T:(.*?):F'); if ($m.Success) { $m.Groups[1].Value } else { "(titre : $([N]::Title($h)))" } }
function Heure($ligne) { [datetime]::ParseExact($ligne.Substring(1, 23), 'yyyy-MM-dd HH:mm:ss.fff', $null) }
function Mesure($version, $nom, [scriptblock]$avant, $attendu, $seuil) {
  $n0 = @(Get-Content $log -ErrorAction SilentlyContinue).Count
  & $avant
  Start-Sleep -Milliseconds 800
  Write-Host "       avant F10 : texte '$(Texte)'"
  if ((Texte) -ne 'akuo') { Fail "$version $nom : « akuo » n'a pas été tapé correctement ('$(Texte)')" }
  $appui = Get-Date
  [N]::Key(0x79)   # F10
  $fin = $null; $sw = [Diagnostics.Stopwatch]::StartNew()
  while (-not $fin -and $sw.ElapsedMilliseconds -lt 8000) {
    Start-Sleep -Milliseconds 200
    $fin = @(Get-Content $log -ErrorAction SilentlyContinue) | Select-Object -Skip $n0 | Where-Object { $_ -match 'Ctrl\+V sent|NOT pasted|nothing done|stopped' } | Select-Object -Last 1
  }
  if (-not $fin) { Fail "$version $nom : aucune fin dans le journal (F10 pas pris ?)" }
  # outside measure, same for both versions: from the F10 press to the end of the attempt (it includes
  # the 100 ms wait before the clipboard is put back)
  $ext = [int]((Heure $fin) - $appui).TotalMilliseconds
  # the timing line may come just after the end line: waited for up to 2 s (avis Fable)
  $exact = $null; $sw2 = [Diagnostics.Stopwatch]::StartNew()
  do {
    $exact = @(Get-Content $log) | Select-Object -Skip $n0 | Where-Object { $_ -match 'Timing: until paste (\d+) ms' } | Select-Object -Last 1
    if (-not $exact) { Start-Sleep -Milliseconds 200 }
  } while (-not $exact -and $sw2.ElapsedMilliseconds -lt 2000 -and -not $script:avant)
  $ms = if ($exact) { [int]([regex]::Match($exact, 'until paste (\d+) ms').Groups[1].Value) } else { $null }
  Start-Sleep 1
  $texte = Texte
  Write-Host "       fin : $(($fin -split '\] ', 2)[1])"
  $jusque = if ($null -ne $ms) { "$ms ms jusqu'au collage, " } else { '' }
  Write-Host "MESURE $version $nom : $jusque$ext ms en tout ; texte '$texte'"
  if ($exact) { Write-Host "       $(($exact -split 'Timing: ')[1])" }
  if ($env:GITHUB_STEP_SUMMARY) { "| $version | $nom | $(if ($null -ne $ms) { "$ms ms" } else { '-' }) | $ext ms |" | Out-File -Append -Encoding utf8 $env:GITHUB_STEP_SUMMARY }
  if ($texte -ne $attendu) { Fail "$version $nom : texte attendu « $attendu », obtenu « $texte »" }
  return $ms
}
# the new version: 3 tries, the MEDIAN is compared with the threshold (a slow runner once is not a failure)
function Mediane3($version, $nom, [scriptblock]$avant, $attendu, $seuil) {
  $v = @(1..3 | ForEach-Object { Mesure $version "$nom (essai $_)" $avant $attendu $null } | Where-Object { $null -ne $_ } | Sort-Object)
  if ($v.Count -lt 3) { Fail "$version $nom : temps « until paste » absent du journal ($($v.Count) mesures sur 3)" }
  $m = $v[1]
  Write-Host "MEDIANE $version $nom : $m ms jusqu'au collage ($($v -join ', '))"
  if ($null -eq $m -or $m -gt $seuil) { Fail "$version $nom : médiane $m ms, trop lent (seuil $seuil ms)" }
  "OK    $version $nom : médiane $m ms"
}
$shalom = [string][char]0x05E9 + [char]0x05DC + [char]0x05D5 + [char]0x05DD
# the box is emptied with Ctrl+A then Backspace (the Delete key sent by keybd_event did nothing in the
# 3rd 1.9.2 trial: the texts piled up)
$vider = { [N]::English($h); Start-Sleep -Milliseconds 300
           # several rounds: even without a selection each Backspace removes a letter (avis Fable)
           for ($i = 0; $i -lt 12 -and (Texte) -ne ''; $i++) { [N]::CtrlA(); Start-Sleep -Milliseconds 150; [N]::Key(0x08); Start-Sleep -Milliseconds 300 }
           Write-Host "       vidé en $i tour(s)"
           if ((Texte) -ne '') { Fail "zone de texte pas vidée : '$(Texte)'" } }
$sans = { & $vider; [System.Windows.Forms.SendKeys]::SendWait('akuo') }
$avec = { & $vider; [System.Windows.Forms.SendKeys]::SendWait('akuo'); Start-Sleep -Milliseconds 300; [N]::CtrlA()
          Start-Sleep -Milliseconds 300; if ([N]::Title($h) -notmatch ':F sel') { Fail 'sélection non faite (Ctrl+A)' } }
if ($env:GITHUB_STEP_SUMMARY) { @("", "### Vitesse de F10 dans Edge", "", "| Version | Cas | Jusqu'au collage | En tout (dont 100 ms avant la remise du presse-papiers) |", "|---|---|---|---|") | Out-File -Append -Encoding utf8 $env:GITHUB_STEP_SUMMARY }
# AVANT : la 1.9.1 publiée
$ancien = Join-Path $env:TEMP 'klf-191\KeyboardLayoutFixer.exe'
New-Item -ItemType Directory -Force -Path (Split-Path $ancien) | Out-Null
$script:avant = $true
try {
  Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/breslev558/hebrew-keyboard-layout-fixer/releases/download/v1.9.1/KeyboardLayoutFixer.exe' -OutFile $ancien
  Lancer $ancien
  [void](Mesure '1.9.1 (avant)' 'sans sélection' $sans $shalom $null)
  [void](Mesure '1.9.1 (avant)' 'avec sélection' $avec $shalom $null)
} catch { Write-Host "MANQUE : mesure avant impossible ($($_.Exception.Message))" }
$script:avant = $false
# APRÈS : la version de cet essai (installée)
Lancer $exe
Mediane3 'nouvelle' 'sans sélection' $sans $shalom 300
Mediane3 'nouvelle' 'avec sélection' $avec $shalom 300
Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Process $exe
Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
Get-Process msedge -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $edge } | Stop-Process -Force -ErrorAction SilentlyContinue
exit 0
