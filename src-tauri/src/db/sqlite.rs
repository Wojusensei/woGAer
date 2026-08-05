use rusqlite::{Connection, Result, params};
use std::path::PathBuf;
use std::sync::Mutex;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BuildRecord {
    pub id: i64,
    pub run_id: i64,
    pub repo_name: String,
    pub workflow_id: String,
    pub status: String,
    pub created_at: String,
    pub artifact_url: Option<String>,
    pub trigger_type: String,
    pub platform: String,
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
                artifact_url TEXT,
                trigger_type TEXT NOT NULL,
                platform TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_repo_name ON build_records(repo_name)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_created_at ON build_records(created_at DESC)",
            [],
        )?;

        let has_run_id = {
            let mut stmt = conn.prepare("PRAGMA table_info(build_records)")?;
            let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
            let mut found = false;
            for name in columns {
                if name? == "run_id" {
                    found = true;
                    break;
                }
            }
            found
        };
        if !has_run_id {
            conn.execute("ALTER TABLE build_records ADD COLUMN run_id INTEGER DEFAULT 0", [])?;
        }

        Ok(Database {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert_record(&self, record: &BuildRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO build_records 
             (run_id, repo_name, workflow_id, status, created_at, artifact_url, trigger_type, platform) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                record.run_id,
                record.repo_name,
                record.workflow_id,
                record.status,
                record.created_at,
                record.artifact_url,
                record.trigger_type,
                record.platform
            ],
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

    pub fn get_record_by_id(&self, id: i64) -> Result<Option<BuildRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, run_id, repo_name, workflow_id, status, created_at, artifact_url, trigger_type, platform 
             FROM build_records WHERE id = ?1"
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(BuildRecord {
                id: row.get(0)?,
                run_id: row.get(1)?,
                repo_name: row.get(2)?,
                workflow_id: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get(5)?,
                artifact_url: row.get(6)?,
                trigger_type: row.get(7)?,
                platform: row.get(8)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_all_records(&self, limit: usize) -> Result<Vec<BuildRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, run_id, repo_name, workflow_id, status, created_at, artifact_url, trigger_type, platform 
             FROM build_records ORDER BY created_at DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(BuildRecord {
                id: row.get(0)?,
                run_id: row.get(1)?,
                repo_name: row.get(2)?,
                workflow_id: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get(5)?,
                artifact_url: row.get(6)?,
                trigger_type: row.get(7)?,
                platform: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    }

    pub fn get_records_by_repo(&self, repo_name: &str, limit: usize) -> Result<Vec<BuildRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, run_id, repo_name, workflow_id, status, created_at, artifact_url, trigger_type, platform 
             FROM build_records WHERE repo_name = ?1 ORDER BY created_at DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![repo_name, limit as i64], |row| {
            Ok(BuildRecord {
                id: row.get(0)?,
                run_id: row.get(1)?,
                repo_name: row.get(2)?,
                workflow_id: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get(5)?,
                artifact_url: row.get(6)?,
                trigger_type: row.get(7)?,
                platform: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    }

    pub fn delete_record(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM build_records WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_records(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM build_records", [])?;
        Ok(())
    }

    pub fn count_records(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM build_records", [], |row| row.get(0))?;
        Ok(count)
    }
}
