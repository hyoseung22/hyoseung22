"""Double-click friendly launcher for GUI-only operation."""

from __future__ import annotations

import platform
import traceback
from pathlib import Path


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


def main() -> int:
    try:
        from band_auto_poster.gui import launch_gui

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
