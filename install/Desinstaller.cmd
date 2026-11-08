@echo off
rem Keyboard Layout Fixer (version NOAM) : lanceur. Tout le travail est fait par Desinstaller.ps1
rem (droits administrateur, arret propre, copie verifiee, retour arriere, verification).
rem Le chemin est passe comme donnee (variable), jamais insere dans du code.
set "KLF_PS1=%~dp0Desinstaller.ps1"
powershell -NoProfile -ExecutionPolicy Bypass -Command "Unblock-File -LiteralPath $env:KLF_PS1 -ErrorAction SilentlyContinue; & $env:KLF_PS1 %*; exit $LASTEXITCODE"
exit /b %errorlevel%
