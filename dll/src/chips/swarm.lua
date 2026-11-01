-- Essaim: the level's value of laser drones at the destroyed enemy's position (e.x, e.y, e.z), slightly apart and
-- a little above it. A drone goes for what you are locked on when it appears, and the game keeps the lock on a
-- destroyed enemy for a moment (~0.7 s): the drones wait until the lock has left it (checked every chip.params.step
-- seconds, chip.params.wait seconds at most).
function on_enemy_destroyed(e)
  local n = chip.value()
  local function spawn()
    for i = 1, n do
      local a = (i - 1) * 2 * math.pi / n
      drone(e.x + 2 * math.cos(a), e.y + 3, e.z + 2 * math.sin(a))
    end
  end
  local step = chip.params.step or 0.1
  local tries = math.floor((chip.params.wait or 2) / step)
  local function try()
    if game.lock_id() ~= e.id or tries <= 0 then spawn() else tries = tries - 1; after(step, try) end
  end
  try()
end
