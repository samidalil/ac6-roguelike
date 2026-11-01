-- Écho plasma: the level's value of plasma cannon shots from the destroyed enemy's position (e.x, e.y, e.z) at what
-- you are locked on next. The game keeps the lock on a destroyed enemy for a moment (~0.7 s): the shots wait until the
-- lock has left it (checked every chip.params.step seconds, chip.params.wait seconds at most).
function on_enemy_destroyed(e)
  local p = chip.params
  local n, h = chip.value(), p.height or 3
  local function fire(k)
    local tx, ty, tz = game.lock_pos()
    if not tx then return end
    shoot(p.weapon, p.behavior, e.x, e.y + h, e.z, tx, ty + h, tz)
    if k < n then after(p.interval or 0.4, function() fire(k + 1) end) end
  end
  local step = p.step or 0.1
  local tries = math.floor((p.wait or 2) / step)
  local function try()
    if game.lock_id() ~= e.id or tries <= 0 then fire(1) else tries = tries - 1; after(step, try) end
  end
  try()
end
