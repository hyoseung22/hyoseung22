from pathlib import Path

from band_auto_poster.config import (
    ActiveTimeConfig,
    AppConfig,
    BandConfig,
    LoggingConfig,
    NaverConfig,
    RetryConfig,
    ScheduleConfig,
    StyleConfig,
    load_config,
    save_config,
)


def test_config_save_and_load_roundtrip(tmp_path: Path) -> None:
    cfg = AppConfig(
        naver=NaverConfig("NAVER_ID", "NAVER_PW", True, "./data/session.json"),
        band=BandConfig("https://band.us/band/demo", "hello", StyleConfig(True, 15, "#112233")),
        schedule=ScheduleConfig(5, "Asia/Seoul", ActiveTimeConfig("07:00", "19:00")),
        retry=RetryConfig(3, 8),
        logging=LoggingConfig("INFO", "./logs/app.log"),
    )

    target = tmp_path / "config.yaml"
    save_config(target, cfg)
    loaded = load_config(target)

    assert loaded.band.target_url == cfg.band.target_url
    assert loaded.band.style.font_color == "#112233"
    assert loaded.schedule.interval_minutes == 5
