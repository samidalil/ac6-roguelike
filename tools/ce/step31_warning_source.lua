-- Who builds the improper-exit warning (message 400003 = 0x61A83)? Trap the text-object makers with that id.
dofile([[D:/samee/Projects/Desktop/ac6-roguelike/ac6_session.lua]])
ac6.clearTraps()
local B = ac6.base
for _, a in ipairs({0xCAEAF0, 0xDD5630}) do
  ac6.trap(B + a, { name = string.format("textobj %X", a), backtrace = 14,
    cond = function() return (RDX & 0xFFFFFFFF) == 0x61A83 end,
    onHit = function() ac6.log("  msg %d", RDX & 0xFFFFFFFF) end })
end
return 1
