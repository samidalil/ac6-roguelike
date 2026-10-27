//! Verified addresses/offsets for armoredcore6.exe 1.09 (base 0x140000000, no ASLR).
//! Every value here is documented in RE_NOTES.md; all are RVAs (add the runtime module base).


// --- global singletons (RVA of the pointer slot, deref once to get the instance) ---
/// GameDataMan instance pointer (TGA symbol "GameDataMan").
pub const GAMEDATAMAN_GLOBAL: usize = 0x4E03A08;
/// CSEventFlagMan instance pointer.
pub const EVENTFLAGMAN_GLOBAL: usize = 0x4E13070;

// --- GameDataMan layout ---
pub const GDM_PLAYER_GAMEDATA: usize = 0x08; // -> player GameData

// --- EquipmentLineupParam row (stride 0x20) ---
pub const ROW_STRIDE: usize = 0x20;
pub const ROW_COUNT: usize = 318;
pub const ROW_EQUIP_ID: usize = 0x00; // s32
pub const ROW_PRICE: usize = 0x04; // s32
pub const ROW_EQUIP_TYPE: usize = 0x0C; // u8 (0 weapon, 1 frame, 4 booster, 5 FCS, 6 generator)
pub const ROW_REWARD_COND: usize = 0x10; // u32

// --- CSEventFlagMan layout (direct bit-page access; verified by tools/flag_monitor.py) ---
pub const EFM_DIVISOR: usize = 0x1C;      // u32 flags per block (1000)
pub const EFM_BLOCK_BYTES: usize = 0x20;  // u32 bytes per block page (125)
pub const EFM_PAGES_BASE: usize = 0x28;   // ptr: page array base (type-1 nodes index into it)
pub const EFM_MAP_HEADER: usize = 0x38;   // ptr: rb-tree header of block nodes {left@0, right@0x10, nil@0x19, block@0x20, type@0x28, idx/ptr@0x30}
pub const EFM_DIRTY: usize = 0x60;        // u8 set by SetEventFlag so the save serializes the flags

// --- event flags the mod watches ---
pub const FLAG_REPLAY_MISSION: u32 = 3134; // keep OFF for the roguelike
/// Parts Shop (Market) menu unlocked: set by the game at the mission 2 clear, with the first parts wave 6200
pub const FLAG_MARKET: u32 = 3127;
/// rises when the mission is cleared (results pending), falls ~2 s after the obtained-parts commit
pub const FLAG_MISSION_CLEARED: u32 = 5074;
/// result screen up (rises after the commit, falls when the player closes it, back in the garage)
pub const FLAG_RESULTS: u32 = 3139;

// --- item manager / give path (RE 2026-09-24, see RE_NOTES "Pending obtained parts list") ---
/// GameData + this = the item-manager object (`mgr`).
pub const GD_ITEM_MGR: usize = 0x600;
/// GameData + this = the object the give wrapper takes as `this` (mgr - 0x3D0).
pub const GD_GIVE_OBJ: usize = 0x230;
/// mgr + this = DEFER byte: 1 while a mission is active (grants are queued to the pending list).
pub const MGR_DEFER: usize = 0x99;
/// mgr + these = pending-obtained vector<u32 gaitem handle> begin / end pointers (vector object at mgr+0x78)
pub const MGR_PENDING_BEGIN: usize = 0x80;
pub const MGR_PENDING_END: usize = 0x88;
/// mgr + this = pointer to the gaitem table (16-byte entries {handle, id, count, ordinal}; free = 0xFFFFFFFF)
pub const MGR_GAITEM_TABLE: usize = 0x10;
pub const GAITEM_TABLE_MAX: usize = 2048;
/// give-obj + this = last give error (0 ok, 2 = already owned).
pub const GIVEOBJ_ERROR: usize = 0xBCC;
/// item-descriptor builder: `desc* B86700(ctx = [exe+ITEM_DESC_CTX], out[0x40], const u32* id)`
pub const FN_BUILD_DESC: usize = 0xB86700;
pub const ITEM_DESC_CTX: usize = 0x4E14008;
/// give wrapper: `i32 46B1C0(giveObj, desc, qty, r9b, a5, a6, a7 = bypass-defer)` ; returns -1 when queued
pub const FN_GIVE_ITEM: usize = 0x46B1C0;
/// shop builder's call to the release predicate (`call exe+159ACB0`, 5 bytes) — the hook site
pub const SHOP_PRED_CALLSITE: usize = 0xE89074;
pub const FN_SHOP_PRED: usize = 0x159ACB0;
/// itemCtx + this = pointer to the EquipmentLineupParam row the shop is testing
pub const ITEMCTX_ROW: usize = 0x08;
/// Item-id category tag per lineup equipType (verified from the inventory table, 2026-09-24):
/// weapons (type 0) carry the bare equipId; frames (1, ids 50-53xx) 0x10000000; boosters (4, ids
/// 60xx) 0x60000000; FCS (5, ids 70xx) 0x70000000; generators (6, ids 65xx) 0x50000000.
pub fn id_tag(equip_type: u8) -> u32 {
    match equip_type { 0 => 0, 1 => 0x1000_0000, 4 => 0x6000_0000, 5 => 0x7000_0000, 6 => 0x5000_0000, _ => 0 }
}

