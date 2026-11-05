@echo off
setlocal
set "PRODUCTION_CYCLE_STARTUP_REPORT=%TEMP%\ProductionCycle-startup-%RANDOM%-%RANDOM%.json"
"%~dp0..\production-cycle.exe"
echo.
echo Startup report: "%PRODUCTION_CYCLE_STARTUP_REPORT%"
echo Send this file to the application maintainer if the window did not open.
pause
