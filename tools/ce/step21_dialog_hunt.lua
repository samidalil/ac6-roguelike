-- step21: find how the "Pièces obtenues" dialog is opened, and whether its script step runs on every mission end.
--  (a) one-shot trap on the commit core 4756E0 -> fires at ANY mission end that runs the "obtained parts" step (dl=1
--      commit / dl=0 clear); backtrace = the script step.
--  (b) chained: when the pending vector's END moves (a reward was queued), arm a short-lived trap on the accessor
--      46E180 (max 4 hits) -> the next readers after the push = the dialog builder; their backtrace leads to the
--      menu-open function.
ac6.pend = ac6.pend or {}
ac6.trap(ac6.base + 0x4756E0, { name = "commitCore", once = true, backtrace = 14,
  onHit = function() ac6.log("  commit core: dl(flag)=%d  mgr=%X", RDX & 0xFF, RCX) end })
local gd = readQword(readQword(ac6.base + 0x4E03A08) + 8)
local mgr = gd + 0x600
ac6.pend.hits = 0
function ac6.pend.armAccessor()
  ac6.trap(ac6.base + 0x46E180, { name = "accessorAfterPush", backtrace = 14,
    cond = function() ac6.pend.hits = ac6.pend.hits + 1; return ac6.pend.hits <= 4 end,
    onHit = function()
      ac6.log("  accessor hit #%d after push (caller chain above)", ac6.pend.hits)
      if ac6.pend.hits >= 4 then ac6.pend.removeAcc = true end
    end })
end
-- re-define the pendingEnd watch so a push arms (b) via a timer (can't set breakpoints inside a bp callback)
ac6.clearTraps()
ac6.trap(ac6.base + 0x4756E0, { name = "commitCore", once = true, backtrace = 14,
  onHit = function() ac6.log("  commit core: dl(flag)=%d  mgr=%X", RDX & 0xFF, RCX) end })
ac6.watchWrite(mgr + 0x88, 8, { name = "pendingEnd", backtrace = 16,
  onHit = function()
    local b, e = readQword(mgr + 0x80), readQword(mgr + 0x88)
    ac6.log("  pending now %d bytes; last id=%08X", e - b, (e > b) and readInteger(e - 0x10) or 0)
    if e > b then ac6.pend.armAcc = true end
  end })
if ac6.pend.timer then ac6.pend.timer.destroy() end
ac6.pend.timer = createTimer(nil); ac6.pend.timer.Interval = 200
ac6.pend.timer.OnTimer = function()
  if ac6.pend.armAcc then ac6.pend.armAcc = false; ac6.pend.hits = 0; ac6.pend.armAccessor(); ac6.log("accessor trap armed (dialog hunt)") end
  if ac6.pend.removeAcc then ac6.pend.removeAcc = false; debug_removeBreakpoint(ac6.base + 0x46E180); ac6.traps[ac6.base + 0x46E180] = nil; ac6.log("accessor trap removed") end
end
ac6.log("=== step21 armed: commit-core once-trap + pending-end watch (chains an accessor trap on push) ===")
