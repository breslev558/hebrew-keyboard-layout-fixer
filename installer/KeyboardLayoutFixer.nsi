; Keyboard Layout Fixer — installeur unique pour la version PUBLIQUE (1.6.x).
; Par utilisateur, sans droits administrateur : %LOCALAPPDATA%\Programs\KeyboardLayoutFixer,
; démarrage avec Windows (HKCU Run, valeur posée seulement, jamais la clé recréée), raccourci du
; menu Démarrer, entrée « Applications installées » et désinstalleur. Ferme proprement une ancienne
; copie (WM_CLOSE à la fenêtre cachée KBLayoutFixerMain). Compilé sur Linux par makensis.
; makensis -DVERSION=1.6.0 -DEXE=chemin\KeyboardLayoutFixer.exe -DICON=klf.ico installer/KeyboardLayoutFixer.nsi
Unicode true
SetCompressor /SOLID lzma
RequestExecutionLevel user
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "WinMessages.nsh"
; codes de sortie : 0 = installé, 1 = annulé (copie en marche non fermée), 3 = déjà installé pour tout le PC

!ifndef VERSION
  !error "VERSION manquante"
!endif
Name "Keyboard Layout Fixer"
; 1.7.0 : le même installeur pour le programme ARM64 (-DOUTFILE=KeyboardLayoutFixer-Setup-arm64.exe)
!ifdef OUTFILE
  OutFile "${OUTFILE}"
!else
  OutFile "KeyboardLayoutFixer-Setup.exe"
!endif
InstallDir "$LOCALAPPDATA\Programs\KeyboardLayoutFixer"
BrandingText "Keyboard Layout Fixer ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Keyboard Layout Fixer"
VIAddVersionKey "FileDescription" "Keyboard Layout Fixer setup"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "MIT License - Ravid Saadia, breslev558"
!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_FINISHPAGE_RUN "$INSTDIR\KeyboardLayoutFixer.exe"
!define MUI_FINISHPAGE_RUN_TEXT "$(LancerMaintenant)"

!define RUNKEY "Software\Microsoft\Windows\CurrentVersion\Run"
!define APPROVED "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
!define ARP "Software\Microsoft\Windows\CurrentVersion\Uninstall\KeyboardLayoutFixer"

!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "French"
!insertmacro MUI_LANGUAGE "Hebrew"

LangString LancerMaintenant ${LANG_ENGLISH} "Start Keyboard Layout Fixer"
LangString LancerMaintenant ${LANG_FRENCH} "Lancer Keyboard Layout Fixer"
LangString LancerMaintenant ${LANG_HEBREW} "הפעל את Keyboard Layout Fixer"
LangString DejaMachine ${LANG_ENGLISH} "Keyboard Layout Fixer is already installed for the whole PC by an administrator. Nothing was changed."
LangString DejaMachine ${LANG_FRENCH} "Keyboard Layout Fixer est déjà installé pour tout le PC par un administrateur. Rien n'a été changé."
LangString DejaMachine ${LANG_HEBREW} "Keyboard Layout Fixer כבר מותקן לכל המחשב על ידי מנהל מערכת. לא שונה דבר."
LangString PasFerme ${LANG_ENGLISH} "The running copy did not close. Close it from its tray icon (right-click > Exit), then try again."
LangString PasFerme ${LANG_FRENCH} "Le programme en cours d'exécution ne s'est pas fermé. Fermez-le depuis son icône (clic droit > Quitter), puis réessayez."
LangString PasFerme ${LANG_HEBREW} "העותק הפועל לא נסגר. סגרו אותו מהסמל (לחיצה ימנית > יציאה) ונסו שוב."
LangString EffacerAussi ${LANG_ENGLISH} "Also delete your settings and the words the program learned (my-words.txt)?"
LangString EffacerAussi ${LANG_FRENCH} "Effacer aussi vos réglages et les mots que le programme a appris (my-words.txt) ?"
LangString EffacerAussi ${LANG_HEBREW} "למחוק גם את ההגדרות ואת המילים שהתוכנה למדה (my-words.txt)?"

Var Premiere

