-- step19: self-re-arming inventory watch. Watches the NEXT FREE gaitem-table entry's id field; on every hit it logs a
-- 16-frame backtrace + the new id, then moves the watch to the new next-free entry. Survives log pickups mid-mission so
-- the mission REWARD grant (e.g. MIND ALPHA after mission 6) is always caught with its full vanilla call chain.
ac6.inv = ac6.inv or {}
function ac6.inv.table()
  local gd = readQword(readQword(ac6.base + 0x4E03A08) + 8)
  return readQword(gd + 0x600 + 0x10)
end
function ac6.inv.nextFree()
  local t = ac6.inv.table()
  for i = 0, 255 do
    local hd = readInteger(t + i * 16)
    if hd == nil or (hd & 0xFFFFFFFF) == 0xFFFFFFFF then return i, t + i * 16 end
  end
end
function ac6.inv.arm()
  local i, e = ac6.inv.nextFree()
  if ac6.inv.armed then debug_removeBreakpoint(ac6.inv.armed); ac6.traps[ac6.inv.armed] = nil end
  local target = e + 4  -- id field
  ac6.inv.armed = target
  ac6.watchWrite(target, 4, { name = string.format("invEntry%d", i), backtrace = 16,
    onHit = function()
      ac6.log("  >>> inventory entry %d written: handle=%08X id=%08X", i, readInteger(e), readInteger(e + 4))
      ac6.inv.rearm = true
    end })
  ac6.log("inventory watch armed on entry %d (id field %X)", i, target)
end
-- re-arm from a timer (can't modify breakpoints inside the breakpoint callback safely)
if ac6.inv.timer then ac6.inv.timer.destroy() end
ac6.inv.timer = createTimer(nil); ac6.inv.timer.Interval = 300
ac6.inv.timer.OnTimer = function()
  if ac6.inv.rearm then ac6.inv.rearm = false; ac6.inv.arm(); return end
  -- drift guard: if the next-free slot moved (entry freed/reshuffled), follow it
  local i, e = ac6.inv.nextFree()
  if e and ac6.inv.armed ~= e + 4 then ac6.log("inventory next-free moved -> re-arming on entry %d", i); ac6.inv.arm() end
end
ac6.inv.arm()
