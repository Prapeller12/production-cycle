@echo off
setlocal
cd /d "%~dp0.."
if not exist "runtime\webview2\msedgewebview2.exe" (
 echo Fixed WebView2 Runtime is missing. Extract the complete portable ZIP.
 pause
 exit /b 1
)
if not exist "production-cycle.exe" (
 echo Application executable is missing. This source folder is not a Windows release.
 pause
 exit /b 1
)
start "" /wait "production-cycle.exe"
if errorlevel 1 (
 echo Application could not start. Check that the program folder is writable.
 pause
 exit /b 1
)
