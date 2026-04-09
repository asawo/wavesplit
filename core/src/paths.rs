use std::path::{Path, PathBuf};

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
        assert_eq!(track_dir(Path::new(DATA), "abc"), Path::new("/data/tracks/abc"));
    }

    #[test]
    fn stems_dir_is_under_track_dir() {
        assert_eq!(stems_dir(Path::new(DATA), "abc"), Path::new("/data/tracks/abc/stems"));
    }

    #[test]
    fn analysis_dir_is_under_track_dir() {
        assert_eq!(analysis_dir(Path::new(DATA), "abc"), Path::new("/data/tracks/abc/analysis"));
    }

    #[test]
    fn source_wav_is_in_track_dir() {
        assert_eq!(source_wav(Path::new(DATA), "abc"), Path::new("/data/tracks/abc/source.wav"));
    }

    #[test]
    fn stem_filenames_are_under_stems_dir() {
        let base = stems_dir(Path::new(DATA), "abc");
        for name in ["vocals.wav", "drums.wav", "bass.wav", "other.wav"] {
            assert_eq!(base.join(name).parent().unwrap(), base);
        }
    }
}
