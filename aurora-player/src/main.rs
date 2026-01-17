use anyhow::Result;
use aurora_audio::{AudioEngine, ScriptableAudioEngine};
use aurora_core::{LibraryManager, Track, ScriptableLibraryManager};
use aurora_script::{ScriptHost, ScriptableUI};
use aurora_ui::{MainWindow, extract_palette, AppColors};
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

    // Initialize Library Manager
    let library = Arc::new(LibraryManager::new(PathBuf::from("aurora.db"))?);
    println!("Library Manager initialized.");

    // Initialize UI
    let ui = aurora_ui::create_window();
    let ui_handle = ui.as_weak();

    // Initialize Scripting Host
    let script_host = ScriptHost::new()?;
    script_host.register_global("player", ScriptableAudioEngine(engine.clone()))?;
    script_host.register_global("library", ScriptableLibraryManager(library.clone()))?;
    script_host.register_global("ui", ScriptableUI(ui_handle.clone()))?;
    println!("Scripting Host initialized.");

    // Simple test if a file is provided as argument
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let path_str = &args[1];
        let path = Path::new(path_str);
        
        if path.exists() {
            println!("Scanning directory: {:?}", path);
            library.scan_directory(path)?;
        }
    }

    // Load tracks from library
    let tracks = library.get_all_tracks()?;
    println!("Loaded {} tracks from library.", tracks.len());
    
    // Shared state for playback control
    struct PlayerState {
        tracks: Vec<Track>,
        current_index: usize,
    }
    
    let state = Arc::new(Mutex::new(PlayerState {
        tracks: tracks.clone(),
        current_index: 0,
    }));

    // Populate UI Library
    let slint_tracks: Vec<aurora_ui::LibraryTrack> = tracks.iter().map(|t| aurora_ui::LibraryTrack {
        id: t.id as i32,
        title: t.title.clone().into(),
        artist: t.artist.clone().into(),
        album: t.album.clone().into(),
    }).collect();
    
    let model = std::rc::Rc::new(slint::VecModel::from(slint_tracks));
    ui.set_library_tracks(slint::ModelRc::from(model.clone()));

    // Handle track selection from UI
    let ui_handle_select = ui.as_weak();
    let engine_select = engine.clone();
    let state_select = state.clone();
    ui.on_track_selected(move |index| {
        let index = index as usize;
        println!("UI: Track selected at index {}", index);
        let mut state = state_select.lock().unwrap();
        if index < state.tracks.len() {
            state.current_index = index;
            let track = &state.tracks[index];
            let uri = format!("file://{}", track.path);
            
            if let Err(e) = engine_select.play_file(&uri) {
                log::error!("Failed to play selected track: {}", e);
                return;
            }
            
            if let Some(ui) = ui_handle_select.upgrade() {
                ui.set_track_title(track.title.clone().into());
                ui.set_track_artist(track.artist.clone().into());
                
                // Update theme
                let cover_art = find_cover_art(Path::new(&track.path).parent().unwrap_or(Path::new(&track.path)));
                if let Some(cp) = cover_art {
                    update_ui_theme(ui_handle_select.clone(), &cp);
                }
            }
        }
    });

    // Play first track if available
    {
        let state = state.lock().unwrap();
        if !state.tracks.is_empty() {
            let track = &state.tracks[0];
            let uri = format!("file://{}", track.path);
            engine.play_file(&uri)?;
            ui.set_track_title(track.title.clone().into());
            ui.set_track_artist(track.artist.clone().into());
            
            let cover_art = find_cover_art(Path::new(&track.path).parent().unwrap_or(Path::new(&track.path)));
            if let Some(cp) = cover_art {
                update_ui_theme(ui_handle.clone(), &cp);
            }
        }
    }

    // Connect callbacks
    let engine_c = engine.clone();
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
        if state.tracks.is_empty() { return; }

        state.current_index = (state.current_index + 1) % state.tracks.len();
        
        let next_track = &state.tracks[state.current_index];
        let uri = format!("file://{}", next_track.path);
        println!("Playing Next: {}", uri);
        let _ = engine_next.play_file(&uri);
        
        // Update UI
        if let Some(ui) = ui_next.upgrade() {
            ui.set_track_title(next_track.title.clone().into());
            ui.set_track_artist(next_track.artist.clone().into());
            
            // Update cover & theme
            let track_path = Path::new(&next_track.path);
            if let Some(cp) = find_cover_art(track_path.parent().unwrap_or(track_path)) {
                update_ui_theme(ui_next.clone(), &cp);
            }
        }
    });


    ui.on_prev(move || {
        let mut state = state_prev.lock().unwrap();
        if state.tracks.is_empty() { return; }

        if state.current_index == 0 {
            state.current_index = state.tracks.len() - 1;
        } else {
            state.current_index -= 1;
        }
        
        let prev_track = &state.tracks[state.current_index];
        let uri = format!("file://{}", prev_track.path);
        println!("Playing Prev: {}", uri);
        let _ = engine_prev.play_file(&uri);
        
        // Update UI
        if let Some(ui) = ui_prev.upgrade() {
            ui.set_track_title(prev_track.title.clone().into());
            ui.set_track_artist(prev_track.artist.clone().into());

            let track_path = Path::new(&prev_track.path);
            if let Some(cp) = find_cover_art(track_path.parent().unwrap_or(track_path)) {
                update_ui_theme(ui_prev.clone(), &cp);
            }
        }
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
            
            if was_playing && !is_busy {
                 let mut state = state_poll.lock().unwrap();
                 if !state.tracks.is_empty() {
                    state.current_index = (state.current_index + 1) % state.tracks.len();
                    let next_track = &state.tracks[state.current_index];
                    let uri = format!("file://{}", next_track.path);
                    println!("Auto-advancing to: {}", uri);
                    let _ = engine_poll.play_file(&uri);
                    
                    let title = next_track.title.clone();
                    let artist = next_track.artist.clone();
                    let track_path = PathBuf::from(&next_track.path);
                    let cover_path = find_cover_art(track_path.parent().unwrap_or(&track_path));

                    let ui_weak = ui_poll.clone();
                    let cp_for_theme = cover_path.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                         if let Some(ui) = ui_weak.upgrade() {
                             ui.set_track_title(title.into());
                             ui.set_track_artist(artist.into());
                             if let Some(ref cp) = cover_path {
                                 if let Ok(slint_img) = slint::Image::load_from_path(cp) {
                                     ui.set_album_art(slint_img);
                                 }
                             }
                         }
                    });

                    // Trigger palette update separately
                    if let Some(ref cp) = cp_for_theme {
                        update_ui_theme(ui_poll.clone(), cp);
                    }
                 }
            }
            
            was_playing = is_busy;
        }
    });

    ui.run()?;

    Ok(())
}

