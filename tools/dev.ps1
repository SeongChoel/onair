# 로컬 개발 실행: MediaMTX와 onair(dx serve)를 함께 켠다. Ctrl+C로 둘 다 끈다.
#   .\tools\dev.ps1
# 환경 변수(선택):
#   ONAIR_MEDIAMTX_EXE  mediamtx.exe 위치 (기본 %LOCALAPPDATA%\onair\mediamtx\mediamtx.exe)
#   ONAIR_DATA_DIR      SQLite와 업로드 폴더 위치 (기본 %LOCALAPPDATA%\onair\data)
$ErrorActionPreference = 'Stop'

# 새로 설치한 도구가 현재 셸 PATH에 없을 수 있어서 시스템/사용자 PATH를 다시 읽는다.
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')

$root = Split-Path -Parent $PSScriptRoot
$mediamtxExe = if ($env:ONAIR_MEDIAMTX_EXE) { $env:ONAIR_MEDIAMTX_EXE } else { Join-Path $env:LOCALAPPDATA 'onair\mediamtx\mediamtx.exe' }
$mediamtxConfig = Join-Path $PSScriptRoot 'mediamtx.yml'

if (-not (Test-Path $mediamtxExe)) { throw "mediamtx.exe를 찾을 수 없습니다: $mediamtxExe (ONAIR_MEDIAMTX_EXE로 위치를 지정하세요)" }
if (-not (Get-Command dx -ErrorAction SilentlyContinue)) { throw 'dx(Dioxus CLI)를 찾을 수 없습니다. docs/dev.md의 설치 항목을 확인하세요.' }

Write-Host "MediaMTX 시작: $mediamtxExe"
$mediamtx = Start-Process -FilePath $mediamtxExe -ArgumentList "`"$mediamtxConfig`"" -PassThru -NoNewWindow
try {
    Write-Host 'onair 시작: http://127.0.0.1:8080 (dx serve)'
    Push-Location $root
    dx serve -p web --web
}
finally {
    Pop-Location
    if ($mediamtx -and -not $mediamtx.HasExited) {
        Write-Host 'MediaMTX 종료'
        Stop-Process -Id $mediamtx.Id -Force
    }
}
