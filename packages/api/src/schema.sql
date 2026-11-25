-- 서버 시작 시 실행된다. 스키마가 바뀌면 데이터 파일을 지워 초기화한다(docs/dev.md).
CREATE TABLE IF NOT EXISTS channels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    stream_key TEXT NOT NULL UNIQUE,
    profile_image TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS broadcasts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_id INTEGER NOT NULL REFERENCES channels(id),
    title TEXT NOT NULL,
    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ended_at TEXT
);
-- 한 채널에 진행 중인 방송(ended_at이 비어 있는 방송)은 최대 하나.
CREATE UNIQUE INDEX IF NOT EXISTS one_live_broadcast_per_channel
    ON broadcasts(channel_id) WHERE ended_at IS NULL;

CREATE TABLE IF NOT EXISTS chat_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    broadcast_id INTEGER NOT NULL REFERENCES broadcasts(id),
    nickname TEXT NOT NULL,
    content TEXT NOT NULL,
    sent_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS chat_messages_by_broadcast ON chat_messages(broadcast_id, id);
