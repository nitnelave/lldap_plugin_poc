use async_trait::async_trait;

use mlua::{FromLua, Function, IntoLua, Lua, LuaSerdeExt, Result, Table, Value};
use serde::{Deserialize, Serialize};
use tealr::{
    mlu::{TealDataMethods, UserData},
    ToTypename,
};

#[async_trait]
trait Handler {
    async fn get_user_impl(&self, args: GetUserArguments) -> Result<String>;
}

#[derive(Clone, UserData, ToTypename)]
struct PluginHandler;

#[async_trait]
impl Handler for PluginHandler {
    async fn get_user_impl(&self, args: GetUserArguments) -> Result<String> {
        println!("get_user_impl: {}", &args.name);
        Ok(args.name)
    }
}

impl tealr::mlu::TealData for PluginHandler {
    fn add_methods<'lua, M: TealDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_async_method("get_user", |_, this, args: GetUserArguments| async move {
            this.get_user_impl(args).await
        });
        methods.generate_help();
    }
}

#[non_exhaustive]
#[derive(Clone, Serialize, Deserialize, Default, tealr::ToTypename)]
struct GetUserArguments {
    name: String,
    #[serde(default)]
    filter: bool,
}

impl<'lua> IntoLua<'lua> for GetUserArguments {
    fn into_lua(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        lua.to_value(&self)
    }
}

impl<'lua> FromLua<'lua> for GetUserArguments {
    fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> Result<Self> {
        lua.from_value(value)
    }
}

struct Callback {
    priority: u8,
    callback: Function<'static>,
}

struct Plugins {
    lua: &'static Lua,
    on_get_user: Vec<Callback>,
}

fn load_plugin(path: &std::path::Path, plugins: &mut Plugins) -> Result<()> {
    let test_module: Table<'static> = plugins.lua.load(path).eval()?;
    for pair in test_module.pairs() {
        let (_, table): (i32, Table) = pair?;
        if table.contains_key("event")? && table.contains_key("impl")? {
            let priority = table.get("priority").unwrap_or(50);
            let event: String = table.get("event")?;
            if event == "on_get_user" {
                plugins.on_get_user.push(Callback {
                    priority,
                    callback: table.get("impl")?,
                });
            }
        }
    }
    Ok(())
}

fn load_plugins() -> Result<Plugins> {
    let lua = Box::leak(Box::new(Lua::new()));

    let mut plugins = Plugins {
        lua,
        on_get_user: Vec::new(),
    };
    load_plugin(std::path::Path::new("lua/test.lua"), &mut plugins)?;
    plugins.on_get_user.sort_by_key(|c| 255 - c.priority);
    Ok(plugins)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let handler = PluginHandler;

    let plugins = load_plugins()?;

    let mut arg = GetUserArguments {
        name: "custom_uid".to_owned(),
        filter: false,
    };
    for cb in plugins.on_get_user.iter() {
        arg = cb.callback.call_async((handler.clone(), arg)).await?;
    }
    handler.get_user_impl(arg).await?;
    Ok(())
}
