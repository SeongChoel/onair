# 개발 문서

onair를 내 PC에서 켜고, 테스트하고, 초기화하는 방법.

## 필요한 도구

| 도구 | 용도 | 확인 |
|---|---|---|
| Rust (msvc + `wasm32-unknown-unknown`) | onair 빌드 | `cargo --version` |
| Dioxus CLI `dx` 0.7.10 | 개발 서버, 웹 번들 | `dx --version` |
| ffmpeg | OBS 없이 테스트 송출 (`tools/fake-stream.ps1`) | `ffmpeg -version` |
| MediaMTX v1.21.4 | 영상 수신, HLS 변환 | `%LOCALAPPDATA%\onair\mediamtx\mediamtx.exe` |

## 실행

```powershell
.\tools\dev.ps1
```

MediaMTX와 `dx serve`를 함께 켠다. 브라우저에서 http://127.0.0.1:8080 을 연다. Ctrl+C를 누르면 둘 다 꺼진다. 첫 실행은 빌드에 몇 분 걸린다.

- MediaMTX 설정: `tools/mediamtx.yml`. 인증은 onair의 확인 주소(`http://127.0.0.1:8081/internal/mediamtx/auth`)에 묻는다. 이 주소는 앱의 공개 포트(8080)와 따로 127.0.0.1에만 열려서 바깥에서는 닿지 않는다. 포트는 `ONAIR_AUTH_PORT`(기본 8081)로 바꾸고, 바꾸면 `tools/mediamtx.yml`의 `authHTTPAddress`도 맞춘다.
- MediaMTX API 주소를 바꿨다면 `ONAIR_MEDIAMTX_API`(기본 `http://127.0.0.1:9997`)로 알려 준다. 방송 종료 때 송출을 끊는 데 쓴다.
- 채널 페이지 플레이어는 hls.js를 CDN(jsdelivr)에서 불러오므로 인터넷이 필요하다.
- MediaMTX 위치가 다르면 `ONAIR_MEDIAMTX_EXE` 환경 변수로 지정한다.
- 포트: onair 8080, 인증 훅 8081(내 PC에서만), RTMP 1935, HLS 8888, MediaMTX API 9997(내 PC에서만).
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

## OBS 설정

스튜디오(http://127.0.0.1:8080/studio)의 채널 관리 화면에 OBS에 넣을 값이 나온다. MediaMTX 송출 경로는 `live/<채널 아이디>`이고 송출 키는 RTMP 쿼리로 간다.

| OBS 칸 | 값 |
|---|---|
| 서버 | `rtmp://localhost:1935/live` |
| 스트림 키 | `<채널 아이디>?key=<송출 키>` 문자열 전체 (예: `1?key=test-key-1`) |

OBS의 스트림 키 칸에 `?key=...`까지 통째로 넣는다. 송출 키는 비밀이므로 스트림 키 칸 값도 남에게 보이지 않는다. 송출 키는 `onair_`로 시작하는 무작위 문자열이고 브라우저의 localStorage(`onair.streamKey`)에 저장된다. 나가기를 누르면 지워진다.

송출은 그 채널에 진행 중인 방송이 있을 때만 허락된다. 스튜디오에서 방송 시작을 먼저 누르고 송출한다. 방송 시작 전, 종료 후, 틀린 키는 거부되며, 방송을 종료하면 이미 연결된 송출도 끊긴다.

영상 신호(MediaMTX 경로가 송출을 받는 중인지)는 시청 화면과 스튜디오가 2초마다 `/api/channels/<id>/status`로 물어 따라간다. 신호가 끊겨도 방송은 자동으로 종료되지 않는다.

## 테스트 송출 (OBS 없이)

방송 시작을 누른 뒤 실행한다. ffmpeg가 테스트 화면과 소리를 영상 파일 없이 계속 송출하고, Ctrl+C로 멈춘다.

```powershell
.\tools\fake-stream.ps1 -ChannelId 1                       # 시드 채널(1~3)은 송출 키(test-key-N)를 자동으로 쓴다
.\tools\fake-stream.ps1 -ChannelId 5 -StreamKey onair_xxx  # 직접 만든 채널은 송출 키를 준다
```

채널 페이지(`/channel/<아이디>`)에서 재생된다. 시청 주소는 `http://localhost:8888/live/<채널 아이디>/index.m3u8`이고 송출 키는 나오지 않는다.

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
