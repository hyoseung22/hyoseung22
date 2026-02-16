# NaverBand Auto Poster

네이버 밴드에 주기적으로 텍스트 게시물을 등록하는 자동화 프로그램입니다.

## 기능

- 네이버 로그인 자동화 (세션 저장/재사용)
- 게시 주기 + 운영 시간대 설정 (`예: 5분마다, 07:00~19:00`)
- 텍스트 스타일 적용(굵기/크기/색상)
- 백그라운드 실행용 엔트리포인트
- 실패 시 재시도 및 스크린샷/로그 저장

> ⚠️ 본 프로젝트는 학습/사내 자동화 예시입니다. 서비스 약관과 정책을 반드시 준수하세요.

## 빠른 시작

```bash
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
playwright install chromium
```

환경 변수 설정:

```bash
export NAVER_ID="your_id"
export NAVER_PW="your_password"
```

설정 파일 수정:

```bash
cp config.example.yaml config.yaml
# 원하는 주기, 시간, 밴드 URL, 텍스트 수정
```

실행:

```bash
python -m band_auto_poster.main --config config.yaml
```

GUI 실행(초심자용):

```bash
python -m band_auto_poster.main --config config.yaml --gui
```

## 백그라운드 실행 (Linux systemd 예시)

`/etc/systemd/system/naver-band-poster.service`

```ini
[Unit]
Description=NaverBand Auto Poster
After=network.target

[Service]
Type=simple
WorkingDirectory=/opt/naver-band-poster
Environment=NAVER_ID=your_id
Environment=NAVER_PW=your_pw
ExecStart=/opt/naver-band-poster/.venv/bin/python -m band_auto_poster.main --config /opt/naver-band-poster/config.yaml
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

## 구조

- `band_auto_poster/config.py`: 설정 모델 및 로더
- `band_auto_poster/logger.py`: 로깅 설정
- `band_auto_poster/auth.py`: 로그인/세션 관리
- `band_auto_poster/poster.py`: 글쓰기/스타일/게시 로직
- `band_auto_poster/scheduler.py`: 시간대/주기 스케줄링
- `band_auto_poster/main.py`: 엔트리포인트
