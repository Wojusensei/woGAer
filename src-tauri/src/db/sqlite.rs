use rusqlite::{Connection, Result, params};
use std::path::PathBuf;
use std::sync::Mutex;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BuildRecord {
    pub id: i64,
    pub repo_name: String,
    pub workflow_id: String,
    pub status: String,
    pub created_at: String,
    pub artifact_url: Option<String>,
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(app_data_dir: PathBuf) -> Result<Self> {
        let db_path = app_data_dir.join("wogaer.db");
        let conn = Connection::open(db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS build_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                repo_name TEXT NOT NULL,
                workflow_id TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                artifact_url TEXT
            )",
            [],
        )?;
        Ok(Database {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert_record(&self, repo_name: &str, workflow_id: &str, status: &str, created_at: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO build_records (repo_name, workflow_id, status, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![repo_name, workflow_id, status, created_at],
        )?;
        Ok(())
    }

    pub fn update_record_status(&self, id: i64, status: &str, artifact_url: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE build_records SET status = ?1, artifact_url = ?2 WHERE id = ?3",
            params![status, artifact_url, id],
        )?;
        Ok(())
    }

    pub fn get_all_records(&self) -> Result<Vec<BuildRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, repo_name, workflow_id, status, created_at, artifact_url FROM build_records ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(BuildRecord {
                id: row.get(0)?,
                repo_name: row.get(1)?,
                workflow_id: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get(4)?,
                artifact_url: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    }
}