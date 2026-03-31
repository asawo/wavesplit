use std::path::PathBuf;

pub fn track_dir(data_dir: &PathBuf, id: &str) -> PathBuf {
    data_dir.join("tracks").join(id)
}

pub fn stems_dir(data_dir: &PathBuf, id: &str) -> PathBuf {
    track_dir(data_dir, id).join("stems")
}

pub fn analysis_dir(data_dir: &PathBuf, id: &str) -> PathBuf {
    track_dir(data_dir, id).join("analysis")
}

pub fn source_wav(data_dir: &PathBuf, id: &str) -> PathBuf {
    track_dir(data_dir, id).join("source.wav")
}

pub fn temp_dir(id: &str) -> PathBuf {
    std::env::temp_dir().join("wavesplit").join(id)
}
