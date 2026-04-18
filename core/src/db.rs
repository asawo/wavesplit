use rusqlite::{Connection, Result, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub enum StatusField {
    Download,
    Stems,
    Analysis,
}

impl StatusField {
    fn column(&self) -> &'static str {
        match self {
            Self::Download => "status_download",
            Self::Stems => "status_stems",
            Self::Analysis => "status_analysis",
        }
    }
}

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
         FROM tracks ORDER BY sort_order DESC",
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
    field: StatusField,
    status: &str,
    error: Option<&str>,
) -> Result<()> {
    let sql = format!(
        "UPDATE tracks SET {} = ?1, error_message = ?2 WHERE id = ?3",
        field.column()
    );
    conn.execute(&sql, params![status, error, id])?;
    Ok(())
}

pub fn delete_track(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
    Ok(())
}

/// On startup: reset any track with a pending stage to error so the UI can offer retry.
pub fn mark_interrupted(conn: &Connection) -> Result<()> {
    conn.execute(
        "UPDATE tracks
         SET status_download = CASE WHEN status_download = 'pending' THEN 'error' ELSE status_download END,
             status_stems    = CASE WHEN status_stems    = 'pending' THEN 'error' ELSE status_stems    END,
             status_analysis = CASE WHEN status_analysis = 'pending' THEN 'error' ELSE status_analysis END,
             error_message   = 'interrupted'
         WHERE status_download = 'pending' OR status_stems = 'pending' OR status_analysis = 'pending'",
        [],
    )?;
    Ok(())
}

pub fn get_track(conn: &Connection, id: &str) -> Result<Option<Track>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, source_type, source_url, source_path, created_at, sort_order, duration_ms,
                status_download, status_stems, status_analysis, error_message, export_path, artist
         FROM tracks WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
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
    })?;
    rows.next().transpose()
}

pub fn reset_for_retry(conn: &Connection, id: &str, reset_download: bool, reset_stems: bool) -> Result<()> {
    if reset_download {
        conn.execute(
            "UPDATE tracks SET status_download = 'pending', status_stems = 'pending', status_analysis = 'pending', error_message = NULL WHERE id = ?1",
            params![id],
        )?;
    } else if reset_stems {
        conn.execute(
            "UPDATE tracks SET status_stems = 'pending', status_analysis = 'pending', error_message = NULL WHERE id = ?1",
            params![id],
        )?;
    } else {
        conn.execute(
            "UPDATE tracks SET status_analysis = 'pending', error_message = NULL WHERE id = ?1",
            params![id],
        )?;
    }
    Ok(())
}

