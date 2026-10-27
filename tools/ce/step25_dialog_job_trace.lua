-- Experimental standalone dialog: watch the job builder, the dialog constructor and the fatal-error reporter.
dofile([[D:/samee/Projects/Desktop/ac6-roguelike/ac6_session.lua]])
local B = ac6.base
ac6.clearTraps()
ac6.trap(B + 0xDBE570, { name = "DialogJob args reader DBE570", backtrace = 8, onHit = function()
  ac6.log("  rcx(&args)=%X rdx(out)=%X r8(owner)=%X", RCX, RDX, R8)
end })
ac6.trap(B + 0xD67470, { name = "Dialog ctor D67470", onHit = function()
  ac6.log("  out=%X owner=%X arg1=%d params=%X", RCX, RDX, R8 & 0xFFFFFFFF, R9)
end })
ac6.trap(B + 0x29213B0, { name = "FATAL 29213B0", backtrace = 12, onHit = function()
  ac6.log("  ecx=%X edx=%X r8=%X (%s)", RCX, RDX, R8, readString(R8, 80) or "?")
end })
return 1
