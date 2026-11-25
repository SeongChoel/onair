-- 서버 시작 시 실행된다. 스키마가 바뀌면 데이터 파일을 지워 초기화한다(docs/dev.md).
CREATE TABLE IF NOT EXISTS channels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);