pub fn next_sort_order(conn: &Connection) -> Result<i64> {
    let max: Option<i64> = conn.query_row(
        "SELECT MAX(sort_order) FROM tracks",
        [],
        |row| row.get(0),
    )?;
    Ok(max.unwrap_or(0) + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn open_mem() -> Connection {
        open(Path::new(":memory:")).unwrap()
    }

    fn sample_track(id: &str) -> Track {
        Track {
            id: id.to_string(),
            title: "Test Track".to_string(),
            source_type: "youtube".to_string(),
            source_url: Some("https://example.com".to_string()),
            source_path: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            sort_order: 1,
            duration_ms: None,
            status_download: "pending".to_string(),
            status_stems: "pending".to_string(),
            status_analysis: "pending".to_string(),
            error_message: None,
            export_path: None,
            artist: None,
        }
    }

    #[test]
    fn insert_and_list_round_trips() {
        let conn = open_mem();
        insert_track(&conn, &sample_track("t1")).unwrap();
        let tracks = list_tracks(&conn).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "t1");
        assert_eq!(tracks[0].title, "Test Track");
    }

    #[test]
    fn update_status_sets_done() {
        let conn = open_mem();
        insert_track(&conn, &sample_track("t2")).unwrap();
        update_status(&conn, "t2", StatusField::Download, "done", None).unwrap();
        let track = get_track(&conn, "t2").unwrap().unwrap();
        assert_eq!(track.status_download, "done");
        assert_eq!(track.error_message, None);
    }

    #[test]
    fn update_status_stores_error_message() {
        let conn = open_mem();
        insert_track(&conn, &sample_track("t3")).unwrap();
        update_status(&conn, "t3", StatusField::Download, "error", Some("network failure")).unwrap();
        let track = get_track(&conn, "t3").unwrap().unwrap();
        assert_eq!(track.status_download, "error");
        assert_eq!(track.error_message.as_deref(), Some("network failure"));
    }

    #[test]
    fn mark_interrupted_resets_pending_stages_to_error() {
        let conn = open_mem();
        let mut track = sample_track("t4");
        track.status_download = "done".to_string();
        insert_track(&conn, &track).unwrap();
        mark_interrupted(&conn).unwrap();
        let track = get_track(&conn, "t4").unwrap().unwrap();
        assert_eq!(track.status_download, "done"); // already done — unchanged
        assert_eq!(track.status_stems, "error");
        assert_eq!(track.status_analysis, "error");
        assert_eq!(track.error_message.as_deref(), Some("interrupted"));
    }

    #[test]
    fn mark_interrupted_ignores_finished_tracks() {
        let conn = open_mem();
        let mut track = sample_track("t5");
        track.status_download = "done".to_string();
        track.status_stems = "done".to_string();
        track.status_analysis = "done".to_string();
        insert_track(&conn, &track).unwrap();
        mark_interrupted(&conn).unwrap();
        let track = get_track(&conn, "t5").unwrap().unwrap();
        assert_eq!(track.status_analysis, "done");
        assert_eq!(track.error_message, None);
    }

    #[test]
    fn next_sort_order_increments() {
        let conn = open_mem();
        assert_eq!(next_sort_order(&conn).unwrap(), 1);
        let mut track = sample_track("t6");
        track.sort_order = 1;
        insert_track(&conn, &track).unwrap();
        assert_eq!(next_sort_order(&conn).unwrap(), 2);
    }

    #[test]
    fn reset_for_retry_from_download_resets_all() {
        let conn = open_mem();
        let mut track = sample_track("t7");
        track.status_download = "error".to_string();
        track.error_message = Some("failed".to_string());
        insert_track(&conn, &track).unwrap();
        reset_for_retry(&conn, "t7", true, true).unwrap();
        let track = get_track(&conn, "t7").unwrap().unwrap();
        assert_eq!(track.status_download, "pending");
        assert_eq!(track.status_stems, "pending");
        assert_eq!(track.status_analysis, "pending");
        assert_eq!(track.error_message, None);
    }

    #[test]
    fn get_track_returns_none_for_missing_id() {
        let conn = open_mem();
        assert!(get_track(&conn, "nonexistent").unwrap().is_none());
    }

    #[test]
    fn update_track_meta_changes_title_and_artist() {
        let conn = open_mem();
        insert_track(&conn, &sample_track("t9")).unwrap();
        update_track_meta(&conn, "t9", "New Title", Some("New Artist")).unwrap();
        let track = get_track(&conn, "t9").unwrap().unwrap();
        assert_eq!(track.title, "New Title");
        assert_eq!(track.artist.as_deref(), Some("New Artist"));
    }

    #[test]
    fn update_track_meta_clears_artist_when_none() {
        let conn = open_mem();
        let mut track = sample_track("t10");
        track.artist = Some("Old Artist".to_string());
        insert_track(&conn, &track).unwrap();
        update_track_meta(&conn, "t10", "Title", None).unwrap();
        let track = get_track(&conn, "t10").unwrap().unwrap();
        assert_eq!(track.artist, None);
    }

    #[test]
    fn set_export_path_stores_path() {
        let conn = open_mem();
        insert_track(&conn, &sample_track("t11")).unwrap();
        set_export_path(&conn, "t11", "/exports/t11").unwrap();
        let track = get_track(&conn, "t11").unwrap().unwrap();
        assert_eq!(track.export_path.as_deref(), Some("/exports/t11"));
    }

    #[test]
    fn delete_track_removes_row() {
        let conn = open_mem();
        insert_track(&conn, &sample_track("t12")).unwrap();
        assert!(get_track(&conn, "t12").unwrap().is_some());
        delete_track(&conn, "t12").unwrap();
        assert!(get_track(&conn, "t12").unwrap().is_none());
    }

    #[test]
    fn reset_for_retry_from_stems_preserves_download() {
        let conn = open_mem();
        let mut track = sample_track("t8");
        track.status_download = "done".to_string();
        track.status_stems = "error".to_string();
        track.error_message = Some("demucs failed".to_string());
        insert_track(&conn, &track).unwrap();
        reset_for_retry(&conn, "t8", false, true).unwrap();
        let track = get_track(&conn, "t8").unwrap().unwrap();
        assert_eq!(track.status_download, "done");
        assert_eq!(track.status_stems, "pending");
        assert_eq!(track.status_analysis, "pending");
        assert_eq!(track.error_message, None);
    }
}
