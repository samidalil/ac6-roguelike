-- Who reads the RF-024 TURNER lineup row's price (+4) while the Vente (sell) menu is open?
dofile([[D:/samee/Projects/Desktop/ac6-roguelike/ac6_session.lua]])
ac6.clearTraps()
local addr = 0x7ff4f8c9c0c0 + 4
local seen = {}
debug_setBreakpoint(addr, 4, bptAccess, bpmDebugRegister, function()
  local k = RIP
  seen[k] = (seen[k] or 0) + 1
  if seen[k] <= 2 then
    ac6.log("READ price from %s (hit %d)", ac6.rel(RIP), seen[k])
    for _, l in ipairs(ac6.backtrace(8)) do ac6.log("    %s", l) end
  end
  debug_continueFromBreakpoint(co_run)
  return 1
end)
ac6.traps[addr] = "TURNER price read"
ac6.log("watch set on %X", addr)
return 1
