@echo off
cd /d %~dp0

python run_gui_app.py
if errorlevel 1 (
  echo.
  echo [ERROR] 실행에 실패했습니다. logs\run_gui_error.log 파일을 확인하세요.
  pause
)
