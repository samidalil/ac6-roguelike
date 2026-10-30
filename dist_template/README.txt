ARMORED CORE VI ROGUELIKE
=========================
A roguelike mode for ARMORED CORE VI: FIRES OF RUBICON (Steam, game version 1.09), as a me3 native mod.
It uses its own save file (AC6_roguelike.sl2), so your normal save is untouched.

INSTALL
1. Install me3 (https://github.com/garyttierney/me3) and make sure Steam is running.
2. Unzip this folder anywhere (keep its "mod" folder next to the profile: it holds the reward choice screen).
3. Launch with:   me3 launch --auto-detect -p "<this folder>\ac6-roguelike.me3"
   (or open ac6-roguelike.me3 with me3 if file associations are set up)
4. Start a NEW GAME for a fresh run.

HOW IT PLAYS
- Missions pay nothing. Selling parts is your income; ammunition and repairs still cost money.
- The first mission of a run (Illegal Entry) pays 100 000 credits, your starting money. No other mission pays.
- The parts shop (open from mission 2 on) only offers a few random parts (1 weapon + 2 parts by default), rerolled after every cleared
  mission (or every few missions: reroll_interval). Rarer parts come up less often.
- Each cleared mission gives a reward: on the results screen you pick 1 part out of 3 random ones (pad, or arrows +
  Enter). Several rewards per mission (parts_per_mission) are several pages.
- Parts can come up again at any time. Weapons and frame parts (head, core, arms, legs) can be owned several times:
  the shop also offers ones you already own, and each copy is its own part. Boosters, generators and FCS are owned
  once: they are not offered while you have one, nor after losing one for good.
- OS Tunings are mission rewards, never bought: the OS Tuning menu stays locked. Each pick adds one level (the tile
  shows the level it brings) and a tuning comes back until its max level. The core expansions (Assault Armor, Pulse
  Armor, Pulse Protection, Terminal Armor) are rarer; the first pick also gives the part to equip in Assemblage.
  Each one shows up a limited number of times per run, as many as its levels (Assault Armor and Pulse Protection
  3, Pulse Armor 2, Terminal Armor 1): every results screen that offers it counts, picked or not. The equipped one
  is lost on a failed sortie (part and level); the loss does not give back offers. Every other installed OS
  Tuning loses one level on a failed sortie.
  [max_occurrences] and [lost_on_failure] in the settings set this per item.
  The weapon damage tunings and ACS have 3 levels (6 / 12 / 15 %), melee and direct hit 2 (10 / 15 %), repair kits 2
  (+1 000 / +2 000 AP per kit), access speed 1 (+100 %): fewer levels, same max as the base game ([os_tuning]).
- OS chips of our own, offered like the OS Tunings (3 levels each): Blindage renforcé (AP +5/10/15 %), Générateur
  surcadencé (EN capacity +10/20/30 %), Alliages légers (weight -5/10/15 %), Chargeurs étendus (ammunition
  +20/40/60 %), Contrat préférentiel (epic parts +25/50/100 % more often, shop and rewards), Prime de performance
  (+2 000 / 4 000 / 8 000 credits after each cleared mission, shown in the garage), Nanoréparation (your AC repairs
  20 / 40 / 60 AP per second during missions), Prime de série (every 3rd mission cleared in a row: +10 000 / 20 000
  / 40 000 credits; a failure resets it), Récupération (each enemy destroyed repairs 300 / 600 / 1 000 AP), Dernier
  recours (below 30 % AP: weapon damage +10 / 20 / 30 %). The AC chips change your
  AC only (enemies are not affected) and do not show in Assemblage's stats: the AP and ammunition show on the mission
  HUD. A failed sortie takes one level of each. The ammunition bonus applies when the mission starts, not to
  resupplies.
- A part you sell (or that is sold automatically) stays in the shop at its shop price until the next reroll,
  so you can buy it back.
- The game's own reward parts (S-rank and arena rewards) are not handed out: they show up in the shop like any part.
- No pause menu during missions: no retry, no quitting.
- Dying, or leaving a mission any other way (even closing the game), counts as a failed sortie:
  * every equipped weapon and frame part / internal is lost, and the equipped core expansion (its OS Tuning level
    goes back to 0); every other installed OS Tuning loses one level; a lost booster, generator or FCS is lost for
    good (never offered again, except the starter parts), lost weapons and frame parts can come up again;
    [lost_on_failure] in the settings changes what is lost, per item;
  * each emptied slot gets your most recently obtained spare part for it; with no spare, the starter part is bought
    (its shop price) and equipped. Tank legs have no booster: the booster slot stays empty.
  A dialog lists what you lost. Equipped parts are never sold automatically.
- The 7 starter frame parts / internals are always in the shop while you don't own them.
- If your credits go below 0, your spare parts are sold automatically, most expensive first.
- If even that is not enough, the run is over: GAME OVER, back to the title. The save is dead; start a New Game.

SETTINGS
ac6_roguelike.toml (created next to the DLL on first launch). Edit it and restart the game:
mission pay %, starting credits, ammo/repair cost %, sell and buy %, shop size, reroll interval, parts per mission,
parts offered per reward, OS Tunings as rewards, part prices, the pools parts are drawn from, what a failed sortie
takes ([lost_on_failure]) and how many times per run an item can be offered ([max_occurrences]: any part or OS Tuning,
1 to 8, every shop or reward choice that shows it counts). Each setting's allowed values are in the file's comments.
Pools: each [pool.NAME] section is a pool with a weight (its odds), a color ("#RRGGBB" or 0xRRGGBB) and optionally a
label and show_name; [part_rarity] puts parts in them ("SG-027 ZIMMERMAN" = epic). Shipped: common, uncommon, rare,
epic, and the built-in unlisted pool (weight 0) for every part [part_rarity] does not list. The reward choice shows
each part's pool name in the pool's colour. A part listed as 0, or in a pool whose weight is 0, is never offered in
the shop nor as a reward; with few parts left, shop slots stay empty and rewards offer fewer parts or none.
A settings file from an older version uses numbered tiers ([part_rarity] = 1/2/3, [rarity] *_weight): those are no
longer read, so every part ends up in the unlisted pool and nothing is offered. Delete the file to get the new one.
An older settings file has no [lost_on_failure] / [max_occurrences] sections (an old [unique_occurrences] section is
ignored): core expansions are then kept after a failure and offered without limit; delete the file (or copy the
sections from a new one) to get them. An older [lost_on_failure] does not list the other OS Tunings: they then keep
their levels after a failure.
Each setting is explained in the file. Delete the file to get the defaults back.
[os_tuning] sets each OS Tuning's effect per level ("6/12/15": 3 levels); an older settings file has no such section
and keeps the game's values.

OS CHIPS
The chips folder (created next to the DLL on first launch) holds one file per chip: its id (1 to 32), name, the stat
named on the reward tile, its values per level and its effects: a target = "+" / "-" (the level's value in %) or
"add" / "sub" (the value itself). Targets: ap, en_capacity, weight, ammo (your AC), pool.<NAME>.weight (odds of a
pool), reward.count, reward.choices, shop.weapons, shop.parts. A chip can also react to events with [on.<event>]
sections (mission_cleared, commit, death, shop_reroll, pick...; the list is in the default chip files): credits,
give = "part", "level.<row>" = "+1", "flag.<id>" = 1, speffect = <SpEffectParam row> (applied to your AC; a chip
can declare its own rows in [speffect.<row id>] sections, built into the mod's regulation.bin, see nanorepair.toml). Add a file for a new chip, delete one to remove it; in the
settings a chip is known by its row (9000 + id) or its name.
A chip can also have a Lua script with the same name (streak_bonus.toml + streak_bonus.lua): its on_<event>(e)
functions run on the events (plus enemy_destroyed), with log, chip, store (a byte saved with the run), game (AP,
credits, flags, levels) and the actions above; the API is listed in streak_bonus.lua. `unsafe = true` in the chip file
gives the script raw game memory (can break the game). A script error is written to the log and skipped.
Delete the folder to get the default chips back.

LOG
%USERPROFILE%\ac6_roguelike_dll.log (rewritten at every launch) - useful when reporting a problem.
