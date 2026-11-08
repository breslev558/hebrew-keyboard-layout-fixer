# Keyboard Layout Fixer (version NOAM) - installation pour tous les utilisateurs de ce PC.
# Lance par Installer.cmd. Option : -NonInteractive (installation a distance : aucune question,
# rien n'est force sauf une ancienne version RECONNUE (1.0 a 1.2, ne sait pas se fermer
# proprement ; c'est ecrit) ; code de sortie different de 0 en cas de probleme).
# Codes : 0 = installe et lance ; 1 = erreur (ancienne version remise) ; 2 = programme encore
# en marche (rien change) ; 3 = installe mais lancement non confirme ; 4 = installation PARTIELLE
# (le retour arriere n'a pas tout remis, liste affichee) ; 5 = pas de droits admin.
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

$Src = Join-Path $PSScriptRoot 'KeyboardLayoutFixer.exe'
$Dest = Join-Path $env:ProgramFiles 'KeyboardLayoutFixer'
$Exe = Join-Path $Dest 'KeyboardLayoutFixer.exe'
$Old = "$Exe.old"
$Lnk = Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs\Keyboard Layout Fixer.lnk'
$RunKey = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run'
if (-not (Test-Path -LiteralPath $Src)) { Fin 1 'ERREUR : KeyboardLayoutFixer.exe doit etre dans le meme dossier. Decompressez d''abord le zip.' }
# --- droits administrateur : verifies sur le jeton Windows, une seule relance ---
$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) {
    if ($Eleve -or $NonInteractive) { Fin 5 'ERREUR : il faut les droits administrateur.' }
    Write-Host 'Demande des droits administrateur...'
    try {
        $p = Start-Process powershell -Verb RunAs -Wait -PassThru -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"", '-Eleve')
        exit $p.ExitCode
    } catch {
        Write-Host 'Droits administrateur refuses : rien n''a ete installe.'
        exit 5
    }
}

Unblock-File -LiteralPath $Src -ErrorAction SilentlyContinue
function Copies { @(Get-CimInstance Win32_Process -Filter "Name='KeyboardLayoutFixer.exe'") }
# encore en marche : seulement si le PID est toujours CE programme (un PID libere peut etre reattribue)
function Vivants($ids) { @(Get-Process -Id $ids -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -eq 'KeyboardLayoutFixer' }) }

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

# Arret PROPRE (taskkill sans /f = message de fermeture) : chaque copie termine d'abord une
# correction en cours et remet le presse-papiers. Pire cas d'une correction : environ 15 s
# (delai de collage 10 s max + copie + touches) : on attend 30 s, puis on DEMANDE.
function Arreter($liste, [string]$quoi) {
    foreach ($p in $liste) { cmd.exe /c "taskkill /pid $($p.ProcessId) >nul 2>&1" }
    $ids = @($liste | ForEach-Object { $_.ProcessId })
    Fermer-Fenetres $ids
    for ($i = 0; $i -lt 30; $i++) {
        if (-not (Vivants $ids)) { return $true }
        Start-Sleep -Seconds 1
    }
    # On ne regarde que les copies ENCORE en marche. Arret force SANS question uniquement pour une
    # ancienne version RECONNUE (elle ne sait pas se fermer proprement) :
    # NOAM .NET 1.2.x ou version d'origine 1.0/1.1. Toute autre version (1.3+, future,
    # illisible, format inconnu) : question, ou rien en non interactif (audit 1.3.7, P3).
    $vivantsIds = @(Vivants $ids | ForEach-Object { $_.Id })
    $restants = @($liste | Where-Object { $vivantsIds -contains $_.ProcessId })
    $vieilles = @($restants | Where-Object {
        $fi = try { (Get-Item -LiteralPath $_.ExecutablePath -ErrorAction Stop).VersionInfo } catch { $null }
        $v = if ($fi) { "$($fi.ProductVersion)".Trim() } else { '' }
        if ($fi -and -not $v) { $v = "$($fi.FileVersion)".Trim() }
        $v -match '^1\.[0-2](\D|$)' })
    if ($restants.Count -gt 0 -and $vieilles.Count -eq $restants.Count) {
        Write-Host "$quoi (ancienne version) ne sait pas se fermer proprement : arret force."
    } elseif (-not (Question "$quoi ne s'est pas arrete en 30 s (une correction est peut-etre en cours). Forcer l'arret ?")) { return $false }
    Vivants $ids | Stop-Process -Force
    Start-Sleep -Seconds 2
    return -not (Vivants $ids)
}

