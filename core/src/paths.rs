use std::path::{Path, PathBuf};

/// The only track-id shape the app ever mints is a UUID (`Uuid::new_v4()` in `add_track`),
/// so this is both a format check and a path-traversal guard: a well-formed UUID string can
/// never contain `/`, `..`, or an absolute-path prefix. Returns the canonical (lowercase,
/// hyphenated) form so callers can't be fooled by e.g. uppercase-hex variants of a real id
/// mismatching the lowercase form stored in the DB/on disk.
///
/// Every Tauri command that accepts an id/track_id from the frontend must call this first and
/// use the returned normalized string for everything downstream (DB calls, path joins).
pub fn parse_track_id(raw: &str) -> Result<String, String> {
    uuid::Uuid::parse_str(raw)
        .map(|u| u.to_string())
        .map_err(|_| "invalid track id".to_string())
}

pub fn track_dir(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join("tracks").join(id)
}

pub fn stems_dir(data_dir: &Path, id: &str) -> PathBuf {
    track_dir(data_dir, id).join("stems")
}

pub fn analysis_dir(data_dir: &Path, id: &str) -> PathBuf {
    track_dir(data_dir, id).join("analysis")
}

pub fn source_wav(data_dir: &Path, id: &str) -> PathBuf {
    track_dir(data_dir, id).join("source.wav")
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = "/data";

    #[test]
    fn track_dir_is_data_tracks_id() {
        assert_eq!(
            track_dir(Path::new(DATA), "abc"),
            Path::new("/data/tracks/abc")
        );
    }

    #[test]
    fn stems_dir_is_under_track_dir() {
        assert_eq!(
            stems_dir(Path::new(DATA), "abc"),
            Path::new("/data/tracks/abc/stems")
        );
    }

    #[test]
    fn analysis_dir_is_under_track_dir() {
        assert_eq!(
            analysis_dir(Path::new(DATA), "abc"),
            Path::new("/data/tracks/abc/analysis")
        );
    }

    #[test]
    fn source_wav_is_in_track_dir() {
        assert_eq!(
            source_wav(Path::new(DATA), "abc"),
            Path::new("/data/tracks/abc/source.wav")
        );
    }

    #[test]
    fn stem_filenames_are_under_stems_dir() {
        let base = stems_dir(Path::new(DATA), "abc");
        for stem in crate::constants::STEM_NAMES {
            assert_eq!(base.join(format!("{stem}.wav")).parent().unwrap(), base);
        }
    }

    #[test]
    fn parse_track_id_accepts_valid_uuid() {
        assert_eq!(
            parse_track_id("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn parse_track_id_normalizes_case() {
        assert_eq!(
            parse_track_id("550E8400-E29B-41D4-A716-446655440000").unwrap(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn parse_track_id_rejects_traversal_and_absolute_paths() {
        assert!(parse_track_id("../../etc").is_err());
        assert!(parse_track_id("/etc/passwd").is_err());
        assert!(parse_track_id("").is_err());
        assert!(parse_track_id("not-a-uuid").is_err());
    }

    #[test]
    fn track_dir_stays_direct_child_of_tracks_root_for_any_valid_id() {
        let data_dir = Path::new(DATA);
        for raw in [
            "550e8400-e29b-41d4-a716-446655440000",
            "550E8400-E29B-41D4-A716-446655440000",
        ] {
            let id = parse_track_id(raw).unwrap();
            let dir = track_dir(data_dir, &id);
            assert_eq!(dir.parent(), Some(data_dir.join("tracks").as_path()));
        }
    }
}
