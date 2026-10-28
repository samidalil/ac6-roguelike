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
  They are kept after a failed sortie.
- A part you sell (or that is sold automatically) stays in the shop at its shop price until the next reroll,
  so you can buy it back.
- The game's own reward parts (S-rank and arena rewards) are not handed out: they show up in the shop like any part.
- No pause menu during missions: no retry, no quitting.
- Dying, or leaving a mission any other way (even closing the game), counts as a failed sortie:
  * every equipped weapon and frame part / internal is lost; a lost booster, generator or FCS is lost for good
    (never offered again, except the starter parts), lost weapons and frame parts can come up again;
  * each emptied slot gets your most recently obtained spare part for it; with no spare, the starter part is bought
    (its shop price) and equipped. Tank legs have no booster: the booster slot stays empty.
  A dialog lists what you lost. Equipped parts are never sold automatically.
- The 7 starter frame parts / internals are always in the shop while you don't own them.
- If your credits go below 0, your spare parts are sold automatically, most expensive first.
- If even that is not enough, the run is over: GAME OVER, back to the title. The save is dead; start a New Game.

SETTINGS
ac6_roguelike.toml (created next to the DLL on first launch). Edit it and restart the game:
mission pay %, starting credits, ammo/repair cost %, sell and buy %, shop size, reroll interval, parts per mission,
parts offered per reward, OS Tunings as rewards, part prices, rarity odds and tiers.
Each setting is explained in the file. Delete the file to get the defaults back.

LOG
%USERPROFILE%\ac6_roguelike_dll.log (rewritten at every launch) - useful when reporting a problem.
