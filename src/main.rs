use async_trait::async_trait;

use mlua::{
    Error as LuaError, FromLua, FromLuaMulti, Function, IntoLua, IntoLuaMulti, Lua, LuaSerdeExt,
    Result as LuaResult, Table, Value,
};
use serde::{Deserialize, Serialize};
use tealr::{
    mlu::{TealDataMethods, UserData},
    ToTypename,
};

#[async_trait]
trait Handler {
    async fn get_user_impl(&self, args: GetUserArguments) -> anyhow::Result<String>;
}

#[derive(Clone, UserData, ToTypename)]
struct PluginHandler;

#[async_trait]
impl Handler for PluginHandler {
    async fn get_user_impl(&self, args: GetUserArguments) -> anyhow::Result<String> {
        println!("get_user_impl: {}", &args.name);
        Ok(args.name)
    }
}

struct MyLuaResult<T>(Result<T, String>);

impl<T: ToTypename> tealr::ToTypename for MyLuaResult<T> {
    fn to_typename() -> tealr::Type {
        tealr::Type::Or(vec![T::to_typename(), String::to_typename()])
    }
}

impl<'lua, T: IntoLuaMulti<'lua>> IntoLuaMulti<'lua> for MyLuaResult<T> {
    fn into_lua_multi(self, lua: &'lua Lua) -> LuaResult<mlua::MultiValue<'lua>> {
        match self.0 {
            Ok(v) => v.into_lua_multi(lua),
            Err(s) => Ok(mlua::MultiValue::from_iter([lua.null(), s.into_lua(lua)?])),
        }
    }
}

impl<T> From<anyhow::Result<T>> for MyLuaResult<T> {
    fn from(value: anyhow::Result<T>) -> Self {
        Self(value.map_err(|e| e.to_string()))
    }
}

impl<'lua, T: FromLua<'lua>> mlua::FromLuaMulti<'lua> for MyLuaResult<T> {
    fn from_lua_multi(values: mlua::MultiValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
        let mut values = values.into_vec();
        if values.len() == 1 {
            Ok(MyLuaResult(Ok(FromLua::from_lua(values.remove(0), lua)?)))
        } else if values.len() == 2 {
            if values[0].is_nil() {
                Ok(MyLuaResult(Err(lua.from_value(values.remove(1))?)))
            } else {
                Err(LuaError::external("Multiple values not supported"))
            }
        } else if values.is_empty() {
            unreachable!()
        } else {
            Err(LuaError::external("Too many values"))
        }
    }
}

impl tealr::mlu::TealData for PluginHandler {
    fn add_methods<'lua, M: TealDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_async_method("get_user", |_, this, args: GetUserArguments| async move {
            //LuaResult::Ok(MyLuaResult::<String>(Err("get_user error".to_owned())))
            LuaResult::Ok(MyLuaResult::from(this.get_user_impl(args).await))
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
    fn into_lua(self, lua: &'lua Lua) -> LuaResult<Value<'lua>> {
        lua.to_value(&self)
    }
}

impl<'lua> FromLua<'lua> for GetUserArguments {
    fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
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

fn load_plugin(path: &std::path::Path, plugins: &mut Plugins) -> LuaResult<()> {
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

fn load_plugins() -> LuaResult<Plugins> {
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
async fn main() -> anyhow::Result<()> {
    let handler = PluginHandler;

    let plugins = load_plugins()?;

    let mut arg = GetUserArguments {
        name: "custom_uid".to_owned(),
        filter: false,
    };
    for cb in plugins.on_get_user.iter() {
        let maybe_arg: MyLuaResult<GetUserArguments> = FromLuaMulti::from_lua_multi(
            cb.callback
                .call_async((handler.clone(), arg.clone()))
                .await?,
            plugins.lua,
        )?;
        match maybe_arg.0 {
            Ok(v) => arg = v,
            Err(e) => println!("Got an error while executing plugin, skipping: {}", e),
        }
    }
    handler.get_user_impl(arg).await?;
    Ok(())
}
