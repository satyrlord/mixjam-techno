@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0run.ps1" %*
rem Keep the window open after an error so that its message can be read.
if errorlevel 1 pause
