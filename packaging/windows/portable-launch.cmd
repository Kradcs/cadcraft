@echo off
setlocal
if not exist "%~dp0cadcraft.exe" (
  echo Extraia TODO o ZIP antes de abrir o programa.
  pause
  exit /b 1
)
set "CADCRAFT_CODEX_EXECUTABLE=%~dp0codex\bin\codex.exe"
if not exist "%CADCRAFT_CODEX_EXECUTABLE%" (
  echo O pacote esta incompleto: codex\bin\codex.exe nao foi extraido.
  pause
  exit /b 1
)
start "" /D "%~dp0" "%~dp0cadcraft.exe"
