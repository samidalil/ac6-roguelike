-- Nuée: after each core expansion, a laser drone every chip.params.interval seconds around your AC (a ring of
-- chip.params.radius m, chip.params.height m above) for the level's value in seconds.
function on_core_expansion(e)
  local p = chip.params
  local step = p.interval or 0.2
  local n = math.floor(chip.value() / step + 0.5)
  local r, h, k = p.radius or 4, p.height or 6, 0
  local function spawn()
    k = k + 1
    local x, y, z = game.player_pos()
    if x then
      local a = k * 2.39996   -- golden angle: spread around the ring
      drone(x + r * math.cos(a), y + h, z + r * math.sin(a))
    end
    if k < n then after(step, spawn) end
  end
  spawn()
end
