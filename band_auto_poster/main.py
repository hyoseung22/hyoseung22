from __future__ import annotations

import argparse
import logging
import signal
import sys
import time
from datetime import datetime
from threading import Event
from typing import Callable

from apscheduler.schedulers.background import BackgroundScheduler
from playwright.sync_api import sync_playwright

from .auth import AuthError, create_context, ensure_logged_in
from .config import AppConfig, load_config
from .logger import setup_logging
from .poster import PostError, capture_failure, post_message
from .scheduler import within_active_window

logger = logging.getLogger(__name__)


StatusCallback = Callable[[str], None]


def run_once(cfg: AppConfig, status_callback: StatusCallback | None = None) -> None:
    _notify(status_callback, "실행 시작")
    if not within_active_window(cfg.schedule):
        message = "활성 시간대가 아니어서 이번 사이클은 건너뜁니다."
        logger.info(message)
        _notify(status_callback, message)
        return

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        context = create_context(browser, cfg)
        page = context.new_page()

        ensure_logged_in(page, context, cfg)
        _notify(status_callback, "로그인 성공")

        for attempt in range(1, cfg.retry.max_attempts + 1):
            try:
                post_message(page, cfg)
                message = f"게시 성공 (시도 {attempt}/{cfg.retry.max_attempts})"
                logger.info(message)
                _notify(status_callback, message)
                break
            except PostError as exc:
                screenshot = capture_failure(page, attempt)
                logger.error("Post failed (attempt=%s): %s / screenshot=%s", attempt, exc, screenshot)
                _notify(status_callback, f"게시 실패 (시도 {attempt}): {exc}")
                if attempt == cfg.retry.max_attempts:
                    raise
                time.sleep(cfg.retry.backoff_seconds)

        context.close()
        browser.close()


def run_dry(cfg: AppConfig, status_callback: StatusCallback | None = None) -> list[str]:
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    is_active = within_active_window(cfg.schedule)

    report = [
        "[DRY-RUN] NaverBand 자동 게시 설정 점검",
        f"- 현재 시각: {now}",
        f"- 대상 밴드: {cfg.band.target_url}",
        f"- 주기: {cfg.schedule.interval_minutes}분",
        f"- 활성 시간: {cfg.schedule.active_time.start} ~ {cfg.schedule.active_time.end} ({cfg.schedule.timezone})",
        f"- 활성 시간 여부: {'활성' if is_active else '비활성'}",
        f"- 스타일: bold={cfg.band.style.bold}, size={cfg.band.style.font_size}, color={cfg.band.style.font_color}",
        "- 결과: 실제 로그인/게시 없이 설정과 실행 경로만 점검했습니다.",
    ]

    for line in report:
        logger.info(line)
        _notify(status_callback, line)

    return report


def run_service(
    cfg: AppConfig,
    stop_event: Event | None = None,
    status_callback: StatusCallback | None = None,
) -> None:
    stop_event = stop_event or Event()
    scheduler = BackgroundScheduler(timezone=cfg.schedule.timezone)
    scheduler.add_job(
        lambda: _job_wrapper(cfg, status_callback),
        trigger="interval",
        minutes=cfg.schedule.interval_minutes,
        id="band_post_job",
        max_instances=1,
        coalesce=True,
    )
    scheduler.start()
    logger.info("Scheduler started: every %s minutes", cfg.schedule.interval_minutes)
    _notify(status_callback, f"스케줄러 시작 (매 {cfg.schedule.interval_minutes}분)")

    try:
        while not stop_event.is_set():
            time.sleep(0.5)
    finally:
        scheduler.shutdown(wait=False)
        logger.info("Service stopped")
        _notify(status_callback, "서비스가 중지되었습니다.")


def _job_wrapper(cfg: AppConfig, status_callback: StatusCallback | None = None) -> None:
    try:
        run_once(cfg, status_callback=status_callback)
    except AuthError as exc:
        logger.error("Authentication failed: %s", exc)
        _notify(status_callback, f"인증 실패: {exc}")
    except Exception:
        logger.exception("Unexpected error during posting job")
        _notify(status_callback, "예상치 못한 오류가 발생했습니다. 로그를 확인하세요.")


def _notify(status_callback: StatusCallback | None, message: str) -> None:
    if status_callback:
        status_callback(message)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="NaverBand auto poster")
    parser.add_argument("--config", default="config.yaml", help="Path to YAML config")
    parser.add_argument("--once", action="store_true", help="Run one cycle and exit")
    parser.add_argument("--gui", action="store_true", help="Run desktop GUI")
    parser.add_argument("--dry-run", action="store_true", help="Validate settings without real login/post")
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    if args.gui:
        from .gui import launch_gui

        setup_logging("INFO", "./logs/app.log")
        launch_gui(config_path=args.config)
        return 0

    cfg = load_config(args.config)
    setup_logging(cfg.logging.level, cfg.logging.file)

    if args.dry_run:
        run_dry(cfg)
        return 0

    stop_event = Event()

    def _signal_handler(signum: int, _frame: object) -> None:
        stop_event.set()
        logger.info("Signal received: %s", signum)

    signal.signal(signal.SIGINT, _signal_handler)
    signal.signal(signal.SIGTERM, _signal_handler)

    if args.once:
        _job_wrapper(cfg)
        return 0

    run_service(cfg, stop_event=stop_event)
    return 0


if __name__ == "__main__":
    sys.exit(main())
