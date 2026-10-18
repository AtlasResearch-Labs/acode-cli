@echo off
rem Atlas Daedalus Native Direct Launcher (Pure Rust Engine)
if exist "%~dp0acode-win-x64.exe" (
  "%~dp0acode-win-x64.exe" %*
  exit /b %ERRORLEVEL%
)
if exist "%~dp0acode.exe" (
  "%~dp0acode.exe" %*
  exit /b %ERRORLEVEL%
)
node "%~dp0acode.js" %*
