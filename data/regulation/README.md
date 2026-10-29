# Regulation declarations

Every `*.toml` file here is applied to the game's own `regulation.bin` by `tools/build_regulation.ps1`, which writes
`mod/regulation.bin` (served by me3 from the mod package; `tools/package.ps1` ships it in the dist). With no
declarations nothing is written and the game uses its own regulation.

```toml
[EquipParamWeapon.10010100]    # [ParamName.rowId]: an existing row is edited
totalBulletNum = 600           # Smithbox field name = number or "text"

[SpEffectParam.900000001]      # a row that does not exist is added (rows stay sorted by id)
copy = 6200                    # new row: start from a copy of this row (else every field 0)
effectEndurance = 12.5
```

Chip files (`dll/src/chips/*.toml`) add their `[speffect.<rowId>]` sections as SpEffectParam rows (same keys).
Param names are the regulation's file names (as in Smithbox). The file is the whole game regulation: rebuild it after
a game update, or the old data would replace the new.