$session = (Get-Process -Id $PID).SessionId

# --- etat initial, pour un retour arriere COMPLET ---
$avant = @{
    Dossier = Test-Path -LiteralPath $Dest
    Exe     = Test-Path -LiteralPath $Exe
    Run     = (Get-ItemProperty -Path $RunKey -Name 'KeyboardLayoutFixer' -ErrorAction SilentlyContinue).KeyboardLayoutFixer
}
$New = "$Exe.new"
# Empreinte SHA-256 valide, ou $null (fichier absent ou illisible). Deux fichiers ne sont
# "identiques" que si les DEUX empreintes sont valides et egales : une empreinte illisible
# n'est jamais prise pour une preuve (audit 1.3.5, N1).
function Hash($f) {
    if (-not (Test-Path -LiteralPath $f)) { return $null }
    $h = (Get-FileHash -LiteralPath $f -Algorithm SHA256 -ErrorAction SilentlyContinue).Hash
    if ($h -and $h -match '^[0-9A-Fa-f]{64}$') { return $h.ToUpper() }
    return $null
}
function Pareils($a, $b) { $ha = Hash $a; $hb = Hash $b; return [bool]($ha -and $hb -and $ha -eq $hb) }
# une copie de secours laissee par une installation PARTIELLE precedente n'est jamais ecrasee
if ((Test-Path -LiteralPath $Old) -and -not (Pareils $Old $Exe)) {
    $garde = "$Old.$(Get-Date -Format yyyyMMdd-HHmmss)"
    Move-Item -LiteralPath $Old -Destination $garde -Force
    Write-Host "Ancienne copie de secours gardee sous : $garde"
}

# --- 1. nouveau programme prepare a cote (.new) et copie de secours (.old), AVANT tout arret ---
# (un echec ici ne touche ni au programme en place ni a ceux qui tournent : audit 1.3.3 F3)
try {
    if (-not $avant.Dossier) { New-Item -ItemType Directory -Path $Dest | Out-Null }
    Copy-Item -LiteralPath $Src -Destination $New -Force
    if (-not (Pareils $Src $New)) { throw 'le fichier copie est different de celui du zip (ou empreinte illisible)' }
    if ($avant.Exe) {
        Copy-Item -LiteralPath $Exe -Destination $Old -Force
        if (-not (Pareils $Exe $Old)) { throw 'copie de secours differente du programme en place (ou empreinte illisible)' }
    }
} catch {
    $raison = $_.Exception.Message
    Remove-Item -LiteralPath $New, $Old -Force -ErrorAction SilentlyContinue
    if (-not $avant.Dossier) { Remove-Item -LiteralPath $Dest -Recurse -Force -ErrorAction SilentlyContinue }
    Fin 1 "ERREUR : $raison. Rien n'a ete change (le programme en place tourne toujours)."
}

