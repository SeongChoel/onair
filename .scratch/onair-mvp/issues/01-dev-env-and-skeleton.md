# 01: 개발 환경과 빈 앱 뼈대

**What to build:** 개발자가 명령 하나로 onair와 MediaMTX를 함께 켜고, 브라우저에서 onair 첫 화면을 볼 수 있다. 이후 모든 티켓이 쓸 onair 서비스 테스트 틀(임시 SQLite)이 준비된다. 명세: `.scratch/onair-mvp/spec.md`

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 설치 전에 설치할 도구(rustup, Dioxus CLI, ffmpeg, MediaMTX)와 각 용도·설치 방법을 표로 사용자에게 보여 주고, 설치 진행 상황을 화면에 표시한다
- [ ] 설치 후 각 도구의 버전이 확인된다
- [ ] Dioxus 공식 워크스페이스 템플릿 구조에서 웹만 남긴 워크스페이스(web, ui, api)가 있다
- [ ] 로컬 실행 스크립트 하나로 onair와 MediaMTX가 함께 켜지고, 브라우저에 "onair" 첫 화면이 뜬다
- [ ] MediaMTX 설정 파일이 워크스페이스 도구 폴더에 있다
- [ ] onair 서비스를 임시 SQLite와 임시 업로드 폴더로 만드는 테스트 틀이 있고, 첫 테스트 하나가 통과한다
- [ ] 자동 테스트를 명령 하나로 돌릴 수 있고, 그 방법이 개발 문서에 적혀 있다
- [ ] SQLite 파일과 업로드 폴더가 git에서 제외된다
