# 개발 문서

onair를 내 PC에서 켜고, 테스트하고, 초기화하는 방법.

## 필요한 도구

| 도구 | 용도 | 확인 |
|---|---|---|
| Rust (msvc + `wasm32-unknown-unknown`) | onair 빌드 | `cargo --version` |
| Dioxus CLI `dx` 0.7.10 | 개발 서버, 웹 번들 | `dx --version` |
| ffmpeg | OBS 없이 테스트 송출 (이후 티켓) | `ffmpeg -version` |
| MediaMTX v1.21.4 | 영상 수신, HLS 변환 | `%LOCALAPPDATA%\onair\mediamtx\mediamtx.exe` |

## 실행

```powershell
.\tools\dev.ps1
```

MediaMTX와 `dx serve`를 함께 켠다. 브라우저에서 http://127.0.0.1:8080 을 연다. Ctrl+C를 누르면 둘 다 꺼진다. 첫 실행은 빌드에 몇 분 걸린다.

- MediaMTX 설정: `tools/mediamtx.yml`. 인증은 아직 `internal`(모두 허용)이다. 티켓 05에서 onair 확인 주소로 바꾼다.
- MediaMTX 위치가 다르면 `ONAIR_MEDIAMTX_EXE` 환경 변수로 지정한다.
- 포트: onair 8080, RTMP 1935, HLS 8888, MediaMTX API 9997(내 PC에서만).
- 처음 켤 때 Windows 방화벽 창이 뜨면 localhost만 쓰므로 취소해도 된다.

## 테스트

```powershell
cargo test -p api --features server
```

테스트는 모두 onair 서비스(`packages/api/src/service.rs`)를 직접 만들어 쓴다. 테스트마다 비어 있는 임시 SQLite 파일과 임시 업로드 폴더를 쓰므로 데이터 폴더와 무관하고, dx와 MediaMTX가 필요 없다. 테스트는 `packages/api/tests/`에 둔다.

## 테스트 채널

SQLite 파일이 비어 있는 채로 서버가 시작하면 테스트 채널 3개가 들어간다. 송출 키는 고정값이다(코드: `SEED_STREAM_KEYS`, `packages/api/src/service.rs`).

| 채널 | 송출 키 |
|---|---|
| 테스트 채널 1 | `test-key-1` |
| 테스트 채널 2 | `test-key-2` |
| 테스트 채널 3 | `test-key-3` |

## 데이터 위치

| 무엇 | 위치 |
|---|---|
| SQLite 파일 | `%LOCALAPPDATA%\onair\data\onair.sqlite` |
| 업로드 파일(프로필 이미지) | `%LOCALAPPDATA%\onair\data\uploads\` |
| 빌드 산출물 | `C:\cargo-target\onair` (`.cargo/config.toml`) |

저장소가 OneDrive 안에 있어서 동기화되지 않도록 모두 밖에 둔다. 데이터 폴더는 `ONAIR_DATA_DIR` 환경 변수로 바꿀 수 있다. 저장소 안의 `data/`, `uploads/`, `*.sqlite`는 git에서 제외된다.

## 초기화

서버를 끄고 데이터 폴더를 지우면 다음 실행에서 깨끗하게 다시 시작한다.

```powershell
Remove-Item -Recurse -Force "$env:LOCALAPPDATA\onair\data"
```

`ONAIR_DATA_DIR`를 쓰고 있다면 그 폴더를 지운다. 스키마가 바뀐 뒤 서버가 시작하지 못할 때도 이 방법을 쓴다.