# --- 2. arret propre des copies ---
$toutes = Copies
$nos = @($toutes | Where-Object { $_.ExecutablePath -and ($_.ExecutablePath -ieq $Exe) })
$nosIci = @($nos | Where-Object { $_.SessionId -eq $session }).Count -gt 0   # a relancer si retour arriere
if ($nos.Count -gt 0 -and -not (Arreter $nos 'Le programme installe')) {
    Remove-Item -LiteralPath $New, $Old -Force -ErrorAction SilentlyContinue
    if (-not $avant.Dossier) { Remove-Item -LiteralPath $Dest -Recurse -Force -ErrorAction SilentlyContinue }
    Fin 2 'Le programme tourne encore : rien n''a ete change. Quittez-le (icone pres de l''horloge > Quitter) puis relancez l''installation.'
}
# autres copies (portables, ancienne version) : seules celles de CETTE session empechent la
# nouvelle de demarrer (instance unique propre a chaque session Windows)
$autres = @($toutes | Where-Object { -not ($_.ExecutablePath -and ($_.ExecutablePath -ieq $Exe)) })
$autresIci = @($autres | Where-Object { $_.SessionId -eq $session })
$autresAilleurs = @($autres | Where-Object { $_.SessionId -ne $session })
if ($autresAilleurs.Count -gt 0) {
    Write-Host 'Info : autres copies dans d''autres sessions Windows (elles y restent jusqu''a la fermeture de ces sessions) :'
    $autresAilleurs | ForEach-Object { Write-Host "  - $($_.ExecutablePath) (session $($_.SessionId))" }
}
$autresRestent = $false
if ($autresIci.Count -gt 0) {
    Write-Host 'Autres copies de Keyboard Layout Fixer dans cette session (elles empechent la nouvelle de demarrer) :'
    $autresIci | ForEach-Object { Write-Host "  - $($_.ExecutablePath) (PID $($_.ProcessId))" }
    if ((Question 'Les arreter ?') -and (Arreter $autresIci 'Une autre copie')) { Write-Host 'Autres copies arretees.' } else { $autresRestent = $true }
}

