@echo off
setlocal
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -STA -ExecutionPolicy RemoteSigned -File "%~dp0Pebrel-Menu.ps1"
if errorlevel 1 pause
