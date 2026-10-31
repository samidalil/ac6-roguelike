-- Prime de série: every 3rd mission cleared in a row pays the level's value in COAM; a failed mission resets the
-- streak. The streak is the chip's saved byte (store), kept in the save with the run.
--
-- Script API (each chip runs in its own environment):
--   on_<event>(e)   called on an event (the event names of performance_bonus.toml, plus enemy_destroyed, enemy_staggered, core_expansion) while the
--                   chip is at level 1+; e.level, e.value (the level's value), and the event's own fields
--                   (enemy_destroyed, enemy_staggered: e.max_ap, e.tier = its [enemy_tiers] tier, 1 = the first;
--                   enemy_destroyed also e.x / e.y / e.z its position)
--   log(...)        a line in the DLL log
--   chip            .id, .name, .values, .params ([params] of the chip file: a number, "a/b/c" = a list, else
--                   the text), level(), value()
--   store           get() / set(v): one byte (0..255) saved with the run
--   game            ap(), max_ap(), coam(), in_mission(), flag(id), level(row or name), tier_name(n)
--   actions         credits(n), give("part name"), speffect(row), resupply(%), energy(%), drone(x, y, z[, delay s]) (a laser drone of
--                   yours at that world position), add_level(row or name, n),
--                   set_flag(id, bool)
--   unsafe          (only with unsafe = true in the chip file) base, player(), readable(addr, n), read_u8/u16/i32/
--                   u32/u64/f32(addr), write_u8/u16/i32/u32/u64/f32(addr, v): raw game memory, can break the game

local EVERY = 3

function on_mission_cleared(e)
  local n = store.get() + 1
  if n >= EVERY then
    credits(e.value)
    n = 0
  end
  store.set(n)
  log("streak " .. n .. "/" .. EVERY)
end

function on_mission_failed(e)
  store.set(0)
  log("streak reset")
end
