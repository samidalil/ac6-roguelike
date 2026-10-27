-- Who reads the session byte / GameMan exit bytes when the title checks for an improper exit?
dofile([[D:/samee/Projects/Desktop/ac6-roguelike/ac6_session.lua]])
ac6.clearTraps()
local st = readQword(ac6.base + 0x4E140B0)
local gm = readQword(ac6.base + 0x4E29CC0)
for _, w in ipairs({ {st + 0xCB8, "sessionOpen"}, {gm + 0x3FAE, "gm3FAE"}, {gm + 0x3FA8, "gm3FA8"} }) do
  local addr, name = w[1], w[2]
  local seen = {}
  debug_setBreakpoint(addr, 1, bptAccess, bpmDebugRegister, function()
    local k = RIP
    seen[k] = (seen[k] or 0) + 1
    if seen[k] <= 2 then
      ac6.log("ACCESS %s = %X at %s", name, readBytes(addr, 1), ac6.rel(RIP))
      for _, l in ipairs(ac6.backtrace(6)) do ac6.log("    %s", l) end
    end
    debug_continueFromBreakpoint(co_run); return 1
  end)
  ac6.traps[addr] = name
end
ac6.log("session check watch armed")
return 1
