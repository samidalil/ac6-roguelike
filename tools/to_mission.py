"""to_mission.py [--garage] [--row N] [--kill] [--results]: drive the running game from the title screen into a mission, no
screenshots: real clicks (SendInput) at fixed 1920x1080 positions, held keys, the DLL log for the slow steps
(save loaded, sortie) and two screen-pixel probes to tell the mission list from the mission menu. The game is brought
to the foreground before every input (the clicks are blind).

  title: OK on the "session not ended properly" warning, CONTINUER -> garage (log "garage opened")
  DÉPLOIEMENT -> MISSIONS -> mission row N (0 = first; 1 = "Destruction de l'AC d'essai" on the test save)
  briefing (first view in a game session only): Escape (pressed until it takes, it is ignored while the briefing
  loads) goes back to the mission list, the mission is selected again -> mission menu -> Déploiement -> OUI -> sortie (log "defer 0->1")
  --kill: once the mission is up, tools/kill_enemies.py --max-ap 12300 (the tester AC) -> mission cleared
  --results: after the clear, Fermer on the results screen -> the reward grid opens

Start it with the game at the title screen (tools/relaunch.ps1, tools/restore_save.ps1 for the test save)."""
import ctypes, os, subprocess, sys, time

user32, gdi32 = ctypes.windll.user32, ctypes.windll.gdi32
user32.SetProcessDPIAware()
LOG = os.path.join(os.environ["USERPROFILE"], "ac6_roguelike_dll.log")
HERE = os.path.dirname(os.path.abspath(__file__))
# screen positions (1920x1080)
WARNING_OK, CONTINUER = (959, 616), (960, 761)
GARAGE_DEPLOY, MISSIONS = (237, 345), (659, 156)
MISSION_ROW = lambda n: (396, 240 + 66 * n)
MENU_DEPLOY, YES = (311, 222), (770, 592)
PARK = (923, 1042)  # mouse rest spot (no button: the probes read unhovered colours)

def game_window():
    pid = int(subprocess.run(["tasklist", "/FI", "IMAGENAME eq armoredcore6.exe", "/FO", "CSV", "/NH"],
                             capture_output=True, text=True).stdout.split('","')[1])
    found = []
    @ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)
    def each(h, _):
        p = ctypes.c_ulong()
        user32.GetWindowThreadProcessId(ctypes.c_void_p(h), ctypes.byref(p))
        if p.value == pid and user32.IsWindowVisible(ctypes.c_void_p(h)): found.append(h)
        return True
    user32.EnumWindows(each, None)
    return found[0] if found else None
GAME = None

def focus():
    """Bring the game to the foreground (clicks are blind: another window on top would get them). A tapped Alt lets
    SetForegroundWindow through Windows' foreground lock."""
    global GAME
    if GAME is None: GAME = game_window()
    if GAME is None: sys.exit("game window not found")
    if user32.GetForegroundWindow() == GAME: return
    user32.keybd_event(0x12, 0, 0, 0); user32.keybd_event(0x12, 0, 2, 0)
    user32.ShowWindow(ctypes.c_void_p(GAME), 9)  # SW_RESTORE
    user32.SetForegroundWindow(ctypes.c_void_p(GAME)); time.sleep(0.5)
    if user32.GetForegroundWindow() != GAME: sys.exit("could not bring the game to the foreground")

def click(p, hold=0.15):
    focus()
    user32.SetCursorPos(*p); time.sleep(0.1)
    user32.mouse_event(2, 0, 0, 0, 0); time.sleep(hold)
    user32.mouse_event(4, 0, 0, 0, 0); time.sleep(0.1)
    user32.SetCursorPos(*PARK)

VK = {"esc": (0x1B, 0x01), "enter": (0x0D, 0x1C)}
def key(name, hold=0.3):
    vk, sc = VK[name]
    focus()
    user32.keybd_event(vk, sc, 0, 0); time.sleep(hold)
    user32.keybd_event(vk, sc, 2, 0); time.sleep(0.1)

def pixel(p):
    dc = user32.GetDC(0); c = gdi32.GetPixel(dc, *p); user32.ReleaseDC(0, dc)
    return c & 0xFF, (c >> 8) & 0xFF, (c >> 16) & 0xFF
luma = lambda c: (c[0] * 3 + c[1] * 6 + c[2]) / 10

def on_list(row):   # the selected mission row is bright on the list
    return luma(pixel(MISSION_ROW(row))) > 180
def on_menu():      # the mission menu's Déploiement button is lit, the AC view's lower right is dark
    return 130 < luma(pixel(MENU_DEPLOY)) < 200 and luma(pixel((1300, 600))) < 80

def log_size():
    return os.path.getsize(LOG) if os.path.exists(LOG) else 0
def wait_log(text, since, timeout):
    end = time.time() + timeout
    while time.time() < end:
        with open(LOG, encoding="utf-8", errors="replace") as f:
            f.seek(since)
            if text in f.read(): return True
        time.sleep(0.5)
    return False

def step(msg): print(time.strftime("%H:%M:%S"), msg, flush=True)

def main():
    row = int(sys.argv[sys.argv.index("--row") + 1]) if "--row" in sys.argv else 1
    if "--garage" not in sys.argv:  # --garage: the game already shows the garage menu
        mark = log_size()
        click(WARNING_OK); time.sleep(6)
        click(CONTINUER)
        if not wait_log("garage opened", mark, 120): sys.exit("no garage after Continue")
        step("garage"); time.sleep(3)
    click(GARAGE_DEPLOY); time.sleep(2.5)
    click(MISSIONS); time.sleep(2.5)
    click(MISSION_ROW(row)); time.sleep(5)
    if not on_menu():
        # the briefing ignores Escape while it loads: press it until the mission list (or menu) is back
        step("briefing: skip")
        for _ in range(30):
            if on_menu(): break
            if on_list(row): click(MISSION_ROW(row)); time.sleep(3); continue
            key("esc"); time.sleep(3)
    if not on_menu(): sys.exit("mission menu not reached (pixel probe)")
    step("mission menu")
    mark = log_size()
    click(MENU_DEPLOY); time.sleep(2.5)
    click(YES)
    if not wait_log("defer 0->1", mark, 90): sys.exit("no sortie")
    step("sortie")
    if "--kill" in sys.argv:
        time.sleep(25)  # into the mission (zeroing AP during the load froze the game once)
        mark = log_size()
        for _ in range(20):
            out = subprocess.run([sys.executable, os.path.join(HERE, "kill_enemies.py"), "--max-ap", "12300"],
                                 capture_output=True, text=True).stdout
            if "AP 0 ->" in out: break
            time.sleep(2)
        step("tester AC killed")
        if not wait_log("5074 rose", mark, 60): sys.exit("mission not cleared")
        step("mission cleared")
        if "--results" in sys.argv:
            mark = log_size()
            for _ in range(15):  # Fermer once the results screen takes input
                time.sleep(3); key("enter", 0.2)
                if wait_log("results:", mark, 2): break
            step("reward grid open")

if __name__ == "__main__":
    main()