// --- persistence: our state lives in (hopefully unused) event flags, saved in the .sl2 ---
/// first flag id of the 28-bit payload (3 x 9-bit lineup row indices + valid bit).
/// 9900+ turned out to be game-owned (9901/9902/9904/9915/9924 set at mission end); block 8 is
/// empty on a fresh save through mission 2 — verify further with tools/flag_monitor.py.
pub const STATE_FLAG_BASE: u32 = 7800; // block 7: probes 7000/7500/7900 untouched over 2 missions + reload
pub const STATE_FLAG_BITS: u32 = 27; // bits 0..26 = indices, bit 27 = valid
/// Shop slots 3.. (config shop size above 3): 9 flags each from here, value = row index + 1 (0 = empty)
pub const STATE_FLAG_EXTRA_BASE: u32 = 7900;
/// Row bitsets: flag base + lineup row index (ROW_COUNT flags each). Blocks 4 and 7 hold no game flag (only old
/// probes 4000/4500/4900/7000/7500, avoided); set + save + relaunch + read back verified 2026-10-27.
#[allow(dead_code)] // used by the buy-back, lost-forever and pool features
pub const ROWS_SOLD: u32 = 4100;   // 4100..4417: sold since the last reroll (buy-back)
pub const ROWS_LOST: u32 = 4550;   // 4550..4867: lost forever (failures)
#[allow(dead_code)]
pub const ROWS_ROLLED: u32 = 7100; // 7100..7417: already rolled (pool without repeats)
// reward choice (choice.rs): the reward pool's "offered" bitset is split (rows 0..298 / 299..317), the pending offer
// is up to 12 x 9-bit row indices, the reward RNG state 64 bits
pub const REWARD_ROLLED_A: u32 = 7501;     // 7501..7799
pub const REWARD_ROLLED_A_LEN: u32 = 299;
pub const REWARD_ROLLED_B: u32 = 7838;     // 7838..7856
pub const OFFER_ROWS: u32 = 4420;          // 4420..4527
pub const OFFER_COUNT: u32 = 4868;         // 4 bits
pub const OFFER_SETS_LEFT: u32 = 4872;     // 4 bits
pub const REWARD_RNG: u32 = 4901;          // 4901..4964
/// Missions cleared since the last shop reroll (8 bits, LSB first).
pub const REROLL_COUNTER_BASE: u32 = 7830;
pub const REROLL_COUNTER_BITS: u32 = 8;

