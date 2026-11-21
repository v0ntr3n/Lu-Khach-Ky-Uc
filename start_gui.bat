@echo off
setlocal enabledelayedexpansion

cd /d "%~dp0"

set "GUI_MANIFEST=crates\nro-admin-ui\Cargo.toml"
set "GUI_BIN=crates\nro-admin-ui\target\release\nro-server-gui.exe"
set "REBUILD=0"
set "HAS_CONFIG=0"
set "HAS_ASSETS_DIR=0"
set "FORWARD_ARGS="

:parse_args
if "%~1"=="" goto :args_done
set "ARG=%~1"
if /i "%ARG%"=="--rebuild" (
    set "REBUILD=1"
    shift
    goto :parse_args
)
if /i "%ARG%"=="-c" set "HAS_CONFIG=1"
if /i "%ARG%"=="--config" set "HAS_CONFIG=1"
if /i "%ARG:~0,9%"=="--config=" set "HAS_CONFIG=1"
if /i "%ARG%"=="--assets-dir" set "HAS_ASSETS_DIR=1"
if /i "%ARG:~0,13%"=="--assets-dir=" set "HAS_ASSETS_DIR=1"
set "FORWARD_ARGS=!FORWARD_ARGS! %1"
shift
goto :parse_args

:args_done
if not exist "%GUI_BIN%" set "REBUILD=1"

if "%REBUILD%"=="1" (
    echo [start_gui] Building nro-server-gui ^(GPUI Kit + embedded nro-server^)...
    cargo build --manifest-path "%GUI_MANIFEST%" --release
    if errorlevel 1 (
        echo [start_gui] ERROR: cargo build failed.
        exit /b 1
    )
)

set "CONFIG_FILE=%NRO_CONFIG%"
if "%CONFIG_FILE%"=="" set "CONFIG_FILE=server.toml"

set "ASSETS_DIR=%NRO_ASSETS_DIR%"
if "%ASSETS_DIR%"=="" (
    for %%D in ("SRC\data" "Copy of Teamobi2026\SRC\data" "Teamobi2026\SRC\data" "data") do (
        if "!ASSETS_DIR!"=="" (
            if exist "%%~D\update_data\" set "ASSETS_DIR=%%~D"
            if exist "%%~D\icon\" set "ASSETS_DIR=%%~D"
        )
    )
)

set "DEFAULT_ARGS="
if "%HAS_CONFIG%"=="0" set "DEFAULT_ARGS=--config "%CONFIG_FILE%""
if "%HAS_ASSETS_DIR%"=="0" if not "%ASSETS_DIR%"=="" set "DEFAULT_ARGS=!DEFAULT_ARGS! --assets-dir "%ASSETS_DIR%""

echo [start_gui] Launching %GUI_BIN% %DEFAULT_ARGS% %FORWARD_ARGS%
"%GUI_BIN%" %DEFAULT_ARGS% %FORWARD_ARGS%
