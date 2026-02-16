"""Double-click friendly launcher for GUI-only operation."""

from __future__ import annotations

import importlib
import importlib.util
import platform
import subprocess
import sys
import traceback
from pathlib import Path
from typing import Callable


REQUIRED_MODULES = ("yaml", "apscheduler", "playwright")


def _wait_if_needed() -> None:
    # On Windows, keep console open so users can read the error.
    if platform.system().lower().startswith("win"):
        try:
            input("\n[ERROR] 실행 실패. 엔터 키를 누르면 종료됩니다... ")
        except EOFError:
            pass


def _write_error_log(message: str) -> Path:
    log_dir = Path("logs")
    log_dir.mkdir(parents=True, exist_ok=True)
    path = log_dir / "run_gui_error.log"
    path.write_text(message, encoding="utf-8")
    return path


def _missing_modules() -> list[str]:
    return [name for name in REQUIRED_MODULES if importlib.util.find_spec(name) is None]


def _install_requirements() -> None:
    req = Path("requirements.txt")
    if not req.exists():
        raise FileNotFoundError("requirements.txt 파일을 찾을 수 없습니다.")

    print("[INFO] 필요한 패키지가 없어 자동 설치를 시도합니다...")
    subprocess.run([sys.executable, "-m", "pip", "install", "-r", str(req)], check=True)


def _load_gui_launcher() -> Callable[[str], None]:
    module = importlib.import_module("band_auto_poster.gui")
    launcher = getattr(module, "launch_gui", None)
    if launcher is None:
        raise RuntimeError("band_auto_poster.gui.launch_gui 를 찾을 수 없습니다.")
    return launcher


def main() -> int:
    try:
        missing = _missing_modules()
        if missing:
            print(f"[WARN] 누락된 패키지 감지: {', '.join(missing)}")
            _install_requirements()

        launch_gui = _load_gui_launcher()
        launch_gui("config.yaml")
        return 0
    except Exception:
        detail = traceback.format_exc()
        log_path = _write_error_log(detail)
        print("\n[ERROR] run_gui_app 실행 중 오류가 발생했습니다.")
        print(f"[ERROR] 상세 로그: {log_path}")
        print(detail)
        _wait_if_needed()
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
