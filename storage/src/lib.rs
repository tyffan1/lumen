use anyhow::Result;
use chrono::{DateTime, Local, TimeZone, Utc};
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

    /// Кладёт сессию в буфер (автоматически разбивает через локальную полночь).
    /// Не трогает диск.
    pub fn queue(&mut self, session: Session) {
        self.pending.extend(split_session(&session));
    }

    /// То же, что queue(), но без разбивки по дням (для прямого импорта).
    pub fn queue_raw(&mut self, session: Session) {
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

    /// Очищает все данные из таблицы sessions.
    pub fn clear(&mut self) -> Result<()> {
        self.pending.clear();
        self.conn.execute_batch("DELETE FROM sessions")?;
        self.conn.execute_batch("VACUUM")?;
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

    /// Агрегат "сколько секунд на каждое приложение" с начала сегодняшнего дня (локальное время).
    /// Использует datetime(..., 'localtime') для корректного часового пояса.
    /// Возвращает суммарное время (секунды) по дням за последние `days` дней.
    /// Возвращает Vec<(дата "YYYY-MM-DD", total_seconds)> только для дней, в которых есть данные.
    pub fn usage_by_day(&self, days: u32) -> Result<Vec<(String, i64)>> {
        let since = Utc::now() - chrono::Duration::days(days as i64);
        let mut stmt = self.conn.prepare(
            "SELECT DATE(datetime(started_at, 'unixepoch', 'localtime')) as day,
                    SUM(ended_at - started_at) as total
             FROM sessions
             WHERE started_at >= ?1
             GROUP BY day
             ORDER BY day ASC",
        )?;
        let rows = stmt
            .query_map([since.timestamp()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

/// Разбивает сессию на фрагменты по границам локальной полуночи.
/// Каждый фрагмент целиком лежит в одном локальном дне.
fn split_session(session: &Session) -> Vec<Session> {
    let mut result = Vec::new();
    let mut cur = session.started_at;
    let end = session.ended_at;

    if cur >= end {
        return result;
    }

    loop {
        let local = cur.with_timezone(&Local);
        let date = local.date_naive();
        let next_local = date.succ_opt().expect("date overflow").and_hms_opt(0, 0, 0).expect("invalid midnight");
        let next_midnight_local = Local
            .from_local_datetime(&next_local)
            .single()
            .expect("local midnight should not be ambiguous");
        let next = next_midnight_local.with_timezone(&Utc);

        if next >= end {
            result.push(Session {
                exe_name: session.exe_name.clone(),
                window_title: session.window_title.clone(),
                started_at: cur,
                ended_at: end,
                was_fullscreen: session.was_fullscreen,
            });
            break;
        }

        result.push(Session {
            exe_name: session.exe_name.clone(),
            window_title: session.window_title.clone(),
            started_at: cur,
            ended_at: next,
            was_fullscreen: session.was_fullscreen,
        });
        cur = next;
    }

    result
}
