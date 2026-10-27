ARMORED CORE VI ROGUELIKE
=========================
A roguelike mode for ARMORED CORE VI: FIRES OF RUBICON (Steam, game version 1.09), as a me3 native mod.
It uses its own save file (AC6_roguelike.sl2), so your normal save is untouched.

INSTALL
1. Install me3 (https://github.com/garyttierney/me3) and make sure Steam is running.
2. Unzip this folder anywhere.
3. Launch with:   me3 launch --auto-detect -p "<this folder>\ac6-roguelike.me3"
   (or open ac6-roguelike.me3 with me3 if file associations are set up)
4. Start a NEW GAME for a fresh run.

HOW IT PLAYS
- Missions pay nothing. Selling parts is your income; ammunition and repairs still cost money.
- The parts shop only offers a few random parts (1 weapon + 2 parts by default), rerolled after every cleared
  mission. Each cleared mission also gives you a random part. Rarer parts come up less often.
- The game's own reward parts (S-rank and arena rewards) are not handed out: they show up in the shop like any part.
- No pause menu during missions: no retry, no quitting.
- Dying, or leaving a mission any other way (even closing the game), counts as a failed sortie:
  * every equipped weapon is lost;
  * every equipped frame part / internal that is not a starter part is lost and replaced by the starter part;
  * a slot that already had its starter part costs that part's price.
  A dialog lists what you lost.
- If your credits go below 0, your spare parts are sold automatically, most expensive first.
- If even that is not enough, the run is over: GAME OVER, back to the title. The save is dead; start a New Game.

SETTINGS
ac6_roguelike.toml (created next to the DLL on first launch). Edit it and restart the game:
mission pay %, ammo/repair cost %, sell and buy %, shop size, parts per mission, part prices, rarity odds and tiers.
Each setting is explained in the file. Delete the file to get the defaults back.

LOG
%USERPROFILE%\ac6_roguelike_dll.log (rewritten at every launch) - useful when reporting a problem.
