-- Dernier recours: the level's row (900000101..103 in last_stand.toml) on the mission's AC; the game itself turns it on
-- below 30 % AP (conditionHp) and off above.

function on_mission_start(e)
  speffect(900000100 + e.level)
end
