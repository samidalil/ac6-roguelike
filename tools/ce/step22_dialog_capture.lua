-- Capture the arguments of the text-dialog builder exe+D68430(ctx, owner, text) when the game opens a dialog.
dofile([[D:/samee/Projects/Desktop/ac6-roguelike/ac6_session.lua]])
local B = ac6.base
local function q(a) return (a and a ~= 0) and readQword(a) or 0 end
ac6.clearTraps()
ac6.trap(B + 0xD68430, { name = "TextDialog D68430", backtrace = 8, onHit = function()
  ac6.log("  rcx=%X [rcx]=%s  rdx=%X [rdx]=%s", RCX, ac6.rel(q(RCX)), RDX, ac6.rel(q(RDX)))
  local t = R8
  ac6.log("  r8=%X : %X %X %X %X", t, q(t), q(t+8), q(t+16), q(t+24))
  local s = readString(q(t + 8), 200, true) or readString(q(t), 200, true) or "?"
  ac6.log("  text? %s", s)
end })
return 1
