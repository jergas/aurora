use anyhow::Result;
use aurora_audio::AudioEngine;
use aurora_ui::{MainWindow, ThemePalette, extract_palette, AppColors};
use slint::ComponentHandle;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

struct ThreadSafePalette {
   bg: String,
   primary: String,
   secondary: String,
   accent: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    println!("Aurora Music Player starting...");

    let engine = Arc::new(AudioEngine::new()?);
    println!("Audio Engine initialized.");

    let ui = aurora_ui::create_window();
    let ui_handle = ui.as_weak();

    let mut playlist: Vec<PathBuf> = Vec::new();
    let _current_index: Option<usize> = None;

    // Simple test if a file is provided as argument
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let path_str = &args[1];
        let path = Path::new(path_str);
        
        if path.exists() {
            let (target_file, cover_path) = if path.is_dir() {
                // Find all audio files for the playlist
                playlist = std::fs::read_dir(path)?
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                            ["mp3", "flac", "wav", "m4a", "ogg"].contains(&ext)
                        } else {
                            false
                        }
                    })
                    .collect();
                playlist.sort(); 
                
                let audio = playlist.first().cloned();
                
                // Find cover art
                let cover = std::fs::read_dir(path)?
                    .filter_map(|e| e.ok())
                    .find(|e| {
                        let name = e.file_name().to_string_lossy().to_lowercase();
                        let is_image = ["jpg", "jpeg", "png"].iter().any(|ext| name.ends_with(ext));
                        let is_common_name = name.starts_with("cover") || name.starts_with("folder") || name.starts_with("front") || name.starts_with("crosby");
                        is_image && is_common_name
                    })
                    .or_else(|| {
                         std::fs::read_dir(path).ok()?.filter_map(|e| e.ok()).find(|e| {
                             let name = e.file_name().to_string_lossy().to_lowercase();
                             ["jpg", "jpeg", "png"].iter().any(|ext| name.ends_with(ext))
                         })
                    })
                    .map(|e| e.path());

                (audio, cover)
            } else {
                playlist = vec![path.to_path_buf()];
                let cover = path.parent()
                    .and_then(|p| std::fs::read_dir(p).ok())
                    .and_then(|mut entries| {
                        entries.find_map(|e| {
                            let e = e.ok()?;
                            let name = e.file_name().to_string_lossy().to_lowercase();
                            let is_image = ["jpg", "jpeg", "png"].iter().any(|ext| name.ends_with(ext));
                            if is_image && (name.starts_with("cover") || name.starts_with("folder") || name.starts_with("front")) {
                                Some(e.path())
                            } else {
                                None
                            }
                        })
                    });
                (Some(path.to_path_buf()), cover)
            };

            if let Some(audio_path) = target_file {
                let uri = format!("file://{}", audio_path.canonicalize()?.display());
                println!("Playing: {}", uri);
                engine.play_file(&uri)?;
                
                // Update UI metadata
                ui.set_track_title(audio_path.file_name().unwrap().to_string_lossy().to_string().into());

                if let Some(cp) = cover_path {
                    println!("Found cover art: {:?}", cp);
                    if let Ok(palette) = extract_palette(&cp) {
                        // Update UI cover image
                        if let Ok(slint_img) = slint::Image::load_from_path(&cp) {
                            ui.set_album_art(slint_img);
                        }

                        let p = ThreadSafePalette {
                            bg: palette.background,
                            primary: palette.primary,
                            secondary: palette.secondary,
                            accent: palette.accent,
                        };

                        let ui_weak = ui_handle.clone(); 
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let colors = ui.global::<AppColors>();
                                
                                colors.set_background(slint::Color::from_argb_u8(255, 
                                    parse_hex(&p.bg, 1), 
                                    parse_hex(&p.bg, 3), 
                                    parse_hex(&p.bg, 5)));
                                colors.set_primary(slint::Color::from_argb_u8(255, 
                                    parse_hex(&p.primary, 1), 
                                    parse_hex(&p.primary, 3), 
                                    parse_hex(&p.primary, 5)));
                                 colors.set_secondary(slint::Color::from_argb_u8(255, 
                                    parse_hex(&p.secondary, 1), 
                                    parse_hex(&p.secondary, 3), 
                                    parse_hex(&p.secondary, 5)));
                                 colors.set_accent(slint::Color::from_argb_u8(255, 
                                    parse_hex(&p.accent, 1), 
                                    parse_hex(&p.accent, 3), 
                                    parse_hex(&p.accent, 5)));
                            }
                        });
                    }
                }
            }
        }
    }

    // Shared state for playback control
    struct PlayerState {
        playlist: Vec<PathBuf>,
        current_index: usize,
    }
    let state = Arc::new(Mutex::new(PlayerState {
        playlist: playlist,
        current_index: 0,
    }));

    // Connect callbacks
    let engine_c = engine.clone();
    let state_c = state.clone();
    let engine_next = engine.clone();
    let state_next = state.clone();
    let ui_next = ui_handle.clone();
    let engine_prev = engine.clone();
    let state_prev = state.clone();
    let ui_prev = ui_handle.clone();

    let mut is_paused = false;
    ui.on_play_pause(move || {
        if is_paused {
            let _ = engine_c.resume();
            is_paused = false;
        } else {
            let _ = engine_c.pause();
            is_paused = true;
        }
    });

    ui.on_next(move || {
        let mut state = state_next.lock().unwrap();
        if state.playlist.is_empty() { return; }
        
        state.current_index = (state.current_index + 1) % state.playlist.len();
        let next_path = &state.playlist[state.current_index];
        let uri = format!("file://{}", next_path.to_string_lossy());
        println!("Playing Next: {}", uri);
        let _ = engine_next.play_file(&uri);
        
        // Update UI
        let ui = ui_next.unwrap();
        ui.set_track_title(next_path.file_name().unwrap().to_string_lossy().to_string().into());
        // Note: Cover art update logic would need to be duplicated or refactored here
    });

    ui.on_prev(move || {
        let mut state = state_prev.lock().unwrap();
        if state.playlist.is_empty() { return; }

        if state.current_index == 0 {
            state.current_index = state.playlist.len() - 1;
        } else {
            state.current_index -= 1;
        }
        
        let prev_path = &state.playlist[state.current_index];
        let uri = format!("file://{}", prev_path.to_string_lossy());
        println!("Playing Prev: {}", uri);
        let _ = engine_prev.play_file(&uri);
        
        // Update UI
        let ui = ui_prev.unwrap();
        ui.set_track_title(prev_path.file_name().unwrap().to_string_lossy().to_string().into());
    });

    // Auto-advance loop
    let engine_poll = engine.clone();
    let state_poll = state.clone();
    let ui_poll = ui_handle.clone();
    
    tokio::spawn(async move {
        let mut was_playing = false;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            
            let is_busy = engine_poll.is_busy();
            
            // If we were playing and now we are not busy (queue empty), move to next
            // Note: This is a simple logic. If player starts paused, it won't auto-advance.
            // But if we just finished a track, is_busy becomes false.
            if was_playing && !is_busy {
                 let mut state = state_poll.lock().unwrap();
                 if !state.playlist.is_empty() {
                    state.current_index = (state.current_index + 1) % state.playlist.len();
                    let next_path = &state.playlist[state.current_index];
                    let uri = format!("file://{}", next_path.to_string_lossy());
                    println!("Auto-advancing to: {}", uri);
                    let _ = engine_poll.play_file(&uri);
                    
                    let next_title = next_path.file_name().unwrap().to_string_lossy().to_string();
                    
                    let ui_weak = ui_poll.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                         if let Some(ui) = ui_weak.upgrade() {
                             ui.set_track_title(next_title.into());
                         }
                    });
                 }
            }
            
            was_playing = is_busy;
        }
    });

    ui.run()?;

    Ok(())
}

fn parse_hex(hex: &str, start: usize) -> u8 {
    u8::from_str_radix(&hex[start..start+2], 16).unwrap_or(0)
}
