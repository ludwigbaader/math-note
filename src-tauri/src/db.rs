//! SQLite storage for notes. One table, one connection behind a mutex.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, Row};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoteMeta {
    pub id: i64,
    pub title: String,
    /// The first couple of hundred characters, used for a fallback title in lists.
    pub preview: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct Db {
    conn: Mutex<Connection>,
}

/// ISO-8601 UTC timestamp with milliseconds, evaluated inside SQLite.
const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
/// Inserted the first time the database is created, so a new user sees the syntax in action.
const WELCOME_NOTE: &str = "\
Welcome to Math Note

End a line with = to calculate:
2 + 2 =
12 * 3.5 =

Name a result by writing = name after it, then reuse it anywhere:
3 * 4 = area
area / 2 =

Or define variables directly:
r = 5
2 pi r =

Words before a formula are ignored:
The total is 3 * 4 + 1 =

Check your own answers:
7 * 8 = 56
7 * 8 = 54

Functions, powers and more:
sqrt(16) + 2^3 =
5! =
50% =
10 mod 3 =
";

const META_COLUMNS: &str = "id, title, substr(content, 1, 200), updated_at";

impl Db {
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        let fresh = !table_exists(&conn, "notes")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS notes (
                 id         INTEGER PRIMARY KEY AUTOINCREMENT,
                 title      TEXT NOT NULL DEFAULT '',
                 content    TEXT NOT NULL DEFAULT '',
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL
             );",
        )?;
        if fresh {
            conn.execute(
                &format!("INSERT INTO notes (title, content, created_at, updated_at) VALUES ('Welcome', ?1, {NOW}, {NOW})"),
                params![WELCOME_NOTE],
            )?;
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn list(&self) -> rusqlite::Result<Vec<NoteMeta>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {META_COLUMNS} FROM notes ORDER BY updated_at DESC, id DESC"
        ))?;
        let rows = stmt.query_map([], meta_from_row)?;
        rows.collect()
    }

    pub fn create(&self) -> rusqlite::Result<Note> {
        let conn = self.conn();
        conn.execute(
            &format!("INSERT INTO notes (title, content, created_at, updated_at) VALUES ('', '', {NOW}, {NOW})"),
            [],
        )?;
        let id = conn.last_insert_rowid();
        fetch(&conn, id)
    }

    pub fn get(&self, id: i64) -> rusqlite::Result<Note> {
        fetch(&self.conn(), id)
    }

    pub fn save(&self, id: i64, title: &str, content: &str) -> rusqlite::Result<NoteMeta> {
        let conn = self.conn();
        let changed = conn.execute(
            &format!("UPDATE notes SET title = ?1, content = ?2, updated_at = {NOW} WHERE id = ?3"),
            params![title, content, id],
        )?;
        if changed == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        conn.query_row(
            &format!("SELECT {META_COLUMNS} FROM notes WHERE id = ?1"),
            params![id],
            meta_from_row,
        )
    }

    pub fn delete(&self, id: i64) -> rusqlite::Result<()> {
        self.conn()
            .execute("DELETE FROM notes WHERE id = ?1", params![id])?;
        Ok(())
    }
}

fn table_exists(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![name],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
}

fn fetch(conn: &Connection, id: i64) -> rusqlite::Result<Note> {
    conn.query_row(
        "SELECT id, title, content, created_at, updated_at FROM notes WHERE id = ?1",
        params![id],
        |row| {
            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        },
    )
}

fn meta_from_row(row: &Row<'_>) -> rusqlite::Result<NoteMeta> {
    Ok(NoteMeta {
        id: row.get(0)?,
        title: row.get(1)?,
        preview: row.get(2)?,
        updated_at: row.get(3)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let db = Db::open(":memory:").unwrap();
        let initial = db.list().unwrap();
        assert_eq!(initial.len(), 1);
        assert_eq!(initial[0].title, "Welcome");

        let note = db.create().unwrap();
        assert_eq!(note.title, "");

        let meta = db
            .save(note.id, "Budget", "rent = 900\nrent * 12 =")
            .unwrap();
        assert_eq!(meta.title, "Budget");
        assert!(meta.preview.starts_with("rent = 900"));

        let fetched = db.get(note.id).unwrap();
        assert_eq!(fetched.content, "rent = 900\nrent * 12 =");
        assert_eq!(db.list().unwrap().len(), 2);

        db.delete(note.id).unwrap();
        assert_eq!(db.list().unwrap().len(), 1);
        assert!(db.get(note.id).is_err());
        assert!(db.save(note.id, "x", "y").is_err());
    }
}
