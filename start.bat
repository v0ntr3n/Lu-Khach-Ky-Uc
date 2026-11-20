@echo off
setlocal EnableExtensions
chcp 65001 >nul
pushd "%~dp0"
title Teamobi2026 - nro-server [TCP 14445]

if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)

set "REBUILD=0"
set "FORWARD_ARGS="
:parse_args
if "%~1"=="" goto args_done
if /I "%~1"=="--rebuild" (
    set "REBUILD=1"
) else (
    set "FORWARD_ARGS=%FORWARD_ARGS% "%~1""
)
shift
goto parse_args
:args_done

if defined NRO_CONFIG (
    set "CONFIG_FILE=%NRO_CONFIG%"
) else (
    set "CONFIG_FILE=server.toml"
)

if defined NRO_ASSETS_DIR (
    set "ASSETS_DIR=%NRO_ASSETS_DIR%"
) else if exist "SRC\data\update_data\" (
    set "ASSETS_DIR=SRC\data"
) else if exist "SRC\data\icon\" (
    set "ASSETS_DIR=SRC\data"
) else if exist "Copy of Teamobi2026\SRC\data\update_data\" (
    set "ASSETS_DIR=Copy of Teamobi2026\SRC\data"
) else if exist "Teamobi2026\SRC\data\update_data\" (
    set "ASSETS_DIR=Teamobi2026\SRC\data"
) else if exist "data\update_data\" (
    set "ASSETS_DIR=data"
) else (
    set "ASSETS_DIR=SRC\data"
)

if "%REBUILD%"=="1" goto build_from_source
if exist "nro-server.exe" (
    set "SERVER_BIN=.\nro-server.exe"
    goto bin_ready
)
if exist "Cargo.toml" (
    where cargo >nul 2>nul
    if not errorlevel 1 goto build_from_source
)
if exist "target\release\nro-server.exe" (
    set "SERVER_BIN=.\target\release\nro-server.exe"
    goto bin_ready
)

:build_from_source
where cargo >nul 2>nul
if errorlevel 1 (
    echo [ERROR] Neither nro-server.exe nor Cargo toolchain was found.
    echo         Install Rust 1.100.0+ from https://rustup.rs/ with C++ Build Tools.
    popd
    pause
    exit /b 1
)

echo === Ensuring nro-server.exe and nro-import-sql.exe release build is up to date ===
cargo build --release -p nro-server -p nro-db
if not errorlevel 1 goto build_ok

where rustup >nul 2>nul
if errorlevel 1 goto build_failed

echo [INFO] Retrying build with Rust 1.100.0 / nightly toolchain...
cargo +1.100.0 build --release -p nro-server -p nro-db
if not errorlevel 1 goto build_ok

cargo +nightly build --release -p nro-server -p nro-db
if not errorlevel 1 goto build_ok

:build_failed
echo [ERROR] Release build failed. Run: rustup toolchain install 1.100.0
popd
pause
exit /b 1

:build_ok
set "SERVER_BIN=.\target\release\nro-server.exe"

:bin_ready
if not exist "%ASSETS_DIR%\update_data\" if not exist "%ASSETS_DIR%\icon\" (
    echo [WARN] Binary client assets not found at "%ASSETS_DIR%".
    echo        Copy Teamobi2026 "SRC\data" into "%CD%\SRC\data" before connecting with Mod Local.
)

echo === Starting Teamobi2026 nro-server [Press Ctrl-C or type baotri to save and stop] ===
"%SERVER_BIN%" --config "%CONFIG_FILE%" --assets-dir "%ASSETS_DIR%" %FORWARD_ARGS%
set "EXIT_CODE=%ERRORLEVEL%"
if not "%EXIT_CODE%"=="0" (
    echo [ERROR] nro-server exited with code %EXIT_CODE%.
    pause
)
popd
exit /b %EXIT_CODE%
