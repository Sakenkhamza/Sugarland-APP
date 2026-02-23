@echo off
set "LOGFILE=C:\Users\hsake\Projects\Sugarland-APP\execution.log"
echo "Starting Sugarland Dev..." > "%LOGFILE%"
set "PATH=%PATH%;C:\Users\hsake\.cargo\bin"
echo "Calling vcvars64.bat..." >> "%LOGFILE%"
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" >> "%LOGFILE%" 2>&1
if %errorlevel% neq 0 (
    echo "Failed to set up MSVC environment!" >> "%LOGFILE%"
    exit /b %errorlevel%
)
echo "Environment set up successfully." >> "%LOGFILE%"
echo "Starting npm run tauri dev..." >> "%LOGFILE%"
npm run tauri dev >> "%LOGFILE%" 2>&1
if %errorlevel% neq 0 (
    echo "npm run tauri dev failed!" >> "%LOGFILE%"
    exit /b %errorlevel%
)
