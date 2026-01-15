# Aurora Music Player

A high-quality, scriptable music player framework and application.

## Features

- **High Quality Audio**: Powered by Rust and GStreamer.
- **Low Latency**: Optimized audio pipeline.
- **Scriptable UI**: Extend the interface using Lua scripts.
- **Dynamic Theming**: Interface colors adjust automatically to match the current album cover.
- **Library Management**: SQLite-backed library for fast searching and organization.

## Stack

- **Core**: Rust
- **Audio**: GStreamer
- **UI**: Slint
- **Scripting**: Lua
- **Database**: SQLite

## Development

### Prerequisites

- Rust (latest stable)
- GStreamer 1.0 + Plugins (Base, Good, Bad, Ugly)
- `pkg-config`

### Building

```bash
cargo build
```

### Running

```bash
cargo run -p aurora-player -- <path_to_audio_file>
```