; ferme la copie en marche de CET utilisateur (message de fermeture, comme l'installeur PowerShell), 30 s au plus
!macro FermerCopie UN
Function ${UN}FermerCopie
  StrCpy $1 0
  boucle:
    FindWindow $0 "KBLayoutFixerMain" ""
    ${If} $0 = 0
      Goto libere
    ${EndIf}
    ${If} $1 = 0
      SendMessage $0 ${WM_CLOSE} 0 0 /TIMEOUT=2000
    ${EndIf}
    Sleep 500
    IntOp $1 $1 + 1
    ${If} $1 < 60
      Goto boucle
    ${EndIf}
  IfSilent +2
  MessageBox MB_ICONEXCLAMATION "$(PasFerme)"
  Abort
  libere:
  ; la fenêtre est fermée mais le processus peut tenir encore le fichier un instant : on attend
  ; qu'il soit libre (10 s au plus) avant de l'écrire ou de l'effacer
  StrCpy $1 0
  ${If} ${FileExists} "$INSTDIR\KeyboardLayoutFixer.exe"
    attente:
      ClearErrors
      FileOpen $2 "$INSTDIR\KeyboardLayoutFixer.exe" a
      ${IfNot} ${Errors}
        FileClose $2
        Return
      ${EndIf}
      Sleep 250
      IntOp $1 $1 + 1
      ${If} $1 < 40
        Goto attente
      ${EndIf}
    IfSilent +2
    MessageBox MB_ICONEXCLAMATION "$(PasFerme)"
    Abort
  ${EndIf}
FunctionEnd
!macroend
!insertmacro FermerCopie ""
!insertmacro FermerCopie "un."

Function .onInit
  ${If} ${FileExists} "$PROGRAMFILES64\KeyboardLayoutFixer\KeyboardLayoutFixer.exe"
  ${OrIf} ${FileExists} "$PROGRAMFILES32\KeyboardLayoutFixer\KeyboardLayoutFixer.exe"
    IfSilent +2
    MessageBox MB_ICONINFORMATION "$(DejaMachine)"
    SetErrorLevel 3
    Quit
  ${EndIf}
FunctionEnd

Section "Keyboard Layout Fixer"
  Call FermerCopie
  StrCpy $Premiere 1
  ${If} ${FileExists} "$INSTDIR\KeyboardLayoutFixer.exe"
    StrCpy $Premiere 0
  ${EndIf}
  SetOutPath "$INSTDIR"
  File "/oname=KeyboardLayoutFixer.exe" "${EXE}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  ; démarrage avec Windows : seulement à la première installation (une mise à jour garde le choix)
  ${If} $Premiere = 1
    WriteRegStr HKCU "${RUNKEY}" "KeyboardLayoutFixer" '"$INSTDIR\KeyboardLayoutFixer.exe"'
    DeleteRegValue HKCU "${APPROVED}" "KeyboardLayoutFixer"
  ${EndIf}
  CreateShortcut "$SMPROGRAMS\Keyboard Layout Fixer.lnk" "$INSTDIR\KeyboardLayoutFixer.exe" "--settings" "$INSTDIR\KeyboardLayoutFixer.exe" 0
  WriteRegStr HKCU "${ARP}" "DisplayName" "Keyboard Layout Fixer"
  WriteRegStr HKCU "${ARP}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${ARP}" "Publisher" "breslev558 (MIT, based on Ravid Saadia)"
  WriteRegStr HKCU "${ARP}" "DisplayIcon" "$INSTDIR\KeyboardLayoutFixer.exe,0"
  WriteRegStr HKCU "${ARP}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${ARP}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "${ARP}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegStr HKCU "${ARP}" "URLInfoAbout" "https://github.com/breslev558/hebrew-keyboard-layout-fixer"
  WriteRegDWORD HKCU "${ARP}" "NoModify" 1
  WriteRegDWORD HKCU "${ARP}" "NoRepair" 1
  WriteRegDWORD HKCU "${ARP}" "EstimatedSize" 700
  ; installation silencieuse (/S) : le programme est lancé aussi
  IfSilent 0 +2
  Exec '"$INSTDIR\KeyboardLayoutFixer.exe"'
SectionEnd

Section "Uninstall"
  Call un.FermerCopie
  ReadRegStr $0 HKCU "${RUNKEY}" "KeyboardLayoutFixer"
  ${If} $0 == '"$INSTDIR\KeyboardLayoutFixer.exe"'
    DeleteRegValue HKCU "${RUNKEY}" "KeyboardLayoutFixer"
  ${EndIf}
  DeleteRegValue HKCU "${APPROVED}" "KeyboardLayoutFixer"
  Delete "$SMPROGRAMS\Keyboard Layout Fixer.lnk"
  Delete "$INSTDIR\KeyboardLayoutFixer.exe"
  Delete "$INSTDIR\KeyboardLayoutFixer.exe.old"
  Delete "$INSTDIR\KeyboardLayoutFixer.exe.new"
  Delete "$INSTDIR\Uninstall.exe"
  ClearErrors
  RMDir "$INSTDIR"
  ; l'entrée « Applications installées » n'est retirée que si le dossier a bien disparu
  ${IfNot} ${Errors}
    DeleteRegKey HKCU "${ARP}"
  ${EndIf}
  ; les réglages (%APPDATA%\KeyboardLayoutFixer) sont gardés, sauf si l'utilisateur demande de
  ; les effacer aussi (1.8.7 : réglages, mots appris, listes ; jamais en silencieux, non par défaut)
  ; seulement si le programme est bien parti, pour ce compte, et si le dossier existe vraiment
  IfSilent garder
  IfFileExists "$INSTDIR\KeyboardLayoutFixer.exe" garder
  SetShellVarContext current
  StrCmp "$APPDATA" "" garder
  IfFileExists "$APPDATA\KeyboardLayoutFixer\*.*" 0 garder
  MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "$(EffacerAussi)" IDNO garder
  RMDir /r "$APPDATA\KeyboardLayoutFixer"
  garder:
SectionEnd
