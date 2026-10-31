-- Pillage: the % of [params] for the destroyed enemy's tier (e.tier, named in [enemy_tiers]) at the chip's level.
function on_enemy_destroyed(e)
  local pct = chip.params[game.tier_name(e.tier) or ""]
  if type(pct) == "number" then pct = { pct } end
  if pct then resupply(pct[math.min(e.level, #pct)]) end
end
