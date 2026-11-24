@echo off
setlocal
set "debug_exe=%~dp0target\debug\pdf-studio.exe"
set "release_exe=%~dp0target\release\pdf-studio.exe"
set "pdf_studio_exe="
if exist "%debug_exe%" if exist "%release_exe%" (
    for /f "usebackq delims=" %%I in (`powershell -NoProfile -Command "if ((Get-Item -LiteralPath '%debug_exe%').LastWriteTimeUtc -ge (Get-Item -LiteralPath '%release_exe%').LastWriteTimeUtc) { Write-Output '%debug_exe%' } else { Write-Output '%release_exe%' }"`) do set "pdf_studio_exe=%%I"
) else if exist "%debug_exe%" (
    set "pdf_studio_exe=%debug_exe%"
) else if exist "%release_exe%" (
    set "pdf_studio_exe=%release_exe%"
)
if not defined pdf_studio_exe (
    echo Aplikasi belum dibangun. Lihat README.md.
    pause
    exit /b 1
)
start "" /D "%~dp0" "%pdf_studio_exe%" %*