# Scripts AutoHotkey de demarrage qui detournent F10 (ex. f10-klf.ahk "F10::^q" fait pour la
# version d'origine) : ils prendraient F10 a la place de ce programme.
# (dossiers Demarrage de TOUS les comptes du PC + commun ; non vus : raccourcis vers un .ahk
# ailleurs, scripts compiles en .exe)
$usersRoot = 'C:\Users'
$ahkDossiers = @(Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs\StartUp') + @(Get-ChildItem $usersRoot -Directory -ErrorAction SilentlyContinue | ForEach-Object { Join-Path $_.FullName 'AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup' })
$ahkMotif = '(?im)^\s*[~*$]*F(9|10)(\s+up|\s*&\s*\S+)?\s*::|^\s*Hotkey\s*[,(]\s*"?[~*$]*F(9|10)\b'
$ahk = @(foreach ($d in $ahkDossiers) { Get-ChildItem -LiteralPath $d -Filter '*.ahk' -File -ErrorAction SilentlyContinue | Where-Object { (Get-Content -LiteralPath $_.FullName -Raw -ErrorAction SilentlyContinue) -match $ahkMotif } })
if ($ahk.Count -gt 0) {
    Write-Host 'Scripts AutoHotkey de demarrage qui detournent F9/F10 (ils empecheraient ce programme de marcher) :'
    $ahk | ForEach-Object { Write-Host "  - $($_.FullName)" }
    if (Question 'Les arreter et les retirer du demarrage (une copie verifiee est gardee dans C:\ProgramData\KeyboardLayoutFixer-anciens-scripts, lisible par tous les comptes) ?') {
        # une sauvegarde par compte et par passage, jamais ecrasee ; copie VERIFIEE avant de retirer
        # l'original ; inventaire pour pouvoir remettre chaque script a sa place (audit 1.3.7, P1)
        $garde = Join-Path $env:ProgramData ("KeyboardLayoutFixer-anciens-scripts\" + (Get-Date -Format 'yyyyMMdd-HHmmss'))
        New-Item -ItemType Directory -Path $garde -Force | Out-Null
        $inventaire = Join-Path $garde 'inventaire.txt'
        $n = 0
        foreach ($f in $ahk) {
            $n++
            $compte = if ($f.FullName -match "^$([regex]::Escape($usersRoot))\\([^\\]+)\\") { $Matches[1] } else { 'commun' }
            $copie = Join-Path $garde ("{0:D2}-{1}-{2}" -f $n, $compte, $f.Name)
            try {
                $copiee = $false
                if (Test-Path -LiteralPath $copie) { throw 'une sauvegarde de ce nom existe deja' }
                Copy-Item -LiteralPath $f.FullName -Destination $copie
                if (-not (Pareils $f.FullName $copie)) { throw 'copie de sauvegarde non verifiee' }
                $copiee = $true
                Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object { $_.Name -match 'autohotkey' -and "$($_.CommandLine)".IndexOf($f.FullName, [StringComparison]::OrdinalIgnoreCase) -ge 0 } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
                Remove-Item -LiteralPath $f.FullName -Force
                Add-Content -LiteralPath $inventaire -Encoding UTF8 -Value "$copie`t<- $($f.FullName)"
                Write-Host "  retire : $($f.FullName) (copie : $copie)"
            } catch {
                Write-Host "  ATTENTION : $($f.FullName) non retire ($($_.Exception.Message))"
                if (-not $copiee -and (Test-Path -LiteralPath $copie)) { Remove-Item -LiteralPath $copie -Force -ErrorAction SilentlyContinue }
                if ($copiee) { Add-Content -LiteralPath $inventaire -Encoding UTF8 -Value "$copie`t<- $($f.FullName)`t(copie seule : original toujours en place)" -ErrorAction SilentlyContinue }
            }
        }
    } else { Write-Host '  Laisses en place : F9/F10 risquent de ne pas marcher tant qu''ils tournent.' }
}

# Retour arriere : remet l'etat initial, RELIT l'etat final, relance l'ancienne version
# si elle tournait ici ; dit precisement ce qui n'a pas pu etre remis (code 4).
function RetourArriere([string]$raison) {
    $restes = @()
    # programme : destination absente OU differente => on recopie la sauvegarde (audit 1.3.3 F2)
    if ($avant.Exe) {
        if (Test-Path -LiteralPath $Old) {
            # toujours recopier si on ne PROUVE pas que l'ancien est en place ; la sauvegarde n'est
            # supprimee qu'une fois la remise prouvee par deux empreintes valides
            if (-not (Pareils $Exe $Old)) { try { Copy-Item -LiteralPath $Old -Destination $Exe -Force } catch { } }
            if (Pareils $Exe $Old) { Remove-Item -LiteralPath $Old -Force -ErrorAction SilentlyContinue } else { $restes += "ancien programme : remise non prouvee (copie de secours gardee : $Old)" }
        } else { $restes += 'copie de secours absente' }
    } else {
        Remove-Item -LiteralPath $Exe -Force -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath $Exe) { $restes += "programme encore present : $Exe" }
    }
    Remove-Item -LiteralPath $New -Force -ErrorAction SilentlyContinue
    if (Test-Path -LiteralPath $New) { $restes += "fichier temporaire encore present : $New" }
    # demarrage automatique : remis, puis RELU (audit 1.3.3 F1)
    try {
        if ($null -ne $avant.Run) { New-ItemProperty -Path $RunKey -Name 'KeyboardLayoutFixer' -Value $avant.Run -PropertyType String -Force | Out-Null }
        else { Remove-ItemProperty -Path $RunKey -Name 'KeyboardLayoutFixer' -ErrorAction Stop }
    } catch { }
    $runApres = (Get-ItemProperty -Path $RunKey -Name 'KeyboardLayoutFixer' -ErrorAction SilentlyContinue).KeyboardLayoutFixer
    if ($runApres -ne $avant.Run) { $restes += "demarrage automatique non remis (valeur actuelle : '$runApres')" }
    if (-not $avant.Dossier) {
        Remove-Item -LiteralPath $Dest -Recurse -Force -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath $Dest) { $restes += "dossier encore present : $Dest" }
    }
    # l'ancienne version tournait dans cette session et elle est bien remise : on la relance
    $relance = ''
    if ($avant.Exe -and $nosIci) {
        if ($restes.Count -eq 0) { Start-Process -FilePath explorer.exe -ArgumentList "`"$Exe`""; $relance = "`nL'ancienne version a ete relancee." }
        else { $relance = "`nATTENTION : l'ancienne version n'a pas pu etre relancee." }
    }
    if ($restes.Count -gt 0) { Fin 4 ("ERREUR : $raison.`nInstallation PARTIELLE, le retour arriere n'a pas tout remis :`n  - " + ($restes -join "`n  - ") + $relance) }
    Fin 1 "ERREUR : $raison. Tout a ete remis comme avant (verifie).$relance"
}

# --- 3. mise en place ---
try {
    Move-Item -LiteralPath $New -Destination $Exe -Force
    if (-not (Pareils $Src $Exe)) { throw 'le programme mis en place est different de celui du zip (ou empreinte illisible)' }
    New-ItemProperty -Path $RunKey -Name 'KeyboardLayoutFixer' -Value "`"$Exe`"" -PropertyType String -Force | Out-Null
    if ((Get-ItemProperty -Path $RunKey -Name 'KeyboardLayoutFixer').KeyboardLayoutFixer -ne "`"$Exe`"") { throw 'demarrage automatique non enregistre' }
} catch {
    RetourArriere $_.Exception.Message
}
Remove-Item -LiteralPath $Old -Force -ErrorAction SilentlyContinue

# raccourci du menu Demarrer : ouvre les reglages, meme sans icone pres de l'horloge
try {
    $s = (New-Object -ComObject WScript.Shell).CreateShortcut($Lnk)
    $s.TargetPath = $Exe; $s.Arguments = '--settings'; $s.WorkingDirectory = $Dest
    $s.Description = 'F10 : corrige un texte tape avec le mauvais clavier (hebreu/anglais)'
    $s.Save()
} catch { Write-Host "ATTENTION : raccourci du menu Demarrer non cree ($($_.Exception.Message))." }

# ancien demarrage automatique PERSONNEL (compte qui installe seulement) vers une autre copie
$hkcu = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$perso = (Get-ItemProperty -Path $hkcu -Name 'KeyboardLayoutFixer' -ErrorAction SilentlyContinue).KeyboardLayoutFixer
if ($perso -and ($perso -notlike "*$Exe*")) {
    Write-Host "Demarrage automatique personnel vers une autre copie : $perso"
    if (Question 'Le retirer (la version installee demarre deja pour tous) ?') { Remove-ItemProperty -Path $hkcu -Name 'KeyboardLayoutFixer'; Write-Host 'Retire.' }
}

# --- lancement avec les droits normaux de l'utilisateur, puis verification ---
Start-Process -FilePath explorer.exe -ArgumentList "`"$Exe`""
$lance = $false
for ($i = 0; $i -lt 10 -and -not $lance; $i++) {
    Start-Sleep -Seconds 1
    $lance = [bool](Copies | Where-Object { $_.ExecutablePath -ieq $Exe -and $_.SessionId -eq $session })
}
$fin = "Installation terminee : $Exe.`nIl demarrera a chaque ouverture de session. Reglages : menu Demarrer, `"Keyboard Layout Fixer`".`nUtilisation : selectionnez le texte mal tape et appuyez sur F10."
if ($lance) { Fin 0 "$fin`nLe programme tourne (icone pres de l'horloge)." }
if ($autresRestent) { Fin 3 "$fin`nATTENTION : une autre copie tourne encore dans cette session ; la version installee demarrera quand elle sera fermee." }
if (-not $lance) { Fin 3 "$fin`nATTENTION : le lancement n'a pas pu etre confirme. Ouvrez une nouvelle session ou lancez-le depuis le menu Demarrer." }
Fin 0 "$fin`nLe programme tourne (icone pres de l'horloge)."
