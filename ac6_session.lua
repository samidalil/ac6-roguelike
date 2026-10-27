-- ac6_session.lua — Cheat Engine 7.5 helper for the AC6 roguelike RE work
--
-- Load order after a game restart:
--   1. launch via me3 (disable_arxan), attach CE, load AC6_TGA_v0.4.4.CT, tick "Open"
--   2. Memory View -> Ctrl+L (Lua Engine) -> run:   dofile([[C:\path\to\ac6_session.lua]])
--   3. ac6.verify()            -- checks the code addresses still match 1.09
--   4. ac6.captureShopList()   -- then open the Parts Shop once; the list pointer is captured
--   5. ac6.shop.dump(5)        -- sanity check
--
-- Everything logged goes to %USERPROFILE%\ac6_re_log.txt as well as the Lua console.
-- Untested on your machine as-is: CE's Lua API has small signature differences across
-- versions, so expect a one-line fix here or there.

ac6 = ac6 or {}

----------------------------------------------------------------------------- logging
local LOG = os.getenv("USERPROFILE") .. "\\ac6_re_log.txt"
function ac6.log(fmt, ...)
  local s = string.format(fmt, ...)
  print(s)
  local f = io.open(LOG, "a")
  if f then f:write(os.date("%Y-%m-%d %H:%M:%S  ") .. s .. "\n"); f:close() end
end
local log = ac6.log

----------------------------------------------------------------------------- module info
ac6.moduleName = "armoredcore6.exe"
ac6.base = getAddress(ac6.moduleName)              -- observed 0x140000000 (no ASLR)
ac6.size = getModuleSize(ac6.moduleName)
function ac6.inModule(a) return a >= ac6.base and a < ac6.base + ac6.size end
function ac6.rel(a)                                   -- pretty "exe+XXXX"
  if ac6.inModule(a) then return string.format("exe+%X", a - ac6.base) end
  return string.format("%X", a)
end

----------------------------------------------------------------------------- known code (1.09)
-- addr = absolute (base 0x140000000); bytes = expected first bytes, used by ac6.verify()
ac6.code = {
  lineupCtorFlagRead = { addr = 0x1404352D6, bytes = "8B 40 08 83 F8 FF",       what = "CSEquipmentLineupData ctor reads eventFlag_forRelease" },
  priceGetterRead    = { addr = 0x140435B86, bytes = "8B 40 04 48 83 C4 48 C3", what = "lineup price getter (mov eax,[rax+4])" },
  shopListHandleRet  = { addr = 0x140E87FDE, bytes = "4C 8D 44 24 30",          what = "after +E88180: RAX = shop list handle" },
  perFrameLoop       = { addr = 0x140E928F0, bytes = nil,                       what = "per-frame filter loop over shop list" },
  newBadgePredicate  = { addr = 0x140E89820, bytes = "48 83 EC 38",             what = "NEW-badge predicate (returns id NOT in GameData+0x230 set)" },
  entryCopyIdWrite   = { addr = 0x145F09B0B, bytes = "89 47 54",                what = "entry copy: [rdi+54] = [rbx+54] (Arxan region)" },
  entryCopyStateWrite= { addr = 0x145F09B25, bytes = "89 47 70",                what = "entry copy: [rdi+70] = [rbx+70]" },
}

