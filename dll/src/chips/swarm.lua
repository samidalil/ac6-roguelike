-- Essaim: the level's value of laser drones at the destroyed enemy's position (e.x, e.y, e.z), slightly apart and
-- a little above it, after chip.params.delay seconds (a drone goes for what you are locked on when it appears: right
-- at the kill, that is still the enemy just destroyed).
function on_enemy_destroyed(e)
  local n = chip.value()
  local delay = chip.params.delay or 0
  for i = 1, n do
    local a = (i - 1) * 2 * math.pi / n
    drone(e.x + 2 * math.cos(a), e.y + 3, e.z + 2 * math.sin(a), delay)
  end
end
