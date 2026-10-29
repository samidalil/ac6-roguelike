"""garage_to_mission_success.py [--no-results]: from the garage menu (as title_to_garage.py leaves it) to "Destruction de
l'AC d'essai" cleared and its reward grid open. Menu clicks, each one once the screen it needs is up (pixel probes,
each with a timeout); the DLL log for the sortie, the clear and the results. The mission is the 2nd row of
saves/ac-mission-available.sl2 and is gone from the list once cleared: restore the save before each run.
  DÉPLOIEMENT -> MISSIONS -> 2nd mission -> the briefing (first view in a game session, about 50 s; skipped with a
  double Tab while menu_drive.on_briefing sees it) -> mission menu -> Déploiement -> OUI -> the pre-mission talk (double
  Tab until the sortie) -> sortie (log "defer 0->1") -> tools/kill_enemies.py --max-ap 12300
  (the tester AC) -> mission cleared (log "5074 rose") -> Fermer on the results screen -> the reward grid (log "results:"), or the
  commit when there is no part to show (log "defer 1->0"). --no-results stops at the clear."""
import os, subprocess, sys, time
from menu_drive import *

GARAGE_DEPLOY, MISSIONS, MENU_DEPLOY, YES, FERMER = (237, 345), (659, 156), (311, 222), (770, 592), (140, 1036)

def main():
    focus()
    if not until(on_garage_menu, 10): sys.exit("not on the garage menu (pixel probe)")
    click(GARAGE_DEPLOY)
    if not until(on_deploy_menu, 5): sys.exit("DÉPLOIEMENT did not open")
    click(MISSIONS)
    if not until(on_mission_list, 10): sys.exit("mission list not shown")
    time.sleep(0.5)
    click(MISSION_ROW(1))
    step("mission selected")
    # the briefing (first view in a game session only): a double Tab while it shows, until the mission menu is up
    end = time.time() + 120
    while not until(on_mission_menu, 1):
        if time.time() > end: sys.exit("mission menu not reached (pixel probe)")
        if on_briefing(): skip_briefing(); step("briefing skipped")
    step("mission menu")
    mark = log_size()
    click(MENU_DEPLOY); time.sleep(0.8)  # the launch dialog fades in
    click(YES)
    # the pre-mission talk (Walter's launch cinematic, once the mission menu is gone): a double Tab until the sortie
    if not until(lambda: not on_mission_menu(), 10): sys.exit("launch not confirmed")
    end = time.time() + 60
    while not wait_log("defer 0->1", mark, 2):
        if time.time() > end: sys.exit("no sortie")
        skip_briefing(); step("talk skipped")
    step("sortie")
    # the mission loads (about 12 s): wait until the tester AC exists, then a moment more (zeroing AP during the load
    # froze the game once)
    kill = [sys.executable, os.path.join(HERE, "kill_enemies.py")]
    if not until(lambda: "AP 12300/12300" in subprocess.run(kill, capture_output=True, text=True).stdout, 60, 1, False):
        sys.exit("tester AC not found")
    time.sleep(2)
    mark = log_size()
    if "AP 0 ->" not in subprocess.run(kill + ["--max-ap", "12300"], capture_output=True, text=True).stdout:
        sys.exit("tester AC not zeroed")
    step("tester AC killed")
    if not wait_log("5074 rose", mark, 60): sys.exit("mission not cleared")
    step("mission cleared")
    if "--no-results" in sys.argv: return
    mark = log_size()
    for _ in range(40):  # Fermer once the results screen takes clicks; stop at the grid, or at the commit when there
        click(FERMER)      # is no part to show
        if wait_log("results:", mark, 1.5): step("reward grid open"); return
        if wait_log("defer 1->0", mark, 0): step("no reward grid: committed"); return
    sys.exit("results screen not closed")

if __name__ == "__main__":
    main()
