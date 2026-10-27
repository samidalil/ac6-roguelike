-- step20: 8-byte hardware write-watch on the PENDING obtained-items vector's END pointer (mgr+0x88, mgr = GameData+0x600).
-- Fires when a reward is queued (push_back) or committed/cleared -> backtrace = the add-to-pending function.
local gd = readQword(readQword(ac6.base + 0x4E03A08) + 8)
local mgr = gd + 0x600
local endp = mgr + 0x88
ac6.log("=== step20: pending-end watch @%X (begin=%X end=%X) ===", endp, readQword(mgr + 0x80), readQword(endp))
ac6.watchWrite(endp, 8, { name = "pendingEnd", backtrace = 16,
  onHit = function()
    local b, e = readQword(mgr + 0x80), readQword(endp)
    local last = e - 0x10
    ac6.log("  pending now %d bytes; last element id=%08X flags=%02X %02X", e - b, (e > b) and readInteger(last) or 0, (e > b) and readBytes(last + 8, 1) or 0, (e > b) and readBytes(last + 9, 1) or 0)
  end })
