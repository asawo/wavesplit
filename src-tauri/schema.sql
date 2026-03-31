CREATE TABLE IF NOT EXISTS tracks (
    id               TEXT PRIMARY KEY,
    title            TEXT NOT NULL,
    source_type      TEXT NOT NULL CHECK (source_type IN ('youtube', 'local')),
    source_url       TEXT,
    source_path      TEXT,
    created_at       TEXT NOT NULL,
    sort_order       INTEGER NOT NULL,
    duration_ms      INTEGER,
    status_download  TEXT NOT NULL DEFAULT 'pending' CHECK (status_download  IN ('pending', 'done', 'error')),
    status_stems     TEXT NOT NULL DEFAULT 'pending' CHECK (status_stems     IN ('pending', 'done', 'error')),
    status_analysis  TEXT NOT NULL DEFAULT 'pending' CHECK (status_analysis  IN ('pending', 'done', 'error')),
    error_message    TEXT
);
