-- Who writes the "session open" byte [[exe+4E140B0]+0xCB8] and the save request mask (+0xCB4)?
dofile([[D:/samee/Projects/Desktop/ac6-roguelike/ac6_session.lua]])
ac6.clearTraps()
local st = readQword(ac6.base + 0x4E140B0)
for _, w in ipairs({ {0xCB8, 1, "sessionOpen"}, {0xCB4, 4, "saveMask"} }) do
  local addr, size, name = st + w[1], w[2], w[3]
  local n = 0
  debug_setBreakpoint(addr, size, bptWrite, bpmDebugRegister, function()
    n = n + 1
    if n <= 12 then
      ac6.log("WRITE %s = %X from %s", name, size == 1 and readBytes(addr, 1) or readInteger(addr), ac6.rel(RIP))
      for _, l in ipairs(ac6.backtrace(6)) do ac6.log("    %s", l) end
    end
    debug_continueFromBreakpoint(co_run); return 1
  end)
  ac6.traps[addr] = name
end
ac6.log("session watch armed (save state %X)", st)
return 1