fn find_cover_art(dir: &Path) -> Option<PathBuf> {
    if !dir.is_dir() { return None; }
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).find(|e| {
        let name = e.file_name().to_string_lossy().to_lowercase();
        let is_image = ["jpg", "jpeg", "png"].iter().any(|ext| name.ends_with(ext));
        let is_common_name = name.starts_with("cover") || name.starts_with("folder") || name.starts_with("front") || name.contains("album");
        is_image && (is_common_name || true)
    }).map(|e| e.path())
}

fn update_ui_theme(ui_handle: slint::Weak<MainWindow>, cover_path: &Path) {
    if let Ok(palette) = extract_palette(cover_path) {
        let p = ThreadSafePalette {
            bg: palette.background,
            primary: palette.primary,
            secondary: palette.secondary,
            accent: palette.accent,
        };
        let cp = cover_path.to_path_buf();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let colors = ui.global::<AppColors>();
                colors.set_background(slint::Color::from_argb_u8(255, parse_hex(&p.bg, 1), parse_hex(&p.bg, 3), parse_hex(&p.bg, 5)));
                colors.set_primary(slint::Color::from_argb_u8(255, parse_hex(&p.primary, 1), parse_hex(&p.primary, 3), parse_hex(&p.primary, 5)));
                colors.set_secondary(slint::Color::from_argb_u8(255, parse_hex(&p.secondary, 1), parse_hex(&p.secondary, 3), parse_hex(&p.secondary, 5)));
                colors.set_accent(slint::Color::from_argb_u8(255, parse_hex(&p.accent, 1), parse_hex(&p.accent, 3), parse_hex(&p.accent, 5)));
                
                if let Ok(slint_img) = slint::Image::load_from_path(&cp) {
                    ui.set_album_art(slint_img);
                }
            }
        });
    }
}

fn parse_hex(hex: &str, start: usize) -> u8 {
    u8::from_str_radix(&hex[start..start+2], 16).unwrap_or(0)
}
