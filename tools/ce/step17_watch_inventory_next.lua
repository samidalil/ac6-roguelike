-- step17: 4-byte hardware WRITE-watches on the NEXT free inventory slots (serial 0x17F):
--   gaitem table entry 18 @ 7FF46F6AA524 (handle field) and compact owned-list slot @ 7FF46F6B20B0.
-- Fires exactly when the next item is granted -> backtrace = the "add gaitem to inventory" function.
ac6.log("=== step17: inventory next-slot watches (table entry18 %X, list slot %X) ===", 0x7FF46F6AA524, 0x7FF46F6B20B0)
ac6.watchWrite(0x7FF46F6AA524, 4, { name = "gaitemTableNext", backtrace = 14,
  onHit = function() ac6.log("  table entry18: handle=%08X id=%08X", readInteger(0x7FF46F6AA524), readInteger(0x7FF46F6AA528)) end })
ac6.watchWrite(0x7FF46F6B20B0, 4, { name = "ownedListNext", backtrace = 14,
  onHit = function() ac6.log("  list slot: %08X", readInteger(0x7FF46F6B20B0)) end })
