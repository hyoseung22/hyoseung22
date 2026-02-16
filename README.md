# NaverBand Auto Poster

네이버 밴드 자동 게시 프로그램입니다.

## 요청 반영 사항

- 변수 설정(주기, 시간, 텍스트, 스타일, 계정 입력)
- 시작 / 중지 / 1회 실행 / 점검 실행

위 모든 작업을 **GUI 화면에서** 처리하도록 변경했습니다.

## 실행 방법 (응용프로그램 방식)

### Linux

- `launch_app.sh` 실행
- 또는 `NaverBandAutoPoster.desktop` 더블클릭 실행

### 공통 파이썬 실행

```bash
python run_gui_app.py
```

### Windows

- `RUN_GUI.bat` 더블클릭 실행
- 또는 `python run_gui_app.py` 실행

`run_gui_app`은 필수 패키지(yaml/apscheduler/playwright)가 없으면 `requirements.txt`로 자동 설치를 시도합니다.

`run_gui_app` 실행 직후 창이 닫히면 `logs/run_gui_error.log`를 확인하세요.

### 점검 스크립트 실행

```bash
./demo_run.sh
```

실행 결과는 `logs/demo_run_output.txt`에 저장됩니다.

실행 시 `config.yaml`이 없으면 `config.example.yaml`을 복사해 자동 생성합니다.

## GUI에서 하는 작업

- 밴드 URL
- 게시 내용
- 게시 주기 / 시작 시간 / 종료 시간 / 타임존
- 폰트(굵기/크기/색상)
- 재시도 횟수 / 재시도 간격
- 네이버 ID / PW

입력 후 `설정 저장` → `자동 시작` 순서로 사용하면 됩니다.

## 파일 위치

- GUI 메인: `band_auto_poster/gui.py`
- 앱 런처(Python): `run_gui_app.py`
- 앱 런처(Shell): `launch_app.sh`
- Desktop 파일: `NaverBandAutoPoster.desktop`
- 실행 엔진: `band_auto_poster/main.py`
- 설정 로더/저장: `band_auto_poster/config.py`
- 설정 샘플: `config.example.yaml`

## 참고

- 서비스 정책/약관을 준수해 주세요.
- 사이트 DOM 변경 시 셀렉터 수정이 필요할 수 있습니다.
