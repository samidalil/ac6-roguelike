-- Récupération: each enemy destroyed repairs the level's value in AP (rows 900000091..093 in salvage.toml).
-- e.enemy = the destroyed character's NPC param id.

function on_enemy_destroyed(e)
  speffect(900000090 + e.level)
end
