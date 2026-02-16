from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

import yaml


@dataclass(slots=True)
class NaverConfig:
    username_env: str
    password_env: str
    use_saved_session: bool
    session_file: str


@dataclass(slots=True)
class StyleConfig:
    bold: bool
    font_size: int
    font_color: str


@dataclass(slots=True)
class BandConfig:
    target_url: str
    post_text: str
    style: StyleConfig


@dataclass(slots=True)
class ActiveTimeConfig:
    start: str
    end: str


@dataclass(slots=True)
class ScheduleConfig:
    interval_minutes: int
    timezone: str
    active_time: ActiveTimeConfig


@dataclass(slots=True)
class RetryConfig:
    max_attempts: int
    backoff_seconds: int


@dataclass(slots=True)
class LoggingConfig:
    level: str
    file: str


@dataclass(slots=True)
class AppConfig:
    naver: NaverConfig
    band: BandConfig
    schedule: ScheduleConfig
    retry: RetryConfig
    logging: LoggingConfig


class ConfigError(ValueError):
    """Raised when configuration values are invalid."""


def _require(mapping: dict[str, Any], key: str) -> Any:
    if key not in mapping:
        raise ConfigError(f"Missing config key: {key}")
    return mapping[key]


def load_config(path: str | Path) -> AppConfig:
    data = yaml.safe_load(Path(path).read_text(encoding="utf-8"))

    naver = _require(data, "naver")
    band = _require(data, "band")
    style = _require(band, "style")
    schedule = _require(data, "schedule")
    active_time = _require(schedule, "active_time")
    retry = _require(data, "retry")
    logging_cfg = _require(data, "logging")

    interval = int(_require(schedule, "interval_minutes"))
    if interval < 1:
        raise ConfigError("schedule.interval_minutes must be >= 1")

    return AppConfig(
        naver=NaverConfig(
            username_env=str(_require(naver, "username_env")),
            password_env=str(_require(naver, "password_env")),
            use_saved_session=bool(_require(naver, "use_saved_session")),
            session_file=str(_require(naver, "session_file")),
        ),
        band=BandConfig(
            target_url=str(_require(band, "target_url")),
            post_text=str(_require(band, "post_text")),
            style=StyleConfig(
                bold=bool(_require(style, "bold")),
                font_size=int(_require(style, "font_size")),
                font_color=str(_require(style, "font_color")),
            ),
        ),
        schedule=ScheduleConfig(
            interval_minutes=interval,
            timezone=str(_require(schedule, "timezone")),
            active_time=ActiveTimeConfig(
                start=str(_require(active_time, "start")),
                end=str(_require(active_time, "end")),
            ),
        ),
        retry=RetryConfig(
            max_attempts=int(_require(retry, "max_attempts")),
            backoff_seconds=int(_require(retry, "backoff_seconds")),
        ),
        logging=LoggingConfig(
            level=str(_require(logging_cfg, "level")),
            file=str(_require(logging_cfg, "file")),
        ),
    )