// --- param repository (RE 2026-10-26): the shop builder exe+159AAE0 does [exe+4E27300] -> exe+15C8980(repo, 0x29, 0) ---
pub const PARAM_REPO_GLOBAL: usize = 0x4E27300;
pub const REPO_SLOT0: usize = 0x80;          // slot i at repo + 0x80 + i*0x48: {u32 count, pad, ptr res[...]}
pub const REPO_SLOT_STRIDE: usize = 0x48;
pub const REPO_SLOTS: usize = 0x109;
pub const RES_FILE_HOLDER: usize = 0x80;     // file = [[res+0x80]+0x80]; res+0x18 = wide name
pub const RES_NAME: usize = 0x18;            // MSVC std::wstring {buf/ptr, , len @+0x10, cap @+0x18}
/// EquipParamProtector row: legsType u16 (0 none, 1 biped, 100 tetrapod, 200 reverse joint, 300-302 tank)
pub const PROTECTOR_LEGS_TYPE: usize = 0xDA;
pub const PARAMFILE_ROWCOUNT: usize = 0x0A;  // u16
pub const PARAMFILE_ROWS: usize = 0x40;      // row headers {id u32, pad, dataOffset u64, nameOffset u64}
pub const PARAMFILE_ROW_STRIDE: usize = 0x18;
pub const PARAM_IDX_LINEUP: usize = 0x29;
pub const PARAM_IDX_MISSION: usize = 0x91;
pub const PARAM_IDX_ARENA: usize = 0xC8;
// MissionParam (row 0x160): base pay + bonus lines (each {s32 value, s32 text}; stride 8)
pub const MISSION_ROW_SIZE: usize = 0x160;
/// MissionParam row of the first mission of every New Game (Illegal Entry): pays [economy] starting_credits.
pub const MISSION_FIRST: u32 = 2010;
pub const MISSION_CURRENCY_REWARD: usize = 0x2C;
pub const MISSION_EVENT_REWARD0: usize = 0x5C;
pub const MISSION_EVENT_REWARDS: usize = 5;
pub const MISSION_ENEMY_REWARD0: usize = 0xDC;
pub const MISSION_ENEMY_REWARDS: usize = 8;
/// MissionParam f32 cost scales (offsets from the Smithbox definition, tools/paramdef.py)
pub const MISSION_BULLET_COST_RATE: usize = 0x48;
pub const MISSION_REPAIR_COST_COEF: usize = 0x50;

// ArenaParam (row 0x60)
pub const ARENA_ROW_SIZE: usize = 0x60;
pub const ARENA_REPEAT_COAM: usize = 0x30;

// --- COAM (RE 2026-10-26, TGA "AddCOAM" AOB `?? 8b ?? ?? ?? 8d 0c 10 ?? 89 4c ?? 10`) ---
/// PlayerGameData + this = COAM balance (i32). +0x78 = lifetime earnings (don't touch).
pub const GD_COAM: usize = 0x74;
/// `AddCoam(PlayerGameData, delta)`: new = old + delta, clamped to [0, 999999999]
pub const FN_ADD_COAM: usize = 0x48DD70;

// --- player AP (RE 2026-10-26, TGA chain; global found by data scan in a mission) ---
/// WorldChrMan instance pointer slot. AP = [[[[slot]+A5A0]+320]+0]+278 (i32), max AP at +27C.
pub const WORLDCHRMAN_GLOBAL: usize = 0x4E0F598;
pub const WCM_PLAYER: usize = 0xA5A0;
pub const PLAYER_MODULES: usize = 0x320;
pub const CHR_AP: usize = 0x278;
pub const CHR_AP_MAX: usize = 0x27C;

/// Escrow flag (saved in the .sl2): set at sortie, cleared when the mission is cleared. Still set when the
/// garage loads = the mission was failed (death) or the game was killed mid-mission -> penalty.
pub const FLAG_ESCROW: u32 = 7828;

// --- save requests (RE 2026-10-26): per-frame save step exe+1272440 picks the highest-priority set bit of
// [[exe+4E140B0]+0xCB4] and saves; RequestSave() exe+B90140 does `|= 1` when saving is allowed. ---
pub const SAVE_STATE_GLOBAL: usize = 0x4E140B0;
pub const SAVE_REQ_MASK: usize = 0xCB4;
pub const FLAG_SORTIE_BRIEF: u32 = 5003;
/// Set at every new game; 0 at the title screen (no save loaded) -> its 0->1 edge marks a save load.
pub const FLAG_NEW_GAME_DONE: u32 = 1000;

// --- game-thread hook + inventory removal (RE 2026-10-26) ---
/// per-frame `call exe+1272440` (the save step) inside the game step at exe+12743xx: detoured to run our
/// game-thread work after it
pub const FRAME_HOOK_CALLSITE: usize = 0x1274418;
pub const FN_SAVE_STEP: usize = 0x1272440;
/// RemoveItemAt(mgr, inventoryIndex, 1): what selling a part calls (captured with a write-watch)
pub const FN_REMOVE_ITEM_AT: usize = 0x475150;
/// inventory index i: i < [mgr+1C] -> [mgr+40] + i*16, else [mgr+30] + (i-[mgr+1C])*16; total [mgr+08]
pub const MGR_TABLE_COUNT: usize = 0x08;
pub const MGR_TABLE_SPLIT: usize = 0x1C;
pub const MGR_TABLE_LO: usize = 0x40;
pub const MGR_TABLE_HI: usize = 0x30;

