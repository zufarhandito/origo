@echo off
setlocal
set "pdf_studio_exe=%~dp0target\release\pdf-studio.exe"
if not exist "%pdf_studio_exe%" set "pdf_studio_exe=%~dp0target\debug\pdf-studio.exe"
if not exist "%pdf_studio_exe%" (
    echo Aplikasi belum dibangun. Lihat README.md.
    pause
    exit /b 1
)
start "" /D "%~dp0" "%pdf_studio_exe%" %*
