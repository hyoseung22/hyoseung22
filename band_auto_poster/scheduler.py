from __future__ import annotations

from datetime import datetime
from zoneinfo import ZoneInfo

from .config import ScheduleConfig


def within_active_window(schedule: ScheduleConfig, now: datetime | None = None) -> bool:
    tz = ZoneInfo(schedule.timezone)
    current = now.astimezone(tz) if now else datetime.now(tz)

    start_h, start_m = _parse_hhmm(schedule.active_time.start)
    end_h, end_m = _parse_hhmm(schedule.active_time.end)

    start_minutes = start_h * 60 + start_m
    end_minutes = end_h * 60 + end_m
    now_minutes = current.hour * 60 + current.minute

    if start_minutes <= end_minutes:
        return start_minutes <= now_minutes <= end_minutes

    # overnight window, e.g. 22:00 ~ 04:00
    return now_minutes >= start_minutes or now_minutes <= end_minutes


def _parse_hhmm(value: str) -> tuple[int, int]:
    hour, minute = value.split(":")
    h = int(hour)
    m = int(minute)
    if h < 0 or h > 23 or m < 0 or m > 59:
        raise ValueError(f"Invalid HH:MM value: {value}")
    return h, m
