-- Call tracer that de-obfuscates by execution.
-- ac6.traceCalls(startAddr, phase1Stop, opts):
--   when execution reaches startAddr (a `call`), step INTO it; then single-step, logging every `call`
--   (address, operand, rcx/rdx/r8/r9) and stepping OVER it; every other instruction (obfuscated jump chains,
--   push/ret trampolines, cmov-selected returns) is simply followed.
--   Phase 1 ends when RIP == phase1Stop (the call's return address). If opts.phase2 is set, tracing continues
--   in the caller until its next `ret` executes, so we also see what the caller does with the result.
-- Output goes to ac6.log (ac6_re_log.txt). Steps are capped (opts.max, default 400000).

ac6.tr = ac6.tr or {}

local function opcodeAt(addr)
  local d = disassemble(addr)
  local extra, opcode = splitDisassembledString(d)
  return opcode or ""
end

function ac6.traceCalls(startAddr, phase1Stop, opts)
  opts = opts or {}
  local tr = { start = startAddr, stop1 = phase1Stop, phase2 = opts.phase2, max = opts.max or 400000,
               steps = 0, active = false, phase = 1, ncalls = 0, logJumps = opts.logJumps }
  ac6.tr = tr
  debug_setBreakpoint(startAddr, 1, bptExecute, bpmDebugRegister, function()
    debug_removeBreakpoint(startAddr)
    ac6.traps[startAddr] = nil
    tr.active = true; tr.t0 = os.clock()
    ac6.log("TRACE start at %s (%s)  rcx=%X rdx=%X r8=%X r9=%X", ac6.rel(RIP), opcodeAt(RIP), RCX, RDX, R8, R9)
    debug_continueFromBreakpoint(co_stepinto)
    return 1
  end)
  ac6.traps[startAddr] = "traceStart"
  ac6.log("tracer armed at %s, phase 1 stops at %s%s", ac6.rel(startAddr), ac6.rel(phase1Stop), opts.phase2 and " (+phase 2 to caller's ret)" or "")
end

function debugger_onBreakpoint()
  local tr = ac6.tr
  if not (tr and tr.active) then return 0 end
  tr.steps = tr.steps + 1
  local rip = RIP
  if tr.steps >= tr.max then
    tr.active = false
    ac6.log("TRACE ABORTED at %s after %d steps (cap)", ac6.rel(rip), tr.steps)
    debug_continueFromBreakpoint(co_run); return 1
  end
  if tr.phase == 1 and rip == tr.stop1 then
    ac6.log("TRACE phase 1 done at %s: %d steps, %d calls, %.1fs  rax=%X", ac6.rel(rip), tr.steps, tr.ncalls, os.clock() - tr.t0, RAX)
    if not tr.phase2 then
      tr.active = false; debug_continueFromBreakpoint(co_run); return 1
    end
    tr.phase = 2
  end
  local op = opcodeAt(rip)
  if op:match("^call") then
    tr.ncalls = tr.ncalls + 1
    ac6.log("  [%d] CALL @%s  %s   rcx=%X rdx=%X r8=%X r9=%X", tr.phase, ac6.rel(rip), op, RCX, RDX, R8, R9)
    debug_continueFromBreakpoint(co_stepover)
    return 1
  end
  if tr.phase == 2 and op:match("^ret") then
    ac6.log("TRACE phase 2 done: caller returns at %s (rax=%X)", ac6.rel(rip), RAX)
    tr.active = false; debug_continueFromBreakpoint(co_run); return 1
  end
  if tr.logJumps and op:match("^jmp") and not op:match("^jmp %a+ ptr") then
    -- only log far jumps (obfuscation hops) to keep the log readable
  end
  debug_continueFromBreakpoint(co_stepinto)
  return 1
end
