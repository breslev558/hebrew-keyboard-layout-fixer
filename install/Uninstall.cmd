@echo off
rem Keyboard Layout Fixer: launcher. All the work is done by Uninstall.ps1 (current user, no administrator rights).
set "KLF_PS1=%~dp0Uninstall.ps1"
if not exist "%KLF_PS1%" (echo Please extract the zip first: right click ^> Extract All. & pause & exit /b 1)
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -Command "Unblock-File -LiteralPath $env:KLF_PS1 -ErrorAction SilentlyContinue; & $env:KLF_PS1 %*; exit $LASTEXITCODE"
if errorlevel 1 if not errorlevel 2 (echo. & echo If nothing was installed, PowerShell scripts may be blocked on this PC by its administrator.)
exit /b %errorlevel%
