@echo off
rem Atlas Daedalus Native Direct Launcher (Pure Rust Engine)
set "TARGET_BIN=F:\projects\Atlas-Research-Labs\Atlas-Daedalus\npm\bin\acode.exe"
if exist "%TARGET_BIN%" (
  "%TARGET_BIN%" %*
  exit /b %ERRORLEVEL%
)
node "%~dp0acode.js" %*
