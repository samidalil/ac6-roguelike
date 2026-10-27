-- TEST helper: equip owned parts by writing the three per-slot arrays (INDEX +0x24C, HANDLE +0x424, ID +0x4F4).
-- EQUIP = { {slot, itemId(tag|equipId), equipId}, ... }  (slot 0 L-arm, 18 head, 25 FCS ...)
local gd = GD
local mgr = gd + 0x600
local function entry(i)
  local split = readInteger(mgr + 0x1C)
  if i < split then return readQword(mgr + 0x40) + i * 16 end
  return readQword(mgr + 0x30) + (i - split) * 16
end
local cap = math.min(readInteger(mgr + 0x08), 4096)
for _, e in ipairs(EQUIP) do
  local slot, item, eid = e[1], e[2], e[3]
  local found
  for i = 0, cap - 1 do
    local a = entry(i)
    if readInteger(a) ~= -1 and (readInteger(a + 4) & 0xFFFFFFFF) == item then found = i; break end
  end
  if found then
    local h = readInteger(entry(found))
    writeInteger(gd + 0x24C + 4 * slot, found)
    writeInteger(gd + 0x424 + 4 * slot, h)
    writeInteger(gd + 0x4F4 + 4 * slot, eid)
    ac6.log("equipped slot %d <- item %X (index %d handle %X)", slot, item, found, h & 0xFFFFFFFF)
  else
    ac6.log("item %X not owned", item)
  end
end
return 1
