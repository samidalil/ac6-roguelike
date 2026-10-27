-- step15: hardware WRITE-watch (1 byte) on the byte holding event flags 6200..6207. Fires only when that byte is
-- written (rare), logging RIP + a 12-frame backtrace = the call chain that releases the first parts wave.
local b = 0x7FF46F696E67
ac6.log("=== step15: write-watch armed on flag-6200 byte %X (current value %02X) ===", b, readBytes(b,1))
ac6.watchWrite(b, 1, { name = "flag6200byte", backtrace = 12,
  onHit = function() ac6.log("  flag6200 byte now=%02X  (bit 0x80 = 6200)", readBytes(b, 1)) end })
