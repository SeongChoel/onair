# 영상 수신과 변환은 MediaMTX에 맡긴다

OBS가 보내는 RTMP 영상을 받아 브라우저가 재생할 수 있는 HLS로 바꾸는 일은 Rust로 직접 만들지 않고, 오픈소스(MIT) 미디어 서버인 MediaMTX에 맡긴다. 직접 구현하려면 RTMP 서버, FLV 분해, HLS 조각내기, 재생 목록 관리까지 만들어야 해서 "작게 시작"하려는 목표와 맞지 않는다. onair는 MediaMTX가 물어볼 때 송출 키와 진행 중인 방송을 확인해 주는 일만 하고, 영상 데이터는 onair를 거치지 않는다.

## Consequences

- 로컬 실행에 프로그램이 둘(onair, MediaMTX) 필요하므로 `tools/dev.ps1`이 함께 켠다.
- onair는 "영상은 HLS 주소에서 나온다"는 것만 알기 때문에, 나중에 MediaMTX를 다른 서버나 직접 구현으로 바꿔도 나머지는 그대로다.
- 브라우저 직접 송출(WebRTC/WHIP)도 MediaMTX가 지원하므로 추가할 때 시청 쪽은 바꾸지 않아도 된다.