// --- pause menu (RE 2026-10-26): in the game step, `call exe+1195100` tests the pause input, then
// `call exe+D834D0` (at exe+127733D) sets the menu request [[[exe+4E15530]+18]+8]+0xD0 = 7 -> "MainMenu" opens ---
pub const PAUSE_REQ_CALLSITE: usize = 0x127733D;
pub const FN_REQUEST_PAUSE_MENU: usize = 0xD834D0;

// --- death -> straight back to the garage (RE 2026-10-26) ---
/// the mission event script opens the death ("MissionRetryMenu") menu with `call exe+DC1E40` (edx = 1) here
pub const DEATH_MENU_CALLSITE: usize = 0x17A244F;
pub const FN_OPEN_RETRY_MENU: usize = 0xDC1E40;
/// RetireMission(): what the death menu's Quit runs (discard pending items, restore pre-mission state, then
/// exe+B059A0(0) = return to garage). No arguments.
pub const FN_RETIRE_MISSION: usize = 0xB002A0;

// --- sell price (RE 2026-10-27): exe+B885D0(desc) = an item's sale value (item id at desc+8, lineup price); the shop's
// buy list never calls it. Callers: the Vente list, the sale confirmation / transaction, and a loop at exe+474598.
pub const FN_SELL_PRICE: usize = 0xB885D0;
pub const SELL_PRICE_CALLSITES: [usize; 3] = [0xD2FDDA, 0xD303DD, 0x474598];
pub const SELL_DESC_ITEM_ID: usize = 0x8;
/// Vente sale (RE 2026-10-27, buy-back): SellItem exe+D30A80(listItem, n, flag) removes the sold item with
/// `exe+D309D0(&itemId, handle, n, flag) -> count removed` (delta -n through exe+46B4E0 -> RemoveItemAt), called here
/// (the second site removes a linked item when the first removal yields one).
pub const FN_SHOP_REMOVE: usize = 0xD309D0;
pub const SALE_REMOVE_CALLSITES: [usize; 2] = [0xD30AE2, 0xD30B59];

// --- standalone dialog (RE 2026-10-26): talk-script command 17 (5 args) builds menu job type 0xF from its argument list
pub const FN_MENU_JOB_GENERIC_DIALOG: usize = 0xDC1650; // (out handle[3], args) -> out
pub const FN_MENU_JOB_OPEN: usize = 0xDC0BC0;           // (handle {vtable, job, id}) -> bool
pub const FN_MENU_JOB_RESET: usize = 0xDBA200;          // (handle): vtable reset, no release
pub const MENU_JOB_HANDLE_VTABLE: usize = 0x386E0C8;
/// Global the dialog builder requires (null at the title / during loads -> the game's fatal-error path).
pub const DIALOG_REQUIRED_GLOBAL: usize = 0x4F348C8;
// "Pièces obtenues" menu (02_GotReward): std::function<RefPtr<MenuJob>(ComponentStack&)> whose call slot (exe+DC3EC0)
// builds the menu (exe+DBAE70 -> exe+1015AC0); the captured data is unused.
pub const GOT_REWARD_FACTORY_VTABLE: usize = 0x392B7D0;
pub const GOT_REWARD_FACTORY_INT_VTABLE: usize = 0x392B770; // same, capturing an int (built at exe+DC1B95)
pub const FN_MENU_JOB_FROM_FACTORY: usize = 0xDA3410;   // (out RefPtr<MenuJob>, const std::function*) -> out
pub const FN_GOT_REWARD_PARTS_MENU: usize = 0x10158D0; // (out RefPtr<MenuJob>, ctx, const vector<u32>* item ids)
pub const FN_DEFAULT_ALLOCATOR: usize = 0x2923D10;
pub const GOT_REWARD_MOVIE_NAME: usize = 0x39BF5C8;  // UTF-16 "02_GotReward" the class builds its window with
pub const MENU_TABLE_SPARE_ENTRY: usize = 0x4E29168; // menu table {flags, name} name of 02_OnlineArena_GotReward      // () -> the game's default allocator
pub const FN_MENU_JOB_PUSH: usize = 0xDC2A70;
// component API of the menu framework and the part view (one part's slots: logo, icon, category, name)
pub const FN_GET_CHILD: usize = 0xC928E0;        // (component handle, out handle, const char* path "A/B")
pub const FN_SET_TEXT: usize = 0xC8CBA0;
pub const FN_HANDLE_PREPARE: usize = 0xC94640;    // (handle*): done on the content handle before its page fills it
pub const FN_HANDLE_DTOR: usize = 0xC6A370;       // (handle*): component handle destructor         // (handle + 8, const wchar_t*)
pub const FN_PART_VIEW_BIND: usize = 0x1001530;  // (view ~0x8A0 bytes, component handle)
pub const FN_PART_VIEW_FILL: usize = 0x1000B60;  // (view, item descriptor)
pub const PART_VIEW_BIND_CALLSITE: usize = 0x1012CD4; // in the parts mode update
pub const PART_VIEW_FILL_CALLSITE: usize = 0x1012CE4;
pub const CONTENT_LOOKUP_CALLSITE: usize = 0x1010A44;
pub const PAGE_CONTENT_CALLABLE_VTABLE: usize = 0x39BF948; // a parts page's content std::function _Impl (record at +8) // page function: GetChild(root = this+0x4F0, "_Content_")           // (RefPtr<MenuJob>*): hands the job to the menu manager, releases it

