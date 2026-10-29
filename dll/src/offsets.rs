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
#[allow(dead_code)]
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
/// `u32 Obtainable(const u32* id)`: copies of the item the player may still get in the shop = 1 when the id is not
/// in the inventory (exe+474060 < 0), else 0. Asked by the buy list's ownership filter exe+E89820 (via exe+D2F860),
/// the purchase confirmation exe+10A96FE, the buy step exe+D669C5, the purchasable quantity exe+D2F4DF and the adds
/// exe+D2EC51 / D2ECE1.
pub const FN_SHOP_OBTAINABLE: usize = 0xD2F900;
/// The shop purchase's add (shop array, 2026-10-30; hardware breakpoint on D2EC20 during an Achat purchase: return
/// exe+D4E012, stack D663AD < ... D640C0 jobs of the buy step exe+D66940): exe+D4DFE0(transaction) takes the item id
/// (exe+435C90) and the count (+0x78), then `call exe+D2EC20(const u32* id, n) -> al` at exe+D4E00D. D2EC20 = n =
/// min(D2F900, requested), then the add (exe+D30840); al = 1 also when n <= 0 (nothing added). Its 10 other callers
/// are give paths outside the shop (and exe+10A47D3, a list window's own commit, not the Achat purchase).
pub const PURCHASE_ADD_CALLSITE: usize = 0xD4E00D;
pub const FN_ADD_OBTAINABLE: usize = 0xD2EC20;
/// Shop buy step exe+D66940 (inventory cap, 2026-10-29): `call exe+D2F860(const u32* id)` (= D2F900 on a copy) at
/// exe+D669C5; 0 -> the game's refusal dialog with message 231020 ("Vous avez acheté toutes les pièces disponibles.").
pub const BUY_OBTAINABLE_CALLSITE: usize = 0xD669C5;
pub const FN_SHOP_OBTAINABLE_REF: usize = 0xD2F860;
pub const MSG_SHOP_NONE_LEFT: u32 = 231020;
pub const MSG_CAT_SHOP: u32 = 204;
/// The garage's VENTE entry (inventory cap, 2026-10-30): the menu manager runs a std::function whose callable is the
/// game's function-pointer wrapper (vtable exe+3912780, pointer at +8) around exe+10A5C00(out RefPtr<MenuJob>, ctx),
/// which builds the shared part-list window 02_010_EquipTop (builder exe+10A6F20) in sell mode 0x23 (Achat = 0x22).
pub const FN_PTR_FUNCTION_VTABLE: usize = 0x3912780;
pub const FN_SELL_MENU: usize = 0x10A5C00;
/// exe+DC4B60(out handle[3], const std::function*): the menu manager makes the job (exe+D74BC0, ctx = manager+0x10)
/// and opens it over the current menu (exe+D83350); what the generic dialog builder exe+DC1650 calls.
pub const FN_MENU_JOB_OPEN_ON_TOP: usize = 0xDC4B60;
/// exe+DC4AE0(job, id) -> 1 while that job is the manager's open child (+0x68/+0x70) or the next one (+0x78/+0x80).
pub const FN_MENU_JOB_IS_OPEN: usize = 0xDC4AE0;
/// Menu table: {flags, UTF-16 name} entries of 16 bytes (flags low byte = draw layer); the game preloads each movie.
/// A window is built from a name string shared with its entry (02_010_EquipTop exe+3901B60: the part-list window class,
/// ctor exe+10A2E60; 02_Equip exe+38F2A08, built by the list builder exe+10A618B; 02_GaitemStatus_General_* the part
/// info and stats). The cap's sell screen patches those strings to its own movies, preloaded under online-only entries.
pub const MENU_TABLE: usize = 0x4E28CF0;
pub const MENU_ENTRY_EQUIP_TOP: usize = 0x16;
pub const MENU_ENTRY_EQUIP: usize = 0x17;
pub const MENU_ENTRY_GAITEM_STATUS: [usize; 3] = [0x2E, 0x2F, 0x30]; // Normal, DetailComparison, ModelViewer (0x2D Minimum: no file)
/// 02_OnlineArena_MatchLog, _PlayerDetails, _RankMatch, _NamePlateList, _RoomList.
pub const MENU_ENTRY_ONLINE: [usize; 5] = [0x64, 0x65, 0x67, 0x69, 0x62];
/// The VENTE factory's screen id (RE_NOTES "Screen ids and the first-time menu tutorials"): `mov edx, 0x23;
/// call exe+CD74F0(u16* out, id)` at exe+10A5EBE (ACHAT's 0x22 at exe+10A5B59). The cap's screen gets 0x80 instead:
/// no MenuBehaviorParam row (switch exe+15A5BF0 default: no tutorial request, no help) and outside the manager's
/// per-screen state array (every writer skips ids >= 0x63).
pub const SELL_SCREEN_ID_CALLSITE: usize = 0x10A5EBE;
pub const FN_STORE_SCREEN_ID: usize = 0xCD74F0;
pub const CAP_SCREEN_ID: u16 = 0x80;
/// 02_010_EquipTop window class vtable (Assemblage / Achat / Vente lists); slot 1 = its deleting destructor
/// exe+10A58C0(this, flags), run when the list closes.
pub const LIST_WINDOW_VTABLE: usize = 0x39E9F68;
pub const FN_LIST_WINDOW_DTOR: usize = 0x10A58C0;
/// Inventory table inserts (inventory cap, 2026-10-30): exe+C2A0F0 / exe+C2A180 (table = item manager + 8, desc ->
/// index or -1), one per half of the table. Only callers: AddItem exe+474D80 (474E91 / 474EA2) and the commit of the
/// obtained items exe+4756E0 (4757A4 / 4757AB).
pub const FN_INV_INSERT_HI: usize = 0xC2A0F0;
pub const FN_INV_INSERT_LO: usize = 0xC2A180;
pub const ADD_INSERT_HI_CALLSITE: usize = 0x474E91;
pub const ADD_INSERT_LO_CALLSITE: usize = 0x474EA2;
pub const COMMIT_INSERT_HI_CALLSITE: usize = 0x4757A4;
pub const COMMIT_INSERT_LO_CALLSITE: usize = 0x4757AB;
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
pub const ROWS_LOST: u32 = 4550;   // 4550..4867: never offered again (lost forever, or offered [max_occurrences] times)
/// Shop slot k bought since the reroll (1 flag each, 8 slots: 7100..7107; dupshop.rs). Block 7 (7100..7131 free: old
/// saves from before the shop array may hold other bits there; not converted).
pub const SHOP_BOUGHT: u32 = 7100;
/// [max_occurrences] OS Tunings offered their max number of times this run (1 bit per OS_TUNINGS entry: 4418..4434;
/// lineup parts use ROWS_LOST, chips CHIP_GONE). Block 4 was verified to hold only ROWS_SOLD / ROWS_LOST / REWARD_RNG bits several
/// missions in.
pub const OS_GONE: u32 = 4418;
/// Our own chips (chip.rs), by chip id 1..MAX_CHIPS: level (2 bits, LSB first: 4440..4503) and "offered its max
/// number of times" (1 bit: 4504..4535). Block 4, unused range 4435..4549.
pub const CHIP_LEVELS: u32 = 4440;
pub const CHIP_LEVEL_BITS: u32 = 2;
pub const CHIP_GONE: u32 = 4504;
pub const MAX_CHIPS: u16 = 32;
/// [os_tuning]-style row of chip id k (used as its key in [part_rarity], [lost_on_failure], [max_occurrences]).
pub const CHIP_ROW0: u16 = 9000;
/// Chip scripts' saved byte (script.rs `store`): SCRIPT_STORE_BITS per chip id, LSB first, 7132..7387 (block 7).
pub const SCRIPT_STORE: u32 = 7132;
pub const SCRIPT_STORE_BITS: u32 = 8;
pub const CHIP_MAX_LEVEL: usize = 3;
/// Own chips' stat hooks (RE 2026-10-29, RE_NOTES "Own OS chips"): per-frame stat code, every character; the hooks
/// change the player's only. Set max AP (rcx = AP block = player module 0, edx = value; clamps and writes +0x27C):
pub const FN_SET_MAX_AP: usize = 0x82AEC0;
pub const SET_MAX_AP_CALLSITES: [usize; 7] = [0x48FB51, 0x491D6F, 0x71D668, 0x82A4D3, 0x82B5DD, 0x82B6AE, 0xB6AEED];
/// EN capacity of an EN object (rcx = player module CHR_MOD_EN; float, stored at +0x20 every frame)
pub const FN_EN_CAPACITY: usize = 0x88BAE0;
pub const EN_CAPACITY_CALLSITE: usize = 0x88B7C8;
/// Total weight of an assembly (rcx = PlayerGameData + GD_ASSEMBLY for the player; float, stored at weapon/weight
/// module 0x23 +0x408 every frame)
pub const FN_TOTAL_WEIGHT: usize = 0x46CA70;
pub const TOTAL_WEIGHT_CALLSITE: usize = 0x850B77;
pub const GD_ASSEMBLY: usize = 0x230;
/// Player character modules ([[[WorldChrMan]+WCM_PLAYER]+PLAYER_MODULES] + 8 x k): the EN object.
pub const CHR_MOD_EN: usize = 0x2A;
/// SpEffects (RE 2026-10-29, RE_NOTES "SpEffects on the player"): a character's SpEffect manager at chr + 0x308
/// (vtable SPEFFECT_MGR_VTABLE; +8 active list head, entries {id, param row +8, next +0x38, flags +0x64}, +0x10 owner);
/// apply = exe+968EA0(manager, id, target chr, source chr, *params [f32; 4], u8, u8, u8) (callers pass 1.0 floats
/// and 0 flags), the game thread only.
pub const CHR_SPEFFECT_MGR: usize = 0x308;
pub const SPEFFECT_MGR_VTABLE: usize = 0x3839EB8;
pub const FN_APPLY_SPEFFECT: usize = 0x968EA0;
/// AP change report (RE 2026-10-29, RE_NOTES "Enemy destroyed event"): each character's update (exe+756370, every
/// frame) calls exe+1714F00(chr) at exe+7563FF when its AP ratio moved by more than 10 % since the last frame, or its AP
/// changed and is below 10; the AP block's last-frame AP (CHR_AP_PREV) is updated after the call.
pub const FN_AP_CHANGED: usize = 0x1714F00;
pub const AP_CHANGED_CALLSITE: usize = 0x7563FF;
pub const CHR_AP_PREV: usize = 0x38C;
/// Weapon module of a character (module 0x27 of the player; constructor exe+8FF8AE): vtable, owner character at +8,
/// WEAPON_SLOTS slots of WEAPON_SLOT_STRIDE from WEAPON_SLOT0 (slot 1 = right arm, then back units): equipId
/// +0x44, ammunition {loaded, reserve} at +0x88 / +0x8C (-1: no ammunition).
pub const WEAPONS_VTABLE: usize = 0x3829510;
pub const WEAPONS_OWNER: usize = 0x08;
pub const WEAPON_SLOT0: usize = 0x20;
pub const WEAPON_SLOT_STRIDE: usize = 0x480;
pub const WEAPON_SLOTS: usize = 0x20;
pub const SLOT_ID: usize = 0x44;
pub const SLOT_LOADED: usize = 0x88;
pub const SLOT_RESERVE: usize = 0x8C;
/// Adding ammunition to a weapon slot (rcx = slot + SLOT_AMMO_BLOCK, edx = rounds; loaded round first, then the
/// reserve, capped at the weapon's totalBulletNum): called when a weapon is filled (exe+907661, the set-weapon path)
/// and for every slot with a rate (exe+907714).
pub const FN_ADD_AMMO: usize = 0x907750;
pub const ADD_AMMO_CALLSITES: [usize; 2] = [0x907661, 0x907714];
pub const SLOT_AMMO_BLOCK: usize = 0x30;
/// EquipParamWeapon totalBulletNum (s32)
pub const WEAPON_TOTAL_AMMO: usize = 0x2A8;
/// [max_occurrences] items offered but not used up yet (occur.rs): OCCUR_SLOTS slots of {9-bit key: lineup row + 1,
/// or ROW_COUNT + 1 + rank in OS_TUNINGS, 0 = free; 4-bit count}, LSB first, 8030..8991. Block 8 holds no game flag
/// (0 set on a fresh save and several missions in; 8020 = FLAG_NEVER is never set and stays out of the range).
pub const OCCUR_BASE: u32 = 8030;
pub const OCCUR_SLOTS: u32 = 74;
pub const OCCUR_KEY_BITS: u32 = 9;
pub const OCCUR_COUNT_BITS: u32 = 4;
/// reward choice (choice.rs): the reward RNG state, 64 bits
pub const REWARD_RNG: u32 = 4901;          // 4901..4964
/// Missions cleared since the last shop reroll (8 bits, LSB first).
pub const REROLL_COUNTER_BASE: u32 = 7830;
pub const REROLL_COUNTER_BITS: u32 = 8;

