-- Count calls to the item price getter exe+B885D0 by caller (return address [rsp]); dump with ac6.dumpPriceCallers().
ac6.clearTraps()
ac6.priceCallers = {}
local B = ac6.base
debug_setBreakpoint(B + 0xB885D0, 1, bptExecute, bpmDebugRegister, function()
  local ret = readQword(RSP)
  local k = ac6.rel(ret)
  ac6.priceCallers[k] = (ac6.priceCallers[k] or 0) + 1
  if ac6.priceCallers[k] == 1 then
    ac6.log("price getter first call from %s", k)
    for _, l in ipairs(ac6.backtrace(6)) do ac6.log("    %s", l) end
  end
  debug_continueFromBreakpoint(co_run); return 1
end)
ac6.traps[B + 0xB885D0] = "price getter"
function ac6.dumpPriceCallers(tag)
  for k, v in pairs(ac6.priceCallers) do ac6.log("[%s] %s x%d", tag or "", k, v) end
  ac6.priceCallers = {}
end
ac6.log("price getter trap armed")
return 1
