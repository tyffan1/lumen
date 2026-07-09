use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use std::path::Path;

pub struct Storage {
    conn: Connection,
    pending: Vec<Session>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub exe_name: String,
    pub window_title: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub was_fullscreen: bool,
}

impl Storage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                exe_name TEXT NOT NULL,
                window_title TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                ended_at INTEGER NOT NULL,
                was_fullscreen INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_sessions_started
                ON sessions(started_at);
            CREATE INDEX IF NOT EXISTS idx_sessions_exe
                ON sessions(exe_name);",
        )?;

        Ok(Self { conn, pending: Vec::new() })
    }

    /// Кладёт сессию в буфер. Не трогает диск.
    pub fn queue(&mut self, session: Session) {
        self.pending.push(session);
    }

    /// Пишет накопленный буфер одной транзакцией.
    /// Вызывать по таймеру (например, раз в 30-60 сек) и на shutdown.
    pub fn flush(&mut self) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }

        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO sessions (exe_name, window_title, started_at, ended_at, was_fullscreen)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for s in &self.pending {
                stmt.execute(rusqlite::params![
                    s.exe_name,
                    s.window_title,
                    s.started_at.timestamp(),
                    s.ended_at.timestamp(),
                    s.was_fullscreen as i32,
                ])?;
            }
        }
        tx.commit()?;

        self.pending.clear();
        Ok(())
    }

    /// Агрегат "сколько секунд на каждое приложение" за период.
    pub fn totals_by_app(&self, since: DateTime<Utc>) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT exe_name, SUM(ended_at - started_at) as total
             FROM sessions
             WHERE started_at >= ?1
             GROUP BY exe_name
             ORDER BY total DESC",
        )?;
        let rows = stmt
            .query_map([since.timestamp()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}
