-- Backtrace of the lookup of the improper-exit warning text (category 203, id 400003).
dofile([[D:/samee/Projects/Desktop/ac6-roguelike/ac6_session.lua]])
ac6.clearTraps()
ac6.trap(ac6.base + 0x323FE20, { name = "msg lookup 400003", backtrace = 16, once = true,
  cond = function() return (R8 & 0xFFFFFFFF) == 203 and (R9 & 0xFFFFFFFF) == 400003 end })
return 1
