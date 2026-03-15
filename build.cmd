@echo off
chcp 65001 >nul
REM Run build.ps1 with Bypass (no ExecutionPolicy change)
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build.ps1" %*
