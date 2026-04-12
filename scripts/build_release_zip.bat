@echo off
setlocal EnableExtensions

set "SCRIPT_DIR=%~dp0"
set "PS_SCRIPT=%SCRIPT_DIR%build_release_zip.ps1"

if not exist "%PS_SCRIPT%" (
  echo [ERROR] PowerShell script not found: %PS_SCRIPT%
  exit /b 1
)

set "VERSION=%~1"
if "%VERSION%"=="" (
  set /p "VERSION=Enter release version (example: v1.0.1): "
)

if "%VERSION%"=="" (
  echo [ERROR] Version is empty.
  exit /b 1
)

echo.
echo Release version: %VERSION%
set /p "CONFIRM=Create release zip with this version? [y/N]: "
if /I not "%CONFIRM%"=="y" (
  echo Canceled.
  exit /b 0
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%PS_SCRIPT%" -Version "%VERSION%"
if errorlevel 1 (
  echo [ERROR] Failed to create release zip.
  exit /b 1
)

echo Done.
exit /b 0
