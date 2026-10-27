-- step18: watches on the next free inventory slot (entry 19; entry 18 = purchased VP-60LCS-L) to catch a MISSION/LOGHUNT
-- reward grant and compare its call chain with the purchase path (exe+473EE0 wrapper).
ac6.log("=== step18: watches on entry19 id @7FF46F6AA534 + list slot19 @7FF46F6B20B8 ===")
ac6.watchWrite(0x7FF46F6AA534, 4, { name = "gaitemEntry19", backtrace = 16,
  onHit = function() ac6.log("  entry19: handle=%08X id=%08X", readInteger(0x7FF46F6AA530), readInteger(0x7FF46F6AA534)) end })
ac6.watchWrite(0x7FF46F6B20B8, 4, { name = "listSlot19", backtrace = 16,
  onHit = function() ac6.log("  slot19: %08X", readInteger(0x7FF46F6B20B8)) end })
