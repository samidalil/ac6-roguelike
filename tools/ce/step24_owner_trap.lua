-- Capture the owner passed to the Yes/No dialog builder exe+D67220(out, owner, text, params) and the caller's `this`.
local B = ac6.base
local function q(a) return (a and a ~= 0) and readQword(a) or 0 end
ac6.clearTraps()
ac6.trap(B + 0xD4ECE0, { name = "SysMenu ReturnToTitle D4ECE0", onHit = function()
  ac6.log("  this rcx=%X vt=%s   owner rdx=%X vt=%s", RCX, ac6.rel(q(RCX)), RDX, ac6.rel(q(RDX)))
  for i = 0, 0x38, 8 do ac6.log("    owner+%X = %X", i, q(RDX + i)) end
end })
ac6.trap(B + 0xD67220, { name = "YesNoDialog D67220", backtrace = 6, onHit = function()
  ac6.log("  out=%X owner=%X vt=%s text=%X params=%X", RCX, RDX, ac6.rel(q(RDX)), R8, R9)
end })
return 1
