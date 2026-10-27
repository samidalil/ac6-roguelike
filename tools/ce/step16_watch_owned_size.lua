-- step16: 8-byte hardware WRITE-watch on the owned-parts std::set size field (CSEquipmentLineupData+0xA0+0x28).
-- Fires exactly when a part is granted/removed -> backtrace = the grant function.
local a = 0x7FF46EF414C8
ac6.log("=== step16: write-watch on owned-set size @%X (now %d) ===", a, readQword(a))
ac6.watchWrite(a, 8, { name = "ownedSetSize", backtrace = 14,
  onHit = function() ac6.log("  owned-set size now = %d", readQword(a)) end })
