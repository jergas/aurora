use anyhow::Result;
use mlua::prelude::*;
use aurora_ui::{MainWindow, AppColors};
use slint::ComponentHandle;

pub struct ScriptHost {
    lua: Lua,
}

impl ScriptHost {
    pub fn new() -> Result<Self> {
        let lua = Lua::new();
        Ok(Self { lua })
    }

    pub fn register_global<T: LuaUserData + 'static>(&self, name: &str, obj: T) -> Result<()> {
        let globals = self.lua.globals();
        globals.set(name, obj)?;
        Ok(())
    }

    pub fn run_script(&self, script: &str) -> Result<()> {
        self.lua.load(script).exec()?;
        Ok(())
    }
}

pub struct ScriptableUI(pub slint::Weak<MainWindow>);

impl LuaUserData for ScriptableUI {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("set_track_title", |_lua, this, title: String| {
            let ui_weak = this.0.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_track_title(title.into());
                }
            });
            Ok(())
        });

        methods.add_method("set_track_artist", |_lua, this, artist: String| {
            let ui_weak = this.0.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_track_artist(artist.into());
                }
            });
            Ok(())
        });

        methods.add_method("set_background", |_lua, this, color: String| {
            let ui_weak = this.0.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let colors = ui.global::<AppColors>();
                    if let Some(slint_color) = parse_color(&color) {
                        colors.set_background(slint_color);
                    }
                }
            });
            Ok(())
        });

        methods.add_method("set_primary", |_lua, this, color: String| {
            let ui_weak = this.0.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let colors = ui.global::<AppColors>();
                    if let Some(slint_color) = parse_color(&color) {
                        colors.set_primary(slint_color);
                    }
                }
            });
            Ok(())
        });
    }
}

fn parse_color(hex: &str) -> Option<slint::Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 { return None; }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(slint::Color::from_rgb_u8(r, g, b))
}
