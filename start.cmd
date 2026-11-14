@echo off
setlocal
if not exist "%~dp0..\runtime\webview2\msedgewebview2.exe" (
 echo Fixed WebView2 Runtime is missing. Extract the complete portable ZIP.
 pause
 exit /b 1
)
if not exist "%~dp0..\production-cycle.exe" (
 echo Application executable is missing. This source folder is not a Windows release.
 pause
 exit /b 1
)
start "" /wait "%~dp0..\production-cycle.exe" %*
if errorlevel 1 (
 echo Application could not start. Check that the program folder is writable.
 pause
 exit /b 1
)
exit /b 0
