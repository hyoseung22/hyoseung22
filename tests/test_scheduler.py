from datetime import datetime
from zoneinfo import ZoneInfo

from band_auto_poster.config import ActiveTimeConfig, ScheduleConfig
from band_auto_poster.scheduler import within_active_window


def test_within_active_window_daytime() -> None:
    schedule = ScheduleConfig(
        interval_minutes=5,
        timezone="Asia/Seoul",
        active_time=ActiveTimeConfig(start="07:00", end="19:00"),
    )
    dt = datetime(2025, 1, 1, 10, 0, tzinfo=ZoneInfo("Asia/Seoul"))
    assert within_active_window(schedule, dt)


def test_within_active_window_overnight() -> None:
    schedule = ScheduleConfig(
        interval_minutes=5,
        timezone="Asia/Seoul",
        active_time=ActiveTimeConfig(start="22:00", end="04:00"),
    )
    dt = datetime(2025, 1, 1, 23, 30, tzinfo=ZoneInfo("Asia/Seoul"))
    assert within_active_window(schedule, dt)
