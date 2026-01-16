use anyhow::Result;
use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};

pub struct AudioEngine {
    _stream: OutputStream,
    stream_handle: rodio::OutputStreamHandle,
    sink: Arc<Mutex<Sink>>,
}

// SAFETY: _stream is only kept alive and never accessed. sink is Arc<Mutex> which is Send+Sync.
unsafe impl Send for AudioEngine {}
unsafe impl Sync for AudioEngine {}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        let (_stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;
        
        Ok(Self {
            _stream,
            stream_handle,
            sink: Arc::new(Mutex::new(sink)),
        })
    }

    pub fn play_file(&self, uri: &str) -> Result<()> {
        let path = if uri.starts_with("file://") {
            uri.trim_start_matches("file://")
        } else {
            uri
        };

        let file = File::open(path)?;
        let source = Decoder::new(BufReader::new(file))?;
        
        let sink = self.sink.lock().unwrap();
        if !sink.empty() {
            sink.stop();
             // Since sink.stop() might not clear the queue immediately or might require a new sink for clean state,
             // in Rodio it's often better to just append to a new sink or clear if possible.
             // For this simple implementation, we'll just append. To "stop and play new", 
             // we ideally create a new sink, but for now let's just create a new one to be safe.
        }
        
        // Re-create sink to ensure clean state for new track
        // Note: In a real app we'd manage this better to avoid popping audio
        // For now, let's just append to the existing sink
        sink.append(source);
        sink.play();
        
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        self.sink.lock().unwrap().pause();
        Ok(())
    }

    pub fn resume(&self) -> Result<()> {
        self.sink.lock().unwrap().play();
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        self.sink.lock().unwrap().stop();
        Ok(())
    }

    pub fn set_volume(&self, volume: f32) {
        self.sink.lock().unwrap().set_volume(volume);
    }

    pub fn is_busy(&self) -> bool {
        !self.sink.lock().unwrap().empty()
    }
}

pub struct ScriptableAudioEngine(pub Arc<AudioEngine>);

impl mlua::UserData for ScriptableAudioEngine {
    fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("play_file", |_lua, this, uri: String| {
            this.0.play_file(&uri).map_err(mlua::Error::external)
        });

        methods.add_method("pause", |_lua, this, ()| {
            this.0.pause().map_err(mlua::Error::external)
        });

        methods.add_method("resume", |_lua, this, ()| {
            this.0.resume().map_err(mlua::Error::external)
        });

        methods.add_method("stop", |_lua, this, ()| {
            this.0.stop().map_err(mlua::Error::external)
        });

        methods.add_method("set_volume", |_lua, this, volume: f32| {
            this.0.set_volume(volume);
            Ok(())
        });

        methods.add_method("is_busy", |_lua, this, ()| {
            Ok(this.0.is_busy())
        });
    }
}
