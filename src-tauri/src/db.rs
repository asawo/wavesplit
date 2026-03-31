use rusqlite::{Connection, Result, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub source_type: String,
    pub source_url: Option<String>,
    pub source_path: Option<String>,
    pub created_at: String,
    pub sort_order: i64,
    pub duration_ms: Option<i64>,
    pub status_download: String,
    pub status_stems: String,
    pub status_analysis: String,
    pub error_message: Option<String>,
    pub export_path: Option<String>,
    pub artist: Option<String>,
}

pub fn open(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch(include_str!("../schema.sql"))?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    // Migrations — .ok() makes each idempotent (fails silently if column already exists)
    conn.execute_batch("ALTER TABLE tracks ADD COLUMN export_path TEXT;").ok();
    conn.execute_batch("ALTER TABLE tracks ADD COLUMN artist TEXT;").ok();
    Ok(conn)
}

pub fn insert_track(conn: &Connection, track: &Track) -> Result<()> {
    conn.execute(
        "INSERT INTO tracks (id, title, source_type, source_url, source_path, created_at, sort_order, duration_ms, status_download, status_stems, status_analysis, error_message, export_path, artist)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            track.id, track.title, track.source_type, track.source_url, track.source_path,
            track.created_at, track.sort_order, track.duration_ms,
            track.status_download, track.status_stems, track.status_analysis, track.error_message,
            track.export_path, track.artist
        ],
    )?;
    Ok(())
}

pub fn set_export_path(conn: &Connection, id: &str, path: &str) -> Result<()> {
    conn.execute("UPDATE tracks SET export_path = ?1 WHERE id = ?2", params![path, id])?;
    Ok(())
}

pub fn update_track_meta(conn: &Connection, id: &str, title: &str, artist: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE tracks SET title = ?1, artist = ?2 WHERE id = ?3",
        params![title, artist, id],
    )?;
    Ok(())
}

pub fn list_tracks(conn: &Connection) -> Result<Vec<Track>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, source_type, source_url, source_path, created_at, sort_order, duration_ms,
                status_download, status_stems, status_analysis, error_message, export_path, artist
         FROM tracks ORDER BY sort_order ASC",
    )?;
    let tracks = stmt.query_map([], |row| {
        Ok(Track {
            id: row.get(0)?,
            title: row.get(1)?,
            source_type: row.get(2)?,
            source_url: row.get(3)?,
            source_path: row.get(4)?,
            created_at: row.get(5)?,
            sort_order: row.get(6)?,
            duration_ms: row.get(7)?,
            status_download: row.get(8)?,
            status_stems: row.get(9)?,
            status_analysis: row.get(10)?,
            error_message: row.get(11)?,
            export_path: row.get(12)?,
            artist: row.get(13)?,
        })
    })?
    .collect::<Result<Vec<_>>>()?;
    Ok(tracks)
}

pub fn update_status(
    conn: &Connection,
    id: &str,
    field: &str,
    status: &str,
    error: Option<&str>,
) -> Result<()> {
    let sql = format!(
        "UPDATE tracks SET {} = ?1, error_message = ?2 WHERE id = ?3",
        field
    );
    conn.execute(&sql, params![status, error, id])?;
    Ok(())
}

pub fn delete_track(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn incomplete_tracks(conn: &Connection) -> Result<Vec<Track>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, source_type, source_url, source_path, created_at, sort_order, duration_ms,
                status_download, status_stems, status_analysis, error_message, export_path, artist
         FROM tracks WHERE status_stems != 'done' OR status_analysis != 'done'
         ORDER BY sort_order ASC",
    )?;
    let tracks = stmt.query_map([], |row| {
        Ok(Track {
            id: row.get(0)?,
            title: row.get(1)?,
            source_type: row.get(2)?,
            source_url: row.get(3)?,
            source_path: row.get(4)?,
            created_at: row.get(5)?,
            sort_order: row.get(6)?,
            duration_ms: row.get(7)?,
            status_download: row.get(8)?,
            status_stems: row.get(9)?,
            status_analysis: row.get(10)?,
            error_message: row.get(11)?,
            export_path: row.get(12)?,
            artist: row.get(13)?,
        })
    })?
    .collect::<Result<Vec<_>>>()?;
    Ok(tracks)
}

pub fn next_sort_order(conn: &Connection) -> Result<i64> {
    let max: Option<i64> = conn.query_row(
        "SELECT MAX(sort_order) FROM tracks",
        [],
        |row| row.get(0),
    )?;
    Ok(max.unwrap_or(0) + 1)
}
