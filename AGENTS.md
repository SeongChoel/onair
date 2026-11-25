# AGENTS.md

## 작업 흐름
- 큰 작업은 `/grill-with-docs` → `/to-spec` → `/to-tickets` → `/implement-spec` 순서로 진행한다.
- 시작 전 요구사항이 모호하면 추측하지 말고 먼저 묻는다.

## 코드
- 변경은 작게, 요청 범위 안에서만 한다.
- 기존 코드 스타일과 구조를 따른다.
- 가능하면 테스트를 먼저 쓰고(`/tdd`), 완료 전 테스트를 돌려 결과를 보고한다.

## Git
- 커밋·푸시는 사용자가 요청할 때만 한다.
- 커밋 메시지는 무엇을 왜 바꿨는지 한 줄로 쓴다.
- 비밀값(키, 토큰, 비밀번호)은 커밋하지 않는다.

## 소통
- 답변과 문서는 한국어로 쓴다.
- 실패나 건너뛴 단계는 숨기지 않고 그대로 보고한다.

## Agent skills

### Issue tracker

Local markdown files under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five roles (needs-triage, needs-info, ready-for-agent, ready-for-human, wontfix). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `GLOSSARY.md` + `docs/adr/`. See `docs/agents/domain.md`.
