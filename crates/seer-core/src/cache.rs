//! Persistent cache of analysis results, keyed by path and file fingerprint.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;
use std::sync::Mutex;

/// Bump when a cached type changes shape; old rows are then ignored.
const SCHEMA: u32 = 1;

pub struct Cache {
    db: Mutex<Connection>,
}

impl Cache {
    pub fn open(file: &Path) -> Result<Self> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::with(Connection::open(file)?)
    }

    pub fn in_memory() -> Result<Self> {
        Self::with(Connection::open_in_memory()?)
    }

    fn with(db: Connection) -> Result<Self> {
        db.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS cache (
                 path        TEXT NOT NULL,
                 kind        TEXT NOT NULL,
                 fingerprint TEXT NOT NULL,
                 schema      INTEGER NOT NULL,
                 data        BLOB NOT NULL,
                 PRIMARY KEY (path, kind)
             ) WITHOUT ROWID;",
        )?;
        Ok(Self { db: Mutex::new(db) })
    }

    /// Returns the stored value if it was computed for this exact file state.
    pub fn get<T: DeserializeOwned>(
        &self,
        path: &Path,
        kind: &str,
        fingerprint: &str,
    ) -> Option<T> {
        let db = self.db.lock().ok()?;
        let data: Vec<u8> = db
            .query_row(
                "SELECT data FROM cache
                 WHERE path = ?1 AND kind = ?2 AND fingerprint = ?3 AND schema = ?4",
                params![path.to_string_lossy(), kind, fingerprint, SCHEMA],
                |row| row.get(0),
            )
            .optional()
            .ok()??;
        serde_json::from_slice(&data).ok()
    }

    pub fn put<T: Serialize>(
        &self,
        path: &Path,
        kind: &str,
        fingerprint: &str,
        value: &T,
    ) -> Result<()> {
        let data = serde_json::to_vec(value)?;
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("cache lock poisoned"))?;
        db.execute(
            "INSERT OR REPLACE INTO cache (path, kind, fingerprint, schema, data)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![path.to_string_lossy(), kind, fingerprint, SCHEMA, data],
        )?;
        Ok(())
    }

    pub fn clear(&self) -> Result<usize> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("cache lock poisoned"))?;
        Ok(db.execute("DELETE FROM cache", [])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_invalidation() {
        let cache = Cache::in_memory().unwrap();
        let path = Path::new("/a/b.mkv");
        assert_eq!(cache.get::<Vec<u32>>(path, "probe", "f1"), None);
        cache.put(path, "probe", "f1", &vec![1u32, 2]).unwrap();
        assert_eq!(cache.get::<Vec<u32>>(path, "probe", "f1"), Some(vec![1, 2]));
        // A different fingerprint means the file changed.
        assert_eq!(cache.get::<Vec<u32>>(path, "probe", "f2"), None);
        // Kinds are independent.
        assert_eq!(cache.get::<Vec<u32>>(path, "bitrate", "f1"), None);
        assert_eq!(cache.clear().unwrap(), 1);
    }
}
