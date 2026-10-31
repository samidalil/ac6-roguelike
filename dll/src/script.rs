//! Chip scripts (step 5 of RE_NOTES "Modular chips: architecture and roadmap"): a chip may have a Lua 5.4 file next
//! to its .toml (chips/<name>.lua). Its `on_<event>(e)` functions run when the engine emits the event and the chip is
//! at level 1 or more (`e.level`, `e.value` = the level's value, plus the event's own fields), on the game thread
//! (penalty.rs frame hook, the frame after the event). Each chip runs in its own environment: the safe API (game state
//! reads, the chip's saved byte, the declarative actions) and, with `unsafe = true` in the chip file, the `unsafe`
//! table (raw memory). A script error is logged and stops that call only; a call is cut after BUDGET instructions.

use super::*;
use mlua::{HookTriggers, Lua, LuaOptions, StdLib, Table, VmState};
use std::cell::{Cell, RefCell};
use std::sync::Mutex;

/// An event for a chip's script: chip id, event, its level then, the player character at the event (a SpEffect queued
/// at `mission_start` waits for the next one), event fields.
struct Call { chip: u16, e: event::Event, lv: i32, not: usize, data: Vec<(&'static str, f64)> }
static QUEUE: Mutex<Vec<Call>> = Mutex::new(Vec::new());

/// Any thread: the engine emitted `e` for a chip whose script has `on_<e>`.
pub fn queue(chip: u16, e: event::Event, lv: i32, not: usize, data: &[(&'static str, f64)]) {
    QUEUE.lock().unwrap_or_else(|e| e.into_inner()).push(Call { chip, e, lv, not, data: data.to_vec() });
}

/// Instructions a call may run before it is stopped (an endless loop would freeze the game).
const BUDGET: u32 = 2_000_000;
const HOOK_EVERY: u32 = 10_000;

struct Engine { lua: Lua, envs: Vec<(u16, Table)> }
thread_local! {
    static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
    /// The call running now: (level, event, player character at the event), read by the action functions.
    static NOW: Cell<(i32, Option<event::Event>, usize)> = const { Cell::new((0, None, 0)) };
    static STEPS: Cell<u32> = const { Cell::new(0) };
}

/// Game thread, every frame: run the queued script calls (the engine is built at the first one).
pub unsafe fn run_queue() {
    let calls: Vec<Call> = {
        let mut q = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
        if q.is_empty() { return; }
        core::mem::take(&mut *q)
    };
    ENGINE.with(|cell| {
        let mut eng = cell.borrow_mut();
        if eng.is_none() { *eng = Some(build()); }
        let eng = eng.as_ref().unwrap();
        for c in calls {
            let Some((_, env)) = eng.envs.iter().find(|x| x.0 == c.chip) else { continue };
            let Some(chip) = chip::chips().iter().find(|x| x.id == c.chip) else { continue };
            let fname = format!("on_{}", event::name(c.e));
            let Ok(f) = env.get::<mlua::Function>(fname.as_str()) else { continue };
            let arg = match eng.lua.create_table() { Ok(t) => t, Err(_) => continue };
            let _ = arg.set("level", c.lv);
            let _ = arg.set("value", chip::value(chip.row(), c.lv));
            for (k, v) in &c.data { let _ = arg.set(*k, *v); }
            NOW.with(|n| n.set((c.lv, Some(c.e), c.not)));
            STEPS.with(|s| s.set(0));
            if let Err(e) = f.call::<()>(arg) { logf!("script {}: {} failed: {}", chip.name, fname, e); }
            NOW.with(|n| n.set((0, None, 0)));
        }
    });
}

/// The Lua state: the safe standard libraries (no io / os / package / debug, no file loading), the instruction budget,
/// then each chip script run once in its own environment (its functions stay in it).
fn build() -> Engine {
    let lua = Lua::new_with(StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8, LuaOptions::default())
        .unwrap_or_else(|_| Lua::new());
    let g = lua.globals();
    for k in ["dofile", "loadfile", "load", "require", "collectgarbage", "print"] { let _ = g.raw_remove(k); }
    lua.set_hook(HookTriggers::new().every_nth_instruction(HOOK_EVERY), |_, _| {
        let n = STEPS.with(|s| { let n = s.get() + HOOK_EVERY; s.set(n); n });
        if n > BUDGET { Err(mlua::Error::runtime(format!("stopped after {} instructions", BUDGET))) } else { Ok(VmState::Continue) }
    });
    let mut envs = Vec::new();
    for c in chip::chips() {
        let Some((file, src)) = &c.script else { continue };
        let r = env(&lua, c).and_then(|env| {
            STEPS.with(|s| s.set(0));
            lua.load(src.as_str()).set_name(file.as_str()).set_environment(env.clone()).exec().map(|_| env)
        });
        match r {
            Ok(env) => { logf!("script {}: {} loaded{}", c.name, file, if c.unsafe_api { " (unsafe API)" } else { "" }); envs.push((c.id, env)); }
            Err(e) => logf!("script {}: {}: {}", c.name, file, e),
        }
    }
    Engine { lua, envs }
}

/// A chip's environment: the globals (read through __index), `log`, `chip`, `store`, `game`, the actions, `unsafe`.
fn env(lua: &Lua, c: &'static chip::Chip) -> mlua::Result<Table> {
    let env = lua.create_table()?;
    let meta = lua.create_table()?;
    meta.set("__index", lua.globals())?;
    env.set_metatable(Some(meta));
    let row = c.row();

    env.set("log", lua.create_function(move |_, args: mlua::Variadic<mlua::Value>| {
        let s: Vec<String> = args.iter().map(|v| v.to_string().unwrap_or_else(|_| format!("{:?}", v))).collect();
        logf!("script {}: {}", c.name, s.join(" "));
        Ok(())
    })?)?;

    // chip: its id, name, values; level() and value() now
    let t = lua.create_table()?;
    t.set("id", c.id)?;
    t.set("name", c.name.as_str())?;
    t.set("values", c.values.clone())?;
    // params: [params] of the chip file; a number, "a/b/c" numbers = a list (params.small[2]), else the text
    let p = lua.create_table()?;
    for (k, v) in &c.params {
        let nums: Option<Vec<f64>> = v.split('/').map(|x| x.trim().trim_start_matches('+').parse::<f64>().ok()).collect();
        match nums {
            Some(n) if n.len() == 1 => p.set(k.as_str(), n[0])?,
            Some(n) => p.set(k.as_str(), n)?,
            None => p.set(k.as_str(), v.as_str())?,
        }
    }
    t.set("params", p)?;
    t.set("level", lua.create_function(move |_, ()| Ok(unsafe { chip::level(row) }.unwrap_or(0)))?)?;
    t.set("value", lua.create_function(move |_, ()| Ok(chip::value(row, unsafe { chip::level(row) }.unwrap_or(0))))?)?;
    env.set("chip", t)?;

    // store: the chip's own byte in the save (0..255; New Game starts at 0)
    let t = lua.create_table()?;
    let flag = o::SCRIPT_STORE + (c.id as u32 - 1) * o::SCRIPT_STORE_BITS;
    t.set("get", lua.create_function(move |_, ()| Ok(with_flags(|g, man| unsafe { g.get_bits(man, flag, o::SCRIPT_STORE_BITS) }).unwrap_or(0)))?)?;
    t.set("set", lua.create_function(move |_, v: i64| {
        let v = v.clamp(0, (1 << o::SCRIPT_STORE_BITS) - 1) as u32;
        with_flags(|g, man| unsafe { g.put_bits(man, flag, o::SCRIPT_STORE_BITS, v) });
        Ok(())
    })?)?;
    env.set("store", t)?;

    // game: reads
    let t = lua.create_table()?;
    t.set("ap", lua.create_function(|_, ()| Ok(player_ap().map(|x| x.0)))?)?;
    t.set("max_ap", lua.create_function(|_, ()| Ok(player_ap().map(|x| x.1)))?)?;
    t.set("coam", lua.create_function(|_, ()| Ok(game_data().map(|gd| unsafe { mem::rd_i32(gd + o::GD_COAM) })))?)?;
    t.set("in_mission", lua.create_function(|_, ()| Ok(game_data().is_some_and(|gd| unsafe { Game::new(mem::module_base()).defer_byte(gd) } != 0)))?)?;
    t.set("flag", lua.create_function(|_, id: u32| Ok(with_flags(|g, man| unsafe { g.get_flag(man, id) }).flatten()))?)?;
    t.set("tier_name", lua.create_function(|_, i: usize| Ok(config::get().enemy_tiers.get(i.wrapping_sub(1)).map(|t| t.0.clone())))?)?;
    t.set("level", lua.create_function(|_, target: mlua::Value| {
        let Some(r) = event::row_of(&target.to_string()?) else { return Ok(None) };
        Ok(game_data().and_then(|gd| unsafe { os::level(gd, r) }))
    })?)?;
    env.set("game", t)?;

    // actions (the declarative ones)
    env.set("credits", lua.create_function(move |_, n: i32| { act(c, event::Action::Credits(event::Amount::Fixed(n))); Ok(()) })?)?;
    env.set("give", lua.create_function(move |_, name: mlua::Value| { act(c, event::Action::Give(name.to_string()?)); Ok(()) })?)?;
    env.set("add_level", lua.create_function(move |_, (target, n): (mlua::Value, Option<i32>)| {
        act(c, event::Action::Level(target.to_string()?, event::Amount::Fixed(n.unwrap_or(1)))); Ok(())
    })?)?;
    env.set("set_flag", lua.create_function(move |_, (id, v): (u32, bool)| { act(c, event::Action::Flag(id, v)); Ok(()) })?)?;
    env.set("resupply", lua.create_function(move |_, pct: i32| { act(c, event::Action::Resupply(event::Amount::Fixed(pct))); Ok(()) })?)?;
    env.set("speffect", lua.create_function(move |_, id: i32| {
        let not = NOW.with(|n| n.get().2);
        logf!("script {}: speffect {}", c.name, id);
        event::push_speffect(id, not);
        Ok(())
    })?)?;

    if c.unsafe_api { env.set("unsafe", unsafe_table(lua)?)?; }
    Ok(env)
}

/// Runs a declarative action for the chip of the running call (its level and event).
fn act(c: &chip::Chip, a: event::Action) {
    let (lv, e, _) = NOW.with(|n| n.get());
    let Some(e) = e else { return };
    logf!("script {}: {:?}", c.name, a);
    unsafe { event::act(c, lv, &a, e) };
}

fn with_flags<R>(f: impl FnOnce(&Game, usize) -> R) -> Option<R> {
    let g = unsafe { Game::new(mem::module_base()) };
    let man = unsafe { g.flag_man() }?;
    Some(f(&g, man))
}
fn game_data() -> Option<usize> { unsafe { Game::new(mem::module_base()).game_data_fast() } }
fn player_ap() -> Option<(i32, i32)> { unsafe { Game::new(mem::module_base()).player_ap() }.map(|x| (x.0, x.1)) }

/// `unsafe`: raw memory of the game (addresses as integers; reads of unmapped memory give nil, writes there do nothing),
/// the exe base and the player character.
fn unsafe_table(lua: &Lua) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("base", mem::module_base() as i64)?;
    t.set("player", lua.create_function(|_, ()| {
        let base = mem::module_base();
        let wcm = unsafe { mem::rd_usize(base + o::WORLDCHRMAN_GLOBAL) };
        Ok(if wcm == 0 { 0 } else { (unsafe { mem::rd_usize(wcm + o::WCM_PLAYER) }) as i64 })
    })?)?;
    t.set("readable", lua.create_function(|_, (a, n): (i64, Option<i64>)| Ok(mem::is_readable(a as usize, n.unwrap_or(1).max(1) as usize)))?)?;
    macro_rules! rd { ($name:literal, $t:ty, $out:ty) => {
        t.set($name, lua.create_function(|_, a: i64| {
            let a = a as usize;
            Ok(if mem::is_readable(a, core::mem::size_of::<$t>()) { Some(unsafe { core::ptr::read_unaligned(a as *const $t) } as $out) } else { None })
        })?)?;
    } }
    macro_rules! wr { ($name:literal, $t:ty, $in:ty) => {
        t.set($name, lua.create_function(|_, (a, v): (i64, $in)| {
            let a = a as usize;
            if !mem::is_readable(a, core::mem::size_of::<$t>()) { return Ok(false); }
            unsafe { core::ptr::write_unaligned(a as *mut $t, v as $t) };
            Ok(true)
        })?)?;
    } }
    rd!("read_u8", u8, i64); rd!("read_u16", u16, i64); rd!("read_i32", i32, i64); rd!("read_u32", u32, i64);
    rd!("read_u64", u64, i64); rd!("read_f32", f32, f64);
    wr!("write_u8", u8, i64); wr!("write_u16", u16, i64); wr!("write_i32", i32, i64); wr!("write_u32", u32, i64);
    wr!("write_u64", u64, i64); wr!("write_f32", f32, f64);
    Ok(t)
}
