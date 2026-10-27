-- How do the save counters [[exe+4E140B0]+0xCCC] / +0xCD0 move during a save? (log every write with the mask)
dofile([[D:/samee/Projects/Desktop/ac6-roguelike/ac6_session.lua]])
ac6.clearTraps()
local st = readQword(ac6.base + 0x4E140B0)
for _, w in ipairs({ {0xCCC, "ctrCCC"}, {0xCD0, "ctrCD0"}, {0xCB4, "saveMask"} }) do
  local addr, name = st + w[1], w[2]
  debug_setBreakpoint(addr, 4, bptWrite, bpmDebugRegister, function()
    ac6.log("WRITE %s = %X from %s  (mask %X ccc %X cd0 %X)", name, readInteger(addr), ac6.rel(RIP), readInteger(st + 0xCB4), readInteger(st + 0xCCC), readInteger(st + 0xCD0))
    debug_continueFromBreakpoint(co_run); return 1
  end)
  ac6.traps[addr] = name
end
ac6.log("save counter watch armed")
return 1
