use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: u32,
    pub track_number: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
}

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
            "CREATE TABLE IF NOT EXISTS artists (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS albums (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                artist_id INTEGER,
                cover_path TEXT,
                UNIQUE(title, artist_id),
                FOREIGN KEY(artist_id) REFERENCES artists(id)
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS tracks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                artist_id INTEGER,
                album_id INTEGER,
                duration INTEGER,
                track_number INTEGER,
                year INTEGER,
                genre TEXT,
                FOREIGN KEY(artist_id) REFERENCES artists(id),
                FOREIGN KEY(album_id) REFERENCES albums(id)
            )",
            [],
        )?;

        Ok(())
    }

    fn get_or_create_artist(&self, name: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO artists (name) VALUES (?1)",
            params![name],
        )?;
        let id = self.conn.query_row(
            "SELECT id FROM artists WHERE name = ?1",
            params![name],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    fn get_or_create_album(&self, title: &str, artist_id: i64) -> Result<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO albums (title, artist_id) VALUES (?1, ?2)",
            params![title, artist_id],
        )?;
        let id = self.conn.query_row(
            "SELECT id FROM albums WHERE title = ?1 AND artist_id = ?2",
            params![title, artist_id],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn add_track(&self, path: &Path) -> Result<()> {
        let tagged_file = lofty::read_from_path(path)?;
        let tag = tagged_file.primary_tag()
            .or_else(|| tagged_file.first_tag());
        
        let properties = tagged_file.properties();
        let duration = properties.duration().as_secs() as u32;

        let title = tag.and_then(|t| t.title().map(|s| s.into_owned()))
            .unwrap_or_else(|| path.file_stem().unwrap().to_string_lossy().into_owned());
        let artist_name = tag.and_then(|t| t.artist().map(|s| s.into_owned()))
            .unwrap_or_else(|| "Unknown Artist".to_string());
        let album_title = tag.and_then(|t| t.album().map(|s| s.into_owned()))
            .unwrap_or_else(|| "Unknown Album".to_string());
        
        let track_number = tag.and_then(|t| t.track());
        let year = tag.and_then(|t| t.year());
        let genre = tag.and_then(|t| t.genre().map(|s| s.into_owned()));

        let artist_id = self.get_or_create_artist(&artist_name)?;
        let album_id = self.get_or_create_album(&album_title, artist_id)?;

        let path_str = path.to_string_lossy();

        self.conn.execute(
            "INSERT OR REPLACE INTO tracks (path, title, artist_id, album_id, duration, track_number, year, genre)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![path_str, title, artist_id, album_id, duration, track_number, year, genre],
        )?;

        Ok(())
    }

    pub fn scan_directory(&self, path: &Path) -> Result<()> {
        if path.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    self.scan_directory(&path)?;
                } else if is_audio_file(&path) {
                    if let Err(e) = self.add_track(&path) {
                        log::error!("Failed to add track {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_all_tracks(&self) -> Result<Vec<Track>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.path, t.title, ar.name as artist, al.title as album, t.duration, t.track_number, t.year, t.genre
             FROM tracks t
             JOIN artists ar ON t.artist_id = ar.id
             JOIN albums al ON t.album_id = al.id"
        )?;

        let track_iter = stmt.query_map([], |row| {
            Ok(Track {
                id: row.get(0)?,
                path: row.get(1)?,
                title: row.get(2)?,
                artist: row.get(3)?,
                album: row.get(4)?,
                duration: row.get(5)?,
                track_number: row.get(6)?,
                year: row.get(7)?,
                genre: row.get(8)?,
            })
        })?;

        let mut tracks = Vec::new();
        for track in track_iter {
            tracks.push(track?);
        }
        Ok(tracks)
    }
}

impl mlua::UserData for Track {
    fn add_fields<'lua, F: mlua::UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_lua, this| Ok(this.id));
        fields.add_field_method_get("path", |_lua, this| Ok(this.path.clone()));
        fields.add_field_method_get("title", |_lua, this| Ok(this.title.clone()));
        fields.add_field_method_get("artist", |_lua, this| Ok(this.artist.clone()));
        fields.add_field_method_get("album", |_lua, this| Ok(this.album.clone()));
        fields.add_field_method_get("duration", |_lua, this| Ok(this.duration));
    }
}

pub struct ScriptableLibraryManager(pub Arc<LibraryManager>);

impl mlua::UserData for ScriptableLibraryManager {
    fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("scan_directory", |_lua, this, path: String| {
            this.0.scan_directory(Path::new(&path)).map_err(mlua::Error::external)
        });

        methods.add_method("get_all_tracks", |_lua, this, ()| {
            this.0.get_all_tracks().map_err(mlua::Error::external)
        });
    }
}

fn is_audio_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("mp3") | Some("flac") | Some("wav") | Some("m4a") | Some("ogg")
    )
}
