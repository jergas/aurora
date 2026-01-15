use anyhow::Result;
use mlua::prelude::*;
use std::sync::{Arc, Mutex};

pub struct ScriptHost {
    lua: Lua,
}

impl ScriptHost {
    pub fn new() -> Result<Self> {
        let lua = Lua::new();
        Ok(Self { lua })
    }

    pub fn run_script(&self, script: &str) -> Result<()> {
        self.lua.load(script).exec()?;
        Ok(())
    }
}
