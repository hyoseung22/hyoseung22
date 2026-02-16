from __future__ import annotations

import logging
import os
from pathlib import Path

from playwright.sync_api import Browser, BrowserContext, Page, TimeoutError as PlaywrightTimeoutError

from .config import AppConfig

logger = logging.getLogger(__name__)


class AuthError(RuntimeError):
    """Raised when login cannot be completed."""


def _load_credentials(cfg: AppConfig) -> tuple[str, str]:
    user = os.getenv(cfg.naver.username_env, "").strip()
    pwd = os.getenv(cfg.naver.password_env, "").strip()

    if not user or not pwd:
        raise AuthError(
            f"Missing credentials. Set {cfg.naver.username_env} and {cfg.naver.password_env} environment variables."
        )

    return user, pwd


def create_context(browser: Browser, cfg: AppConfig) -> BrowserContext:
    session_path = Path(cfg.naver.session_file)
    session_path.parent.mkdir(parents=True, exist_ok=True)

    if cfg.naver.use_saved_session and session_path.exists():
        logger.info("Using saved session: %s", session_path)
        return browser.new_context(storage_state=str(session_path))

    return browser.new_context()


def ensure_logged_in(page: Page, context: BrowserContext, cfg: AppConfig) -> None:
    page.goto("https://auth.band.us/login", wait_until="domcontentloaded")

    if _is_logged_in(page):
        logger.info("Session already authenticated")
        _save_session(context, cfg)
        return

    user, pwd = _load_credentials(cfg)

    logger.info("Logging in with id/password")
    page.fill('input[name="email"]', user)
    page.fill('input[name="password"]', pwd)
    page.click('button[type="submit"]')

    try:
        page.wait_for_url("**/band/**", timeout=15_000)
    except PlaywrightTimeoutError as exc:
        if _is_logged_in(page):
            logger.info("Logged in successfully after redirect delay")
        else:
            raise AuthError("Login failed or additional verification is required (2FA/CAPTCHA).") from exc

    if not _is_logged_in(page):
        raise AuthError("Login status could not be validated.")

    _save_session(context, cfg)


def _is_logged_in(page: Page) -> bool:
    url = page.url
    if "band.us/band/" in url:
        return True

    # Best-effort element checks, can change with site updates.
    indicators = [
        'a[href*="/band/"]',
        'button:has-text("글쓰기")',
        'button:has-text("Write")',
    ]
    for selector in indicators:
        locator = page.locator(selector)
        if locator.count() > 0:
            return True
    return False


def _save_session(context: BrowserContext, cfg: AppConfig) -> None:
    path = Path(cfg.naver.session_file)
    context.storage_state(path=str(path))
    logger.info("Session saved to %s", path)
