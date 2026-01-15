use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;

pub struct LibraryManager {
    conn: Connection,
}

impl LibraryManager {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let manager = Self { conn };
        manager.initialize_schema()?;
        Ok(manager)
    }

    fn initialize_schema(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS tracks (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                title TEXT,
                artist TEXT,
                album TEXT,
                genre TEXT,
                duration INTEGER,
                track_number INTEGER,
                year INTEGER
            )",
            [],
        )?;
        Ok(())
    }

    pub fn add_track(&self, path: &str, title: Option<&str>, artist: Option<&str>, album: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO tracks (path, title, artist, album) VALUES (?1, ?2, ?3, ?4)",
            [path, title.unwrap_or(""), artist.unwrap_or(""), album.unwrap_or("")],
        )?;
        Ok(())
    }

    pub fn scan_directory(&self, path: &PathBuf) -> Result<()> {
        if path.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    self.scan_directory(&path)?;
                } else if is_audio_file(&path) {
                    let path_str = path.to_str().unwrap_or("");
                    self.add_track(path_str, None, None, None)?;
                }
            }
        }
        Ok(())
    }
}

fn is_audio_file(path: &PathBuf) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("mp3") | Some("flac") | Some("wav") | Some("m4a") | Some("ogg")
    )
}