local function parseHex(s)
  local t = {}
  for b in s:gmatch("%x%x") do t[#t + 1] = tonumber(b, 16) end
  return t
end

function ac6.verify()
  local ok = true
  for name, c in pairs(ac6.code) do
    if c.bytes then
      local want = parseHex(c.bytes)
      local got = readBytes(c.addr, #want, true) or {}
      local match = true
      for i = 1, #want do if got[i] ~= want[i] then match = false end end
      log("%-20s %s  %s", name, ac6.rel(c.addr), match and "OK" or "MISMATCH (game patched? re-find this one)")
      ok = ok and match
    end
  end
  return ok
end

----------------------------------------------------------------------------- params
ac6.rowIds = { melanderHead = nil, finderEye = 112, ransetsuR = 184, ransetsuL = 185, needleR = 371, needleL = 372 }
ac6.lineupRow = { equipId = 0x00, price = 0x04, flag = 0x08, equipType = 0x0C, isTrophy = 0x0F, rewardCondition = 0x10 }

-- TGA-free row lookup: AOB-scan the live SoloParamRepository copy for its "EQUIPMENT_LINEUP_PARAM" type string
-- (data = string - 318*0x20; 64-bit PARAM header (0x40) + id table (318 * 0x18: id u32, pad, dataOff u64, nameOff u64)
-- precede the data).  Verified on 1.09: header at data-0x1E10, rowCount u16 at header+0xA.
ac6.lineup = ac6.lineup or {}
function ac6.locateLineup()
  local res = AOBScan("45 51 55 49 50 4D 45 4E 54 5F 4C 49 4E 45 55 50 5F 50 41 52 41 4D 00", "+W-X")
  if not res then return nil end
  local found
  for i = 0, res.Count - 1 do
    local a = tonumber(res[i], 16)
    if a and not ac6.inModule(a) then
      local data = a - 318 * 0x20
      local hdr = data - 0x40 - 318 * 0x18
      if readSmallInteger(hdr + 0xA) == 318 then
        local rows = {}
        for k = 0, 317 do
          local e = hdr + 0x40 + k * 0x18
          rows[readInteger(e)] = hdr + readQword(e + 8)
        end
        found = { header = hdr, data = data, typeString = a, rows = rows }
        break
      end
    end
  end
  res.destroy()
  if found then
    ac6.lineup = found
    log("lineup param: header=%X data=%X (318 rows, id table parsed)", found.header, found.data)
  else
    log("lineup param: not found (is the game past the title screen?)")
  end
  return found
end

function ac6.row(id)
  if paramUtils then return paramUtils:getIdAddressInParam("EquipmentLineupParam", id) end
  if not ac6.lineup.rows then ac6.locateLineup() end
  return ac6.lineup.rows and ac6.lineup.rows[id]
end

function ac6.rowInfo(id)
  local a = ac6.row(id)
  return {
    addr = a, equipId = readInteger(a), price = readInteger(a + 4),
    flag = readInteger(a + 8), equipType = readBytes(a + 0xC, 1),
    rewardCondition = readInteger(a + 0x10),
  }
end

function ac6.printRow(id)
  local r = ac6.rowInfo(id)
  log("row %d @%X  equipId=%d price=%d flag=%d type=%d reward=%d",
      id, r.addr, r.equipId, r.price, r.flag, r.equipType, r.rewardCondition)
end

function ac6.setPrice(id, v) writeInteger(ac6.row(id) + 4, v); log("row %d price -> %d", id, v) end
function ac6.setFlag(id, v)  writeInteger(ac6.row(id) + 8, v); log("row %d flag  -> %d", id, v) end

-- add price+flag of the usual suspects to the address list (survives restart: re-run after Open)
function ac6.addRows(ids)
  ids = ids or { 184, 185, 112, 371, 372 }
  local al = getAddressList()
  for _, id in ipairs(ids) do
    local a = ac6.row(id)
    for _, f in ipairs({ { 4, "price" }, { 8, "flag" } }) do
      local e = al.createMemoryRecord()
      e.Address = string.format("%X", a + f[1])
      e.Type = vtDword
      e.Description = string.format("EquipmentLineup row %d %s", id, f[2])
    end
  end
end

----------------------------------------------------------------------------- GameData
-- TGA "Open" registers the GameDataMan symbol.
function ac6.gameDataMan()      return readQword(getAddress("GameDataMan")) end
function ac6.gameData()         return readQword(ac6.gameDataMan() + 0x08) end   -- player GameData
function ac6.lineupData()       return readQword(ac6.gameDataMan() + 0x40) end   -- CSEquipmentLineupData
function ac6.seenSetContainer() return ac6.gameData() + 0x230 end                 -- used by NEW-badge predicate

----------------------------------------------------------------------------- registers / stack helpers
local REGS = { "RAX", "RBX", "RCX", "RDX", "RSI", "RDI", "RBP", "RSP", "R8", "R9", "R10", "R11", "R12", "R13", "R14", "R15", "RIP" }

function ac6.regs()
  local t = {}
  for _, r in ipairs(REGS) do t[r] = _G[r] end
  return t
end

function ac6.regsString()
  local parts = {}
  for _, r in ipairs(REGS) do parts[#parts + 1] = string.format("%s=%X", r, _G[r] or 0) end
  return table.concat(parts, " ")
end

-- heuristic backtrace: qwords on the stack that point into the module right after a CALL
function ac6.backtrace(maxFrames, depthBytes)
  maxFrames = maxFrames or 12
  depthBytes = depthBytes or 0x1000
  local out = {}
  local sp = RSP
  for off = 0, depthBytes, 8 do
    local v = readQword(sp + off)
    if v and ac6.inModule(v) then
      local prev = readBytes(v - 5, 5, true) or {}
      local prev2 = readBytes(v - 2, 2, true) or {}
      local isCall = (prev[1] == 0xE8) or (prev2[1] == 0xFF) or ((readBytes(v - 6, 1, true) or {})[1] == 0xFF)
      if isCall then
        out[#out + 1] = string.format("[rsp+%X] %s", off, ac6.rel(v))
        if #out >= maxFrames then break end
      end
    end
  end
  return out
end

----------------------------------------------------------------------------- traps (auto-logging breakpoints)
-- ac6.trap(addr, { cond = function() return RDI == x end, once = true, backtrace = 10, name = "..." , pause = false })
-- The callback runs inside the debugger; returning 1 resumes the game, 0 leaves it paused for manual inspection.
ac6.traps = {}

function ac6.trap(addr, opts)
  opts = opts or {}
  local name = opts.name or ac6.rel(addr)
  local declined = 0
  local function onHit()
    if opts.cond and not opts.cond() then
      -- CE 7.5 breaks on the hit even when this callback returns 1; force the continue explicitly.
      declined = declined + 1
      if declined == 1 or declined % 1000 == 0 then
        log("trap %s: %d declined hits (cond false), last RBX=%X RDI=%X", name, declined, RBX or 0, RDI or 0)
      end
      debug_continueFromBreakpoint(co_run)
      return 1
    end
    log("TRAP %s  %s", name, ac6.regsString())
    if opts.backtrace then
      for _, l in ipairs(ac6.backtrace(opts.backtrace)) do log("    %s", l) end
    end
    if opts.onHit then opts.onHit() end
    if opts.once then debug_removeBreakpoint(addr); ac6.traps[addr] = nil; log("TRAP %s removed (once)", name) end
    if opts.pause then return 0 end
    debug_continueFromBreakpoint(co_run)
    return 1
  end
  debug_setBreakpoint(addr, 1, bptExecute, bpmDebugRegister, onHit)
  ac6.traps[addr] = name
  log("trap set: %s @ %s", name, ac6.rel(addr))
end

-- hardware write watch with logging (max 4 hardware breakpoints total)
function ac6.watchWrite(addr, size, opts)
  opts = opts or {}
  size = size or 4
  local name = opts.name or string.format("write@%X", addr)
  local function onHit()
    log("WRITE %s  from %s  %s", name, ac6.rel(RIP), ac6.regsString())
    if opts.backtrace then for _, l in ipairs(ac6.backtrace(opts.backtrace)) do log("    %s", l) end end
    if opts.onHit then opts.onHit() end
    debug_continueFromBreakpoint(co_run)   -- see ac6.trap
    return 1
  end
  debug_setBreakpoint(addr, size, bptWrite, bpmDebugRegister, onHit)
  ac6.traps[addr] = name
  log("watch set: %s", name)
end

function ac6.clearTraps()
  for addr, name in pairs(ac6.traps) do debug_removeBreakpoint(addr); log("removed %s", name) end
  ac6.traps = {}
end

----------------------------------------------------------------------------- shop list
ac6.shop = ac6.shop or {}
ac6.shop.entrySize = 0x118
ac6.shop.fields = { price = 0x20, sharedPtr = 0x28, f50 = 0x50, gaitemId = 0x54, ordinal = 0x58, rowPtr = 0x60,
                    s68 = 0x68, s6C = 0x6C, s70 = 0x70, b74 = 0x74 }

-- one-shot trap after the +E88180 call: RAX = stack handle whose [0] is the persistent container
function ac6.captureShopList()
  ac6.trap(ac6.code.shopListHandleRet.addr, {
    name = "shopListHandle", once = true,
    onHit = function()
      local handle = RAX
      local container = readQword(handle)
      local s = ac6.shop
      s.container = container
      s.begin = readQword(container + 0x18)
      s.stop  = readQword(container + 0x20)
      s.cap   = readQword(container + 0x28)
      s.count = (s.stop - s.begin) // s.entrySize
      log("shop list: container=%X begin=%X end=%X count=%d", container, s.begin, s.stop, s.count)
    end,
  })
  log("open the Parts Shop now (the trap fires within a frame)")
end

function ac6.shop.entry(i)                 -- 0-based
  return ac6.shop.begin + i * ac6.shop.entrySize
end

function ac6.shop.read(i)
  local e = ac6.shop.entry(i)
  local F = ac6.shop.fields
  local id = readInteger(e + F.gaitemId)
  return {
    base = e, gaitemId = id, equipId = id & 0x0FFFFFFF, category = id >> 28,
    price = readInteger(e + F.price), ordinal = readInteger(e + F.ordinal),
    rowPtr = readQword(e + F.rowPtr),
    s68 = readInteger(e + F.s68), s6C = readInteger(e + F.s6C), s70 = readInteger(e + F.s70),
    b74 = (readBytes(e + F.b74, 1, true) or { -1 })[1],
  }
end

function ac6.shop.dump(n, from)
  from = from or 0
  n = n or 10
  for i = from, math.min(from + n - 1, ac6.shop.count - 1) do
    local r = ac6.shop.read(i)
    log("#%3d @%X  equip=%-9d cat=%d price=%-7d ord=%-4d row=%X  s68=%d s6C=%d s70=%d b74=%d",
        i, r.base, r.equipId, r.category, r.price, r.ordinal, r.rowPtr, r.s68, r.s6C, r.s70, r.b74)
  end
end

function ac6.shop.find(equipId)
  for i = 0, ac6.shop.count - 1 do
    if ac6.shop.read(i).equipId == equipId then return i end
  end
end

-- THE pending task: who fills entry #i each frame, and from where (RBX = source entry)
function ac6.findEntrySource(i, frames)
  i = i or 0
  local target = ac6.shop.entry(i)
  ac6.trap(ac6.code.entryCopyIdWrite.addr, {
    name = string.format("entryCopy->#%d", i), once = true, backtrace = frames or 12,
    cond = function() return RDI == target end,
    onHit = function()
      log("entry #%d source RBX=%X (%s)  RDI=%X", i, RBX, ac6.inModule(RBX) and "module" or (RBX < 0x1000000 and "STACK" or "heap"), RDI)
      ac6.shop.lastSource = RBX
    end,
  })
end

----------------------------------------------------------------------------- quick checks
function ac6.status()
  log("base=%X size=%X  GameDataMan=%X GameData=%X lineupData=%X",
      ac6.base, ac6.size, ac6.gameDataMan(), ac6.gameData(), ac6.lineupData())
  for _, id in ipairs({ 184, 112, 371 }) do ac6.printRow(id) end
  if ac6.shop.begin then log("shop list cached: begin=%X count=%d", ac6.shop.begin, ac6.shop.count) end
end

log("ac6_session loaded. ac6.verify() / ac6.captureShopList() / ac6.shop.dump() / ac6.findEntrySource(0) / ac6.trap(addr,opts) / ac6.watchWrite(addr) / ac6.clearTraps()")
