-- Essaim: the level's value of laser drones at the destroyed enemy's position (e.x, e.y, e.z), slightly apart and
-- a little above it.
function on_enemy_destroyed(e)
  local n = chip.value()
  for i = 1, n do
    local a = (i - 1) * 2 * math.pi / n
    drone(e.x + 2 * math.cos(a), e.y + 3, e.z + 2 * math.sin(a))
  end
end
