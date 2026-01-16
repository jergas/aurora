use anyhow::Result;
use mlua::prelude::*;

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
