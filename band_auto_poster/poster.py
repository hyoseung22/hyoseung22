from __future__ import annotations

import logging
from pathlib import Path
from time import sleep

from playwright.sync_api import Page, TimeoutError as PlaywrightTimeoutError

from .config import AppConfig

logger = logging.getLogger(__name__)


class PostError(RuntimeError):
    """Raised when a post operation fails."""


def post_message(page: Page, cfg: AppConfig) -> None:
    page.goto(cfg.band.target_url, wait_until="domcontentloaded")
    _open_editor(page)
    _write_content(page, cfg.band.post_text)
    _apply_style(page, cfg)
    _submit_post(page)


def _open_editor(page: Page) -> None:
    write_candidates = [
        'button:has-text("글쓰기")',
        'a:has-text("글쓰기")',
        'button:has-text("Write")',
    ]
    for selector in write_candidates:
        locator = page.locator(selector).first
        if locator.count() > 0:
            locator.click()
            page.wait_for_timeout(700)
            return
    raise PostError("Could not find the write button.")


def _write_content(page: Page, text: str) -> None:
    editor_candidates = [
        '[contenteditable="true"]',
        'textarea',
        'div[role="textbox"]',
    ]

    for selector in editor_candidates:
        locator = page.locator(selector).first
        if locator.count() == 0:
            continue
        try:
            locator.click()
            locator.fill(text)
            return
        except Exception:
            # contenteditable can fail with fill in some editors
            locator.click()
            page.keyboard.type(text)
            return

    raise PostError("Could not find writable editor area.")


def _apply_style(page: Page, cfg: AppConfig) -> None:
    style = cfg.band.style

    if style.bold:
        page.keyboard.press("Control+B")
        sleep(0.1)

    # direct DOM styling fallback when toolbar automation is brittle
    js = """
    ({fontSize, color}) => {
      const editable = document.querySelector('[contenteditable="true"], textarea, div[role="textbox"]');
      if (!editable) return false;

      if (editable.isContentEditable) {
        editable.style.fontSize = `${fontSize}px`;
        editable.style.color = color;
      }
      return true;
    }
    """
    page.evaluate(js, {"fontSize": style.font_size, "color": style.font_color})


def _submit_post(page: Page) -> None:
    submit_candidates = [
        'button:has-text("게시")',
        'button:has-text("등록")',
        'button:has-text("올리기")',
        'button:has-text("Post")',
    ]

    for selector in submit_candidates:
        locator = page.locator(selector).first
        if locator.count() > 0:
            locator.click()
            if _wait_for_post_success(page):
                return
    raise PostError("Failed to submit post.")


def _wait_for_post_success(page: Page) -> bool:
    success_signals = [
        'text=게시되었습니다',
        'text=등록되었습니다',
        'text=Posted',
    ]

    for signal in success_signals:
        try:
            page.wait_for_selector(signal, timeout=4_000)
            return True
        except PlaywrightTimeoutError:
            continue

    # fallback: if editor closes, we assume post succeeded
    if page.locator('[contenteditable="true"]').count() == 0:
        return True

    return False


def capture_failure(page: Page, attempt: int) -> str:
    Path("logs").mkdir(exist_ok=True)
    output = f"logs/fail_attempt_{attempt}.png"
    page.screenshot(path=output, full_page=True)
    return output
