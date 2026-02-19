# Game Agent Orchestrator V1 (Local Runnable)

게임 제작용 멀티 에이전트 오케스트레이션의 **실행 가능한 V1**입니다.

## What works now
- SQLite 기반 데이터 저장 (`docs/schema.sql` 자동 적용)
- REST API: agents/tasks/messages/audit/health
- 기본 권한/감사 로그 기록(간단 버전)
- 브라우저 GUI에서 에이전트/태스크/메시지 생성 및 상태 조회

## Run
```bash
npm install
npm start
```

Open: `http://127.0.0.1:8790`

## Key files
- `apps/api/src/index.js`: Express + SQLite API server
- `apps/web/index.html`: 간단 GUI
- `apps/web/app.js`: GUI 동작 로직
- `docs/schema.sql`: DB schema
- `docs/openapi.yaml`: API 초안 명세

## Notes
- 현재는 로컬 개발용 최소 기능입니다.
- 다음 단계: RBAC 강화, 리뷰 게이트 강제 정책 고도화, 테스트 자동화.
