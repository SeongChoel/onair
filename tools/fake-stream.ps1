# OBS 없이 테스트 송출: ffmpeg 테스트 화면 + 소리를 채널에 계속 송출한다. 영상 파일이 필요 없다. Ctrl+C로 멈춘다.
#   .\tools\fake-stream.ps1 -ChannelId 1                      # 시드 채널이면 송출 키(test-key-N)를 자동으로 쓴다
#   .\tools\fake-stream.ps1 -ChannelId 5 -StreamKey onair_xxx # 직접 만든 채널은 송출 키를 준다
# 먼저 스튜디오에서 방송 시작을 눌러야 한다. 방송 중이 아니거나 키가 틀리면 MediaMTX가 거부해서 ffmpeg가 바로 끝난다.
param(
    [Parameter(Mandatory = $true)][int]$ChannelId,
    [string]$StreamKey,
    [string]$Server = 'rtmp://127.0.0.1:1935/live'
)
$ErrorActionPreference = 'Stop'

# 새로 설치한 도구가 현재 셸 PATH에 없을 수 있어서 시스템/사용자 PATH를 다시 읽는다.
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not (Get-Command ffmpeg -ErrorAction SilentlyContinue)) { throw 'ffmpeg를 찾을 수 없습니다. docs/dev.md의 필요한 도구를 확인하세요.' }

if (-not $StreamKey) {
    if ($ChannelId -ge 1 -and $ChannelId -le 3) { $StreamKey = "test-key-$ChannelId" }
    else { throw '시드 채널(1~3)이 아니면 -StreamKey로 송출 키를 주세요.' }
}

# 송출 키는 RTMP 쿼리(?key=)로 간다. 화면에 찍지 않는다.
$url = "$Server/${ChannelId}?key=$StreamKey"
Write-Host "채널 $ChannelId 에 테스트 송출 중 (Ctrl+C로 멈춤)"
# ffmpeg는 거부 같은 오류를 stderr로 내므로, 여기서는 PowerShell 오류로 바꾸지 않는다.
$ErrorActionPreference = 'Continue'
# -g 30: 1초마다 키프레임 (HLS 조각 길이에 맞춤)
ffmpeg -hide_banner -loglevel warning -re `
    -f lavfi -i 'testsrc2=size=1280x720:rate=30' `
    -f lavfi -i 'sine=frequency=440:sample_rate=48000' `
    -c:v libx264 -preset veryfast -tune zerolatency -pix_fmt yuv420p -g 30 `
    -c:a aac -b:a 128k -f flv $url
Write-Host "송출이 끝났습니다 (종료 코드 $LASTEXITCODE). 방송이 시작됐는지, 키가 맞는지 확인하세요."
