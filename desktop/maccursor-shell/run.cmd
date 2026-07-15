@echo off
setlocal

where cargo >nul 2>nul
if errorlevel 1 (
  echo Bewegungskrieg desktop shell requires Cargo on PATH. 1>&2
  exit /b 1
)

if not defined CARGO_BUILD_JOBS set "CARGO_BUILD_JOBS=2"
if not defined CARGO_TARGET_DIR set "CARGO_TARGET_DIR=%LOCALAPPDATA%\rts-0\tauri-target-windows"

cargo run --manifest-path "%~dp0src-tauri\Cargo.toml" -- %*
exit /b %errorlevel%
