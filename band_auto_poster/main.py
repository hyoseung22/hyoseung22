from __future__ import annotations

import argparse
import logging
import signal
import sys
import time

from apscheduler.schedulers.background import BackgroundScheduler
from playwright.sync_api import sync_playwright

from .auth import AuthError, create_context, ensure_logged_in
from .config import AppConfig, load_config
from .logger import setup_logging
from .poster import PostError, capture_failure, post_message
from .scheduler import within_active_window

logger = logging.getLogger(__name__)
_SHUTDOWN = False


def _signal_handler(signum: int, _frame: object) -> None:
    global _SHUTDOWN
    _SHUTDOWN = True
    logger.info("Signal received: %s", signum)


def run_once(cfg: AppConfig) -> None:
    if not within_active_window(cfg.schedule):
        logger.info("Outside active window. Skip this cycle.")
        return

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        context = create_context(browser, cfg)
        page = context.new_page()

        ensure_logged_in(page, context, cfg)

        for attempt in range(1, cfg.retry.max_attempts + 1):
            try:
                post_message(page, cfg)
                logger.info("Post succeeded on attempt %s", attempt)
                break
            except PostError as exc:
                screenshot = capture_failure(page, attempt)
                logger.error("Post failed (attempt=%s): %s / screenshot=%s", attempt, exc, screenshot)
                if attempt == cfg.retry.max_attempts:
                    raise
                time.sleep(cfg.retry.backoff_seconds)

        context.close()
        browser.close()


def run_service(cfg: AppConfig) -> None:
    scheduler = BackgroundScheduler(timezone=cfg.schedule.timezone)
    scheduler.add_job(
        lambda: _job_wrapper(cfg),
        trigger="interval",
        minutes=cfg.schedule.interval_minutes,
        id="band_post_job",
        max_instances=1,
        coalesce=True,
    )
    scheduler.start()
    logger.info("Scheduler started: every %s minutes", cfg.schedule.interval_minutes)

    while not _SHUTDOWN:
        time.sleep(1)

    scheduler.shutdown(wait=False)
    logger.info("Service stopped")


def _job_wrapper(cfg: AppConfig) -> None:
    try:
        run_once(cfg)
    except AuthError as exc:
        logger.error("Authentication failed: %s", exc)
    except Exception:
        logger.exception("Unexpected error during posting job")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="NaverBand auto poster")
    parser.add_argument("--config", required=True, help="Path to YAML config")
    parser.add_argument("--once", action="store_true", help="Run one cycle and exit")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    cfg = load_config(args.config)

    setup_logging(cfg.logging.level, cfg.logging.file)

    signal.signal(signal.SIGINT, _signal_handler)
    signal.signal(signal.SIGTERM, _signal_handler)

    if args.once:
        _job_wrapper(cfg)
        return 0

    run_service(cfg)
    return 0


if __name__ == "__main__":
    sys.exit(main())