// --- param repository (RE 2026-10-26): the shop builder exe+159AAE0 does [exe+4E27300] -> exe+15C8980(repo, 0x29, 0) ---
pub const PARAM_REPO_GLOBAL: usize = 0x4E27300;
/// armoredcore6.exe 1.09 image size (tools/ac6_lib.py SIZE)
pub const IMAGE_SIZE: usize = 0x6C2FC00;
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
// --- OS Tuning (TGA table "OS Tuning"; RE 2026-10-28, RE_NOTES "OS Tuning rewards") ---
/// PlayerGameData + this = pointer to the OS Tuning levels (one i32 per OsReinforce row, in row id order, 0 = not
/// installed). Assemblage lists an owned core expansion only while its level is > 0.
pub const GD_OS_TUNING: usize = 0x1368;
/// The OS Tunings: (OsReinforce row id, level offset, name id in MSG_CAT_OS_NAME, core expansion item equipId or 0).
/// Rows 1011..1013 (offsets 0x18..0x20) have no text: hidden sub-rows, not rewards.
pub const OS_TUNINGS: [(u16, usize, u32, u32); 17] = [
    (10, 0x00, 71510, 0), (20, 0x04, 71511, 0), (30, 0x08, 71512, 0), (40, 0x0C, 71513, 0), (50, 0x10, 71514, 0),
    (1010, 0x14, 71520, 0), (2020, 0x24, 71531, 0), (3010, 0x28, 71540, 0), (4010, 0x2C, 71545, 0),
    (5010, 0x30, 71560, 75000000), (5020, 0x34, 71561, 75010000), (5030, 0x38, 71562, 75020000), (5040, 0x3C, 71563, 75030000),
    (5050, 0x40, 71550, 0), (5060, 0x44, 71552, 0), (5070, 0x48, 71551, 0), (5080, 0x4C, 71553, 0),
];
/// OsReinforce row: max level u8 at +4, then 5 levels of {increase i32 (the effect: read by the game at each hit,
/// shown as <?osValue?>), OST chip cost i32, text id i32} from +0x14, 0x18 apart; a level exists while its cost is > 0.
pub const OSR_MAX_LEVEL: usize = 0x04;
pub const OSR_LEVEL0_VALUE: usize = 0x14;
pub const OSR_LEVEL0_COST: usize = 0x18;
pub const OSR_LEVEL0_TEXT: usize = 0x1C;
pub const OSR_LEVEL_STRIDE: usize = 0x18;
pub const OSR_LEVELS: usize = 5;
/// ACS (1010): hidden sub-rows given the same [os_tuning] values (1013, coral, has 0 everywhere and is left alone).
pub const OSR_ACS: u16 = 1010;
pub const OSR_ACS_SUBROWS: [u16; 2] = [1011, 1012];
/// Item queued for an OS offer that grants no item (the results' parts dialog only opens on a non-empty pending
/// list): PULSE ARMOR, always taken out of the list at the commit.
pub const OS_CARRIER_ITEM: u32 = 75010000;
pub const MSG_CAT_OS_NAME: u32 = 200;
/// (category, id) of the garage menu's title "GARAGE"
pub const MSG_GARAGE_TITLE: (u32, u32) = (200, 100000);
/// garage menu titles / unlock notification titles (73) and texts (74)
pub const MSG_CAT_MENU_TITLE: u32 = 73;
pub const MSG_CAT_MENU_TEXT: u32 = 74;
pub const MSG_OS_MENU: u32 = 2009000;      // "AMÉLIORATION OS"
pub const MSG_OS_UNLOCKED: u32 = 2101000;  // "Accès autorisé : AMÉLIORATION OS/ARÈNE" + its text (both menus listed)
/// UnlockParam_MenuCommand: row of the OS Tuning menu ({type 2 = event flag, flag 3131}) and a flag nothing sets
/// (the game's own row 99999999 uses it): pointing the row at it keeps the menu locked.
pub const MENU_ROW_OS_TUNING: u32 = 10080;
pub const UNLOCK_ROW_FLAG: usize = 0x4;
pub const FLAG_OS_TUNING_MENU: u32 = 3131;
pub const FLAG_NEVER: u32 = 8020;
/// ALLMIND's garage message granting OS Tuning + Arena: TalkParam rows 913001010..016, one voiced line each
/// ({?, msgId @+4, voiceId @+8}). Rows 011 (OS Tuning granted) and 012 ("En outre...") are silenced.
pub const TALK_ALLMIND_OS: u32 = 913001011;
pub const TALK_ALLMIND_OS_MSG: u32 = 913001020;
pub const TALK_ALLMIND_CUT: u32 = 2;
pub const TALK_ROW_MSG: usize = 0x4;
pub const TALK_ROW_VOICE: usize = 0x8;

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
// "Pièces obtenues" menu (02_GotReward, parts mode) and the reward choice grid (rewardmenu.rs)
pub const FN_MENU_JOB_FROM_FACTORY: usize = 0xDA3410;   // (out RefPtr<MenuJob>, const std::function*) -> out
pub const FN_MENU_JOB_PUSH: usize = 0xDC2A70;           // (RefPtr<MenuJob>*): hands the job to the menu manager, releases it
pub const FN_GOT_REWARD_PARTS_MENU: usize = 0x10158D0;  // (out RefPtr<MenuJob>, ctx, const vector<u32>* item ids)
pub const RESULTS_PARTS_MENU_CALLSITE: usize = 0xE9A26E; // results screen lambda exe+E9A220: the pending parts dialog
pub const FN_DEFAULT_ALLOCATOR: usize = 0x2923D10;      // () -> the game's default allocator
pub const GOT_REWARD_MOVIE_NAME: usize = 0x39BF5C8;     // UTF-16 "02_GotReward" the class builds its window with
pub const MENU_TABLE_SPARE_ENTRY: usize = 0x4E29168;    // menu table {flags, name} name of 02_OnlineArena_GotReward
// component API of the menu framework and the part view (one part's slots: logo, icon, category, name)
pub const FN_GET_CHILD: usize = 0xC928E0;               // (component handle, out handle, const char* path "A/B")
pub const FN_SET_TEXT: usize = 0xC8CBA0;                // (handle + 8, const wchar_t*)
pub const FN_HANDLE_PREPARE: usize = 0xC94640;          // (handle*): done on the content handle before its page fills it
pub const FN_HANDLE_DTOR: usize = 0xC6A370;             // (handle*): component handle destructor
pub const FN_PART_VIEW_FILL: usize = 0x1000B60;         // (view, item descriptor)
pub const PART_VIEW_FILL_CALLSITE: usize = 0x1012CE4;   // in the parts mode update
pub const CONTENT_LOOKUP_CALLSITE: usize = 0x1010A44;   // page function: GetChild(root = this+0x4F0, "_Content_"), rsi = page
pub const PAGE_CONTENT_CALLABLE_VTABLE: usize = 0x39BF948; // a parts page's content std::function _Impl (record at +8)
// commit of the pending obtained items (script step exe+B042C0, OK of the results' parts dialog)
pub const FN_PENDING_COMMIT: usize = 0x4756E0;          // (mgr = GameData+0x600, commit u8, obj = GameData+0x230)
pub const PENDING_COMMIT_CALLSITE: usize = 0xB042FC;
// menu input (RE_NOTES "Menu input"): (repeater, input object, ?, ?, f32 dt) -> menu action pulses in the bitset at
// [input + 0x7C8] (virtual input ids: confirm 0x155, back 0x154, right 0x156, left 0x159, down 0x15C, up 0x15F)
pub const FN_INPUT_REPEAT: usize = 0x29F23F0;
pub const INPUT_ACTION_BITS: usize = 0x7C8;

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
