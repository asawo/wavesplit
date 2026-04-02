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