// --- game over ---
/// Run-over flag (saved): set at game over; a save loaded with it goes straight back to the title.
pub const FLAG_RUN_OVER: u32 = 7829;

// --- return to title (RE 2026-10-26, VERIFIED): the in-game update (exe+1271A30) ORs several "leave map" conditions;
// one is byte [[[exe+4E15530]+8]+0x173] (menu manager). Setting it to 1 in the garage (destination
// [[exe+4E140B0]+0x14] == -1) runs the normal exit: MoveMapStep -> Cleanup/Finish, InGameStep reinit, TitleStep -> title.
pub const MENU_MAN_GLOBAL: usize = 0x4E15530;
pub const MENU_MAN_SUB: usize = 0x8;
pub const MENU_LEAVE_MAP_REQUEST: usize = 0x173;
pub const SAVE_DESTINATION: usize = 0x14; // [exe+4E140B0]+0x14: next area id, -1 = none (title)
/// Byte [[exe+4E140B0]+0xCB8]: 1 while a game session is open (set at load); the system menu's Return to Title clears
/// it. Left set, the title shows "Return to Title Screen or Quit Game might not have been selected" (message 400003).
pub const SAVE_SESSION_OPEN: usize = 0xCB8;
/// Save request bits the system menu sets when it ends a session (exe+B900CB writes the byte, then requests 0x200 | 1).
pub const SAVE_REQ_SESSION_END: u32 = 0x201;
// Saves are refused in missions by area checks (exe+DC0760 / B8F5F0 in the save step's gate exe+B8EE10), not by
// [[exe+4E140B0]+0xCC4] (0 at a mission death).
/// OpenSession() exe+B90090 (byte = 1), called once per map load from the in-game update at exe+1271354 (also when
/// a mission returns to the garage). The system menu's EndSession() is exe+B900AF.
pub const FN_OPEN_SESSION: usize = 0xB90090;
pub const OPEN_SESSION_CALLSITE: usize = 0x1271354;
/// GameMan [exe+4E29CC0]; byte +0x3FAE is set to 1 by the system menu's Return to Title (and by RetireMission).
pub const GAME_MAN_GLOBAL: usize = 0x4E29CC0;
pub const GAME_MAN_PROPER_EXIT: usize = 0x3FAE;

// --- messages (RE 2026-10-26): MsgRepository [exe+4E256D8]; LookupEntry(repo, version, category, id) -> wchar*;
// per-table getter exe+32408B0(fmg, id). Death texts: category 200 id 254000 and category 30 id 110003004.
pub const FN_MSG_LOOKUP: usize = 0x323FE20;
pub const FN_FMG_GET: usize = 0x32408B0;
pub const MSG_REPOSITORY_GLOBAL: usize = 0x4E256D8;
/// Part-name message categories (keyed by the bare equipId): weapon, frame, booster, generator, FCS.
pub const MSG_CAT_WEAPON_NAME: u32 = 11;
pub const MSG_CAT_FRAME_NAME: u32 = 12;
pub const MSG_CAT_BOOSTER_NAME: u32 = 38;
pub const MSG_CAT_GENERATOR_NAME: u32 = 35;
pub const MSG_CAT_FCS_NAME: u32 = 41;
