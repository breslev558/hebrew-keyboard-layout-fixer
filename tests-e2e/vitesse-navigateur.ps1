# Essai Windows (CI) 1.9.2-1.9.4 : VITESSE de F10 dans Edge puis dans Chrome (moteur Chromium, le même
# que les applications Electron telles que Claude ou Teams ; aucune application Electron n'est installée
# sur la machine d'essai). Après bout-en-bout.ps1 (programme installé).
# Cas : 1er F10 juste après un changement de fenêtre (1.9.3 : il s'arrêtait sur « champ changé » et ne
# marchait qu'au 2e appui), rien de sélectionné (la ligne est prise), tout le texte sélectionné, et F10
# tenu enfoncé 200 ms (1.9.3 : plus d'attente du doigt sur F10 dans Chromium). Le temps lu est
# « until paste » du journal du programme (de l'appui sur F10 jusqu'au collage du texte corrigé ; aucun
# texte n'est jamais écrit dans le journal). Le texte obtenu est aussi relu (Ctrl+A, Ctrl+C).
# Mesure « avant » : la 1.9.3 publiée (téléchargée de GitHub) passe le même essai dans Edge, à côté.
# Seuil : la nouvelle version doit réussir chaque cas et coller en moins de 300 ms (médiane de 3 ; 1er F10
# à froid ou après un changement de fenêtre : 800 ms, la page prépare alors son accessibilité), et aucun
# arrêt « changed (… » ne doit paraître dans son journal, sinon ECHEC. Chrome absent = ECHEC.
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
  // 1.9.3: a key held down like a finger does (key-up after `ms`)
  public static void Hold(byte vk, int ms) { keybd_event(vk, 0, 0, UIntPtr.Zero); System.Threading.Thread.Sleep(ms); keybd_event(vk, 0, 2, UIntPtr.Zero); }
  // held longer than the repeat delay: key-downs repeated every 33 ms after 500 ms, like Windows does
  public static void HoldRepeat(byte vk, int ms) { keybd_event(vk, 0, 0, UIntPtr.Zero); System.Threading.Thread.Sleep(500); for (int t = 500; t < ms; t += 33) { keybd_event(vk, 0, 0, UIntPtr.Zero); System.Threading.Thread.Sleep(33); } keybd_event(vk, 0, 2, UIntPtr.Zero); }
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
  // 1.9.4: F10 pressed, a click 10 ms later while F10 is still down, then F10 released (timing kept
  // here, not in PowerShell where a 5 ms sleep lasts 15-20 ms, avis Fable)
  public static void F10ThenClick(IntPtr h, int dx, int dy) { keybd_event(0x79, 0, 0, UIntPtr.Zero); System.Threading.Thread.Sleep(10); ClickIn(h, dx, dy); System.Threading.Thread.Sleep(150); keybd_event(0x79, 0, 2, UIntPtr.Zero); }
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
$chrome = @("$env:ProgramFiles\Google\Chrome\Application\chrome.exe", "${env:ProgramFiles(x86)}\Google\Chrome\Application\chrome.exe", "$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
# the page title shows the text of the box (« T:akuo… »): read through the window title, never through
# the clipboard (both reads came back empty in the 2nd 1.9.2 trial)
$html = '<!doctype html><title>T::F</title><body style="margin:0"><textarea id=t autofocus oninput="document.title=''T:''+this.value+'':F''" onselect="document.title=''T:''+this.value+'':F sel''" style="position:fixed;left:0;top:0;width:100%;height:100%;font-size:20px"></textarea></body>'
$page = 'data:text/html;base64,' + [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($html))   # (avis Fable : # % " cassent une URL brute)
$script:h = [IntPtr]::Zero
# opens the test page in its own profile and new window; the window is the one of THIS browser's exe
function Ouvrir($nav, $prive) {
  $nomProc = [IO.Path]::GetFileNameWithoutExtension($nav)
  $profil = Join-Path $env:TEMP "klf-profil-$nomProc"
  $script:proc = Start-Process -FilePath $nav -ArgumentList "--user-data-dir=`"$profil`"", $prive, '--no-first-run', '--no-default-browser-check', '--disable-sync', '--new-window', $page -PassThru
  $script:h = [IntPtr]::Zero; $t = 0
  while ($script:h -eq [IntPtr]::Zero) {
    Start-Sleep -Milliseconds 500
    $w = Get-Process $nomProc -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 -and $_.Path -eq $nav } | Select-Object -First 1
    if ($w) { $script:h = $w.MainWindowHandle }
    if (++$t -gt 40) { Fail "fenêtre $nomProc introuvable" }
  }
  Start-Sleep 4   # page loaded, textarea focused
  $t = 0; while (-not [N]::Front($h)) { Start-Sleep -Milliseconds 300; if (++$t -gt 20) { Fail "$nomProc pas au premier plan" } }
  Start-Sleep 1
  # 1.9.2 (2nd trial): the focus stayed in the address bar (field Chrome_WidgetWin_1, nothing typed in the
  # page): a click in the middle of the window, where the text box fills the whole page
  Cliquer; Start-Sleep -Milliseconds 800
}
function Fermer($nav) {
  Stop-Process -Id $script:proc.Id -Force -ErrorAction SilentlyContinue
  Get-Process ([IO.Path]::GetFileNameWithoutExtension($nav)) -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $nav } | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep 2
}
function Cliquer { $r = New-Object N+RECT; [void][N]::GetWindowRect($h, [ref]$r); [N]::ClickIn($h, [int](($r.R - $r.L) / 2), [int](($r.B - $r.T) / 2)) }

function Lancer($chemin) {
  Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 1
  $reglages | Set-Content -Encoding UTF8 (Join-Path $env:APPDATA 'KeyboardLayoutFixer\settings.json')   # same settings for each version (avis Fable)
  Start-Process $chemin; Start-Sleep 3
  if (-not (Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue)) { Fail "le programme ne démarre pas ($chemin)" }
  $t = 0; while (-not [N]::Front($h)) { Start-Sleep -Milliseconds 300; if (++$t -gt 20) { Fail 'navigateur pas au premier plan' } }
  Start-Sleep -Milliseconds 500
  Cliquer; Start-Sleep -Milliseconds 500
}
function Texte { $m = [regex]::Match([N]::Title($h), 'T:(.*?):F'); if ($m.Success) { $m.Groups[1].Value } else { "(titre : $([N]::Title($h)))" } }
function Heure($ligne) { [datetime]::ParseExact($ligne.Substring(1, 23), 'yyyy-MM-dd HH:mm:ss.fff', $null) }
# $pause: wait between the typing and F10 ; $tenu: F10 held that long (0 = pressed and released at once)
function Mesure($version, $nom, [scriptblock]$avant, $attendu, $seuil, $pause = 800, $tenu = 0) {
  $n0 = @(Get-Content $log -ErrorAction SilentlyContinue).Count
  & $avant
  # 1.9.3: the typed text is waited for (2 s at most), then F10 comes $pause ms later
  $sw0 = [Diagnostics.Stopwatch]::StartNew()
  while ((Texte) -ne 'akuo' -and $sw0.ElapsedMilliseconds -lt 2000) { Start-Sleep -Milliseconds 20 }
  Start-Sleep -Milliseconds $pause
  Write-Host "       avant F10 : texte '$(Texte)'"
  if ((Texte) -ne 'akuo') { Fail "$version $nom : « akuo » n'a pas été tapé correctement ('$(Texte)')" }
  $appui = Get-Date
  if ($tenu -gt 500) { [N]::HoldRepeat(0x79, $tenu) } elseif ($tenu -gt 0) { [N]::Hold(0x79, $tenu) } else { [N]::Key(0x79) }   # F10
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
  # 1.9.3: one F10 = one paste (repeats of a held F10 must never convert again, avis Fable)
  $nb = @(Get-Content $log | Select-Object -Skip $n0 | Where-Object { $_ -match 'Ctrl\+V sent' }).Count
  if ($nb -gt 1 -and -not $script:avant) { Fail "$version $nom : $nb collages pour un seul F10" }
  Write-Host "       fin : $(($fin -split '\] ', 2)[1])"
  $jusque = if ($null -ne $ms) { "$ms ms jusqu'au collage, " } else { '' }
  Write-Host "MESURE $version $nom : $jusque$ext ms en tout ; texte '$texte'"
  if ($exact) { Write-Host "       $(($exact -split 'Timing: ')[1])" }
  if ($env:GITHUB_STEP_SUMMARY) { "| $version | $nom | $(if ($null -ne $ms) { "$ms ms" } else { '-' }) | $ext ms |" | Out-File -Append -Encoding utf8 $env:GITHUB_STEP_SUMMARY }
  # 1.9.3: the new version must never stop on « window or field changed » (the reason is in brackets)
  $arret = @(Get-Content $log) | Select-Object -Skip $n0 | Where-Object { $_ -match 'changed \(' } | Select-Object -First 1
  if ($arret -and -not $script:avant) { Fail "$version $nom : arrêt « $(($arret -split '\] ', 2)[1]) »" }
  if ($texte -ne $attendu) { Fail "$version $nom : texte attendu « $attendu », obtenu « $texte »" }
  if ($null -ne $seuil -and ($null -eq $ms -or $ms -gt $seuil)) { Fail "$version $nom : $ms ms, trop lent (seuil $seuil ms)" }
  return $ms
}
# the new version: 3 tries, the MEDIAN is compared with the threshold (a slow runner once is not a failure)
function Mediane3($version, $nom, [scriptblock]$avant, $attendu, $seuil, $tenu = 0) {
  $v = @(1..3 | ForEach-Object { Mesure $version "$nom (essai $_)" $avant $attendu $null 800 $tenu } | Where-Object { $null -ne $_ } | Sort-Object)
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
# 1.9.3: 1st F10 right after coming back from another window (Notepad): typed at once, F10 150 ms later
$retour = { $np = Start-Process notepad -PassThru; Start-Sleep 2
            $t = 0; while ($np.MainWindowHandle -eq 0 -and $t++ -lt 20) { Start-Sleep -Milliseconds 300; $np.Refresh() }
            if ($np.MainWindowHandle -eq 0) { Fail 'Notepad sans fenêtre' }
            $t = 0; while (-not [N]::Front($np.MainWindowHandle)) { Start-Sleep -Milliseconds 300; if (++$t -gt 20) { Fail 'Notepad pas au premier plan' } }
            Start-Sleep 2
            if ([N]::GetForegroundWindow() -eq $h) { Fail 'le navigateur est resté devant (passage par Notepad non fait)' }
            Stop-Process -Id $np.Id -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 300
            $t = 0; while (-not [N]::Front($h)) { Start-Sleep -Milliseconds 300; if (++$t -gt 20) { Fail 'navigateur pas revenu au premier plan' } }
            Cliquer; Start-Sleep -Milliseconds 300
            & $vider; [System.Windows.Forms.SendKeys]::SendWait('akuo') }
# 1.9.4: Enter, then a click in the text box just BEFORE F10 (60 ms): on 12/11 such F10s were
# cancelled, the 1.9.3 cleared the flag of the click but not the one of Enter (avis Fable); the
# click is in the middle of the page, below the text: the caret goes to the end of « akuo »
$clic = { [System.Windows.Forms.SendKeys]::SendWait('{ENTER}'); & $vider
          [System.Windows.Forms.SendKeys]::SendWait('akuo'); Start-Sleep -Milliseconds 400; Cliquer }
if ($env:GITHUB_STEP_SUMMARY) { @("", "### Vitesse de F10 dans Edge et Chrome", "", "| Version | Cas | Jusqu'au collage | En tout (dont 100 ms avant la remise du presse-papiers) |", "|---|---|---|---|") | Out-File -Append -Encoding utf8 $env:GITHUB_STEP_SUMMARY }
$navs = @(@{ Nom = 'Edge'; Exe = $edge; Prive = '--inprivate' }, @{ Nom = 'Chrome'; Exe = $chrome; Prive = '--incognito' }) | Where-Object { $_.Exe }
if (-not $edge) { Write-Host "MANQUE : Edge introuvable sur cette machine" }
if (-not $chrome) { Fail 'Chrome introuvable sur cette machine (le navigateur de la plainte)' }
if (-not $navs) { Write-Host "MANQUE : aucun navigateur, essai sauté"; exit 0 }
$resultats = @()
foreach ($n in $navs) {
  Write-Host "===== $($n.Nom) ====="
  Ouvrir $n.Exe $n.Prive
  if ($n.Nom -eq 'Edge') {
    # AVANT : la 1.9.3 publiée (dans Edge seulement)
    $ancien = Join-Path $env:TEMP 'klf-193\KeyboardLayoutFixer.exe'
    New-Item -ItemType Directory -Force -Path (Split-Path $ancien) | Out-Null
    $script:avant = $true
    try {
      Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/breslev558/hebrew-keyboard-layout-fixer/releases/download/v1.9.3/KeyboardLayoutFixer.exe' -OutFile $ancien
      Lancer $ancien
      # each « before » case on its own: a failure of the old version only informs (avis Fable)
      foreach ($c in @(@('Edge, 1er F10 après changement de fenêtre', $retour, 150, 0), @('Edge, clic dans la zone 60 ms avant F10', $clic, 60, 0), @('Edge, sans sélection', $sans, 800, 0), @('Edge, F10 tenu 200 ms', $sans, 800, 200))) {
        $n1 = @(Get-Content $log -ErrorAction SilentlyContinue).Count
        try { [void](Mesure '1.9.3 (avant)' $c[0] $c[1] $shalom $null $c[2] $c[3]) } catch { Write-Host "AVANT (pour information) : $($_.Exception.Message)" }
        # does the old version show the « 2nd press » fault here? (else this test proves nothing for it)
        $arr = @(Get-Content $log) | Select-Object -Skip $n1 | Where-Object { $_ -match 'field changed' }
        Write-Host "       1.9.3, $($c[0]) : $(if ($arr) { 'REPRODUIT (arrêt « champ changé »)' } else { 'NON REPRODUIT' })"
      }
    } catch { Write-Host "MANQUE : mesure avant impossible ($($_.Exception.Message))" }
    $script:avant = $false
    # a fresh browser for the new version: the old one switched the page's accessibility on (avis Fable)
    Fermer $n.Exe; Ouvrir $n.Exe $n.Prive
  }
  # APRÈS : la version de cet essai (installée), démarrée à neuf pour chaque navigateur
  $nlog = @(Get-Content $log -ErrorAction SilentlyContinue).Count
  Lancer $exe
  $v = $n.Nom
  # à froid : tout premier F10 de cette version dans ce navigateur, sans changement de fenêtre (avis Fable)
  [void](Mesure 'nouvelle' "$v, tout 1er F10 à froid" $sans $shalom 800 150)
  [void](Mesure 'nouvelle' "$v, 1er F10 après changement de fenêtre" $retour $shalom 800 150)
  [void](Mesure 'nouvelle' "$v, 1er F10 après changement de fenêtre (2e fois)" $retour $shalom 800 150)
  [void](Mesure 'nouvelle' "$v, clic dans la zone 60 ms avant F10" $clic $shalom 800 60)
  [void](Mesure 'nouvelle' "$v, clic dans la zone 60 ms avant F10 (2e fois)" $clic $shalom 800 60)
  # and the other way: a real click DURING the fix still cancels it (the safety must stay): never a
  # paste; retried (3 times) only when the journal shows neither a paste nor a cancel (avis Fable)
  $annule = $false
  foreach ($e in 1..3) {
    $n2 = @(Get-Content $log).Count
    & $sans; $sw0 = [Diagnostics.Stopwatch]::StartNew()
    while ((Texte) -ne 'akuo' -and $sw0.ElapsedMilliseconds -lt 2000) { Start-Sleep -Milliseconds 20 }
    Start-Sleep -Milliseconds 800
    $r = New-Object N+RECT; [void][N]::GetWindowRect($h, [ref]$r)
    [N]::F10ThenClick($h, [int](($r.R - $r.L) / 2), [int](($r.B - $r.T) / 2))
    Start-Sleep 3
    $apres = @(@(Get-Content $log) | Select-Object -Skip $n2) + @('')   # never empty (FindIndex)
    $iVu = [array]::FindIndex([string[]]$apres, [Predicate[string]]{ param($l) $l -match 'User input during the fix' })
    $iColle = [array]::FindIndex([string[]]$apres, [Predicate[string]]{ param($l) $l -match 'Ctrl\+V sent' })
    Write-Host "       clic pendant la correction (essai $e) : $(if ($iVu -ge 0) { ($apres[$iVu] -split '\] ', 2)[1] } else { 'rien vu' }) ; collé : $($iColle -ge 0) ; texte '$(Texte)'"
    # the click comes 10 ms after F10 (well before the paste, about 57 ms): any paste is a failure
    if ($iColle -ge 0) { Fail "$v : texte collé malgré le clic pendant la correction (essai $e)" }
    if ($iVu -ge 0) {
      if ((Texte) -ne 'akuo') { Fail "$v : correction annulée mais texte changé ('$(Texte)')" }
      $annule = $true; break
    }
  }
  if (-not $annule) { Fail "$v : un clic pendant la correction ne l'a jamais annulée (3 essais)" }
  $resultats += Mediane3 'nouvelle' "$v, sans sélection" $sans $shalom 300
  $resultats += Mediane3 'nouvelle' "$v, avec sélection" $avec $shalom 300
  $resultats += Mediane3 'nouvelle' "$v, F10 tenu 200 ms" $sans $shalom 300 200
  # tenu 80 ms (relâché avant le collage) et 1 s avec répétitions : une seule correction, texte « שלום »
  [void](Mesure 'nouvelle' "$v, F10 tenu 80 ms" $sans $shalom 800 800 80)
  [void](Mesure 'nouvelle' "$v, F10 tenu 1 s avec répétitions" $sans $shalom $null 800 1000)
  # the focus must still be in the box after the repeats (F10 reaching the browser opens its menu)
  [void](Mesure 'nouvelle' "$v, après F10 tenu 1 s" $sans $shalom $null)
  $vus = @(Get-Content $log) | Select-Object -Skip $nlog | Where-Object { $_ -match 'Focus moved|Text caret moved|User input during' }
  Write-Host "INFO $v : $($vus.Count) changement(s) de focus vus par la nouvelle version"; $vus | ForEach-Object { Write-Host "       $(($_ -split '\] ', 2)[1])" }
  Fermer $n.Exe
}
$resultats
Get-Process KeyboardLayoutFixer -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Process $exe
exit 0
