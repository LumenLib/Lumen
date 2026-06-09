use super::Migration;
use super::utils;

pub fn migration() -> Migration {
    Migration {
        version: "v013",
        description: "AI 对话记录表",
        up: |conn| {
            if utils::table_exists(conn, "chat_sessions")? {
                return Ok(());
            }

            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS chat_sessions (
                    id              TEXT PRIMARY KEY,
                    literature_id   TEXT NOT NULL,
                    title           TEXT NOT NULL DEFAULT '',
                    system_prompt   TEXT NOT NULL DEFAULT '',
                    created_at      INTEGER NOT NULL,
                    updated_at      INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_chat_sessions_lit_id
                    ON chat_sessions(literature_id, updated_at DESC);

                CREATE TABLE IF NOT EXISTS chat_messages (
                    id              TEXT PRIMARY KEY,
                    session_id      TEXT NOT NULL,
                    role            TEXT NOT NULL,
                    content         TEXT NOT NULL DEFAULT '',
                    attachments     TEXT NOT NULL DEFAULT '[]',
                    created_at      INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_chat_messages_session
                    ON chat_messages(session_id, created_at ASC);",
            )?;

            Ok(())
        },
    }
}
