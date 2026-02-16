from band_auto_poster.config import (
    ActiveTimeConfig,
    AppConfig,
    BandConfig,
    LoggingConfig,
    NaverConfig,
    RetryConfig,
    ScheduleConfig,
    StyleConfig,
)
from band_auto_poster.main import run_dry


def _build_config() -> AppConfig:
    return AppConfig(
        naver=NaverConfig(
            username_env="NAVER_ID",
            password_env="NAVER_PW",
            use_saved_session=True,
            session_file="./data/session.json",
        ),
        band=BandConfig(
            target_url="https://band.us/band/demo",
            post_text="테스트",
            style=StyleConfig(bold=True, font_size=16, font_color="#111111"),
        ),
        schedule=ScheduleConfig(
            interval_minutes=5,
            timezone="Asia/Seoul",
            active_time=ActiveTimeConfig(start="07:00", end="19:00"),
        ),
        retry=RetryConfig(max_attempts=3, backoff_seconds=1),
        logging=LoggingConfig(level="INFO", file="./logs/app.log"),
    )


def test_run_dry_returns_human_readable_report() -> None:
    cfg = _build_config()
    report = run_dry(cfg)

    assert any("[DRY-RUN]" in line for line in report)
    assert any("대상 밴드" in line for line in report)
    assert any("실제 로그인/게시 없이" in line for line in report)
