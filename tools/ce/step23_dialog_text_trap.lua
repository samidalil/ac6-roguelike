-- Who builds the "return to title" Yes/No dialog? Trap the text-object maker exe+CAEAF0(out, msgId) and the two
-- known dialog builders; log the message id and a backtrace.
local B = ac6.base
ac6.clearTraps()
ac6.trap(B + 0xCAEAF0, { name = "TextObj CAEAF0", backtrace = 10, onHit = function() ac6.log("  msgId=%d", RDX) end })
ac6.trap(B + 0xD684A0, { name = "Dialog D684A0", backtrace = 8 })
ac6.trap(B + 0xD68430, { name = "Dialog D68430", backtrace = 8 })
return 1
