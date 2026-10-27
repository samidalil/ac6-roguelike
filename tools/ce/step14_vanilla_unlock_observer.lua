-- step14 v2: VANILLA unlock observer. Trap SetEventFlag (exe+B08500; this=RCX, id=RDX, state=R8b).
--  * unlock ranges (3xxx unlock params, 6xxx release waves): full log + 10-frame backtrace on EVERY call
--  * every other id: logged ONCE (first time seen) so the whole mission-clear flag sequence is visible without noise
ac6.log("=== step14 v2: vanilla unlock observer ===")
ac6.evf = ac6.evf or {}
ac6.evf.man = readQword(ac6.base + 0x4E13070)
ac6.log("CSEventFlagMan=%X", ac6.evf.man)
ac6.sef_total = 0
ac6.sef_seen = {}
local function unlockRange(id) return (id >= 3000 and id < 3300) or (id >= 6000 and id < 7000) end
ac6.trap(ac6.base + 0xB08500, { name = "SetEventFlag", backtrace = 10,
  cond = function()
    local id = RDX & 0xFFFFFFFF
    ac6.sef_total = ac6.sef_total + 1
    if unlockRange(id) then
      -- per-id cap: full log+backtrace only for the first 3 writes of an id (3000/6105 are per-frame housekeeping)
      ac6.sef_seen[id] = (ac6.sef_seen[id] or 0) + 1
      return ac6.sef_seen[id] <= 3
    end
    if not ac6.sef_seen[id] then
      ac6.sef_seen[id] = 1
      ac6.log("flag first-seen id=%d state=%d (call #%d)", id, R8 & 0xFF, ac6.sef_total)
    end
    return false
  end,
  onHit = function()
    local id = RDX & 0xFFFFFFFF
    ac6.log("UNLOCK SetEventFlag id=%d state=%d  (call #%d, this id x%d)", id, R8 & 0xFF, ac6.sef_total, ac6.sef_seen[id])
  end })
ac6.log("=== step14 v2 armed: play until the first shop unlock (fresh save: the intro missions) ===")
