@echo off
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
set PATH=%USERPROFILE%\.cargo\bin;%PATH%
cd /d C:\Users\hsake\Projects\Sugarland-APP\src-tauri
cargo clean 2>nul
set RUST_BACKTRACE=1
cargo build 2>&1 | findstr /i "error link"
echo ---
where link.exe
echo ---
where cl.exe
