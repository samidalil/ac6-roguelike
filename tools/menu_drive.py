"""Shared input helpers for the test scripts (title_to_garage.py, garage_to_mission_success.py): a recorded sequence of
menu clicks (keys where there is nothing to click), the game window brought to the foreground before every key, and the DLL log as the only
state read (save loaded, sortie, mission cleared, results), so the slow steps go on as soon as they are done; screen-pixel
probes (1920x1080) wherever a screen must be up before the next input (each wait has a timeout and exits with an
error)."""
import ctypes, os, subprocess, sys, time

user32, gdi32 = ctypes.windll.user32, ctypes.windll.gdi32
user32.SetProcessDPIAware()
LOG = os.path.join(os.environ["USERPROFILE"], "ac6_roguelike_dll.log")
HERE = os.path.dirname(os.path.abspath(__file__))

def game_window():
    out = subprocess.run(["tasklist", "/FI", "IMAGENAME eq armoredcore6.exe", "/FO", "CSV", "/NH"],
                         capture_output=True, text=True).stdout
    if '","' not in out: return None
    pid = int(out.split('","')[1])
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
    """Bring the game to the foreground (another window on top would get the keys). A tapped Alt lets
    SetForegroundWindow through Windows' foreground lock."""
    global GAME
    if GAME is None: GAME = game_window()
    if GAME is None: sys.exit("game window not found")
    for _ in range(10):
        if user32.GetForegroundWindow() == GAME: return
        user32.keybd_event(0x12, 0, 0, 0); user32.keybd_event(0x12, 0, 2, 0)
        user32.ShowWindow(ctypes.c_void_p(GAME), 9)  # SW_RESTORE
        user32.SetForegroundWindow(ctypes.c_void_p(GAME)); time.sleep(0.3)
    sys.exit("could not bring the game to the foreground")

PARK = (923, 1042)  # mouse rest spot (no menu entry: the probes read unhovered colours)
def click(p, settle=0.3):
    """Menu click: move the cursor onto the entry, let the game hover it (a click on the frame the cursor arrives only
    highlights it), then a held left click; the cursor goes back to PARK."""
    focus()
    user32.SetCursorPos(p[0] - 3, p[1]); time.sleep(0.05)
    user32.SetCursorPos(*p); time.sleep(settle)
    user32.mouse_event(2, 0, 0, 0, 0); time.sleep(0.15)
    user32.mouse_event(4, 0, 0, 0, 0); time.sleep(0.1)
    user32.SetCursorPos(*PARK)

# virtual key, scan code (arrows are extended keys)
VK = {"esc": (0x1B, 0x01, 0), "enter": (0x0D, 0x1C, 0), "down": (0x28, 0x50, 1), "up": (0x26, 0x48, 1),
      "left": (0x25, 0x4B, 1), "right": (0x27, 0x4D, 1)}
def key(name, hold=0.2, after=0.0):
    """Hold a key (the game ignores taps shorter than a frame or two), then wait `after` seconds."""
    vk, sc, ext = VK[name]
    focus()
    user32.keybd_event(vk, sc, ext, 0); time.sleep(hold)
    user32.keybd_event(vk, sc, ext | 2, 0); time.sleep(after)

def play(seq):
    """seq: (key, seconds to wait after it) pairs."""
    for k, after in seq: key(k, after=after)

def log_size():
    return os.path.getsize(LOG) if os.path.exists(LOG) else 0
def wait_log(text, since, timeout):
    end = time.time() + timeout
    while True:
        with open(LOG, encoding="utf-8", errors="replace") as f:
            f.seek(since)
            if text in f.read(): return True
        if time.time() >= end: return False
        time.sleep(0.1)

def step(msg): print(time.strftime("%H:%M:%S"), msg, flush=True)

def pixel(p):
    dc = user32.GetDC(0); c = gdi32.GetPixel(dc, *p); user32.ReleaseDC(0, dc)
    return c & 0xFF, (c >> 8) & 0xFF, (c >> 16) & 0xFF
luma = lambda c: (c[0] * 3 + c[1] * 6 + c[2]) / 10
def on_title_menu():
    """The title menu is up and CONTINUER is selected: its text is bright (dimmed under the warning dialog)."""
    return luma(pixel((960, 761))) > 120
def on_warning():
    """The "session not ended properly" dialog: its blue-grey panel over the black title screen."""
    c = pixel((700, 560))
    return luma(c) > 40 and c[2] > c[0] + 10
def on_garage_menu():
    """The garage menu has faded in (about 3 s after "garage opened"): DESIGN D'AC, selected at a fresh load, is lit."""
    return luma(pixel((300, 155))) > 80
def on_deploy_menu():
    """DÉPLOIEMENT is open (its entry stays bright while its submenu shows MISSIONS / ENTRAÎNEMENT / ESSAI AC)."""
    return luma(pixel((300, 345))) > 200
MISSION_ROW = lambda n: (396, 240 + 66 * n)
def on_mission_list():
    """The mission list is up: its first row's panel is lit."""
    return luma(pixel(MISSION_ROW(0))) > 100
def on_mission_menu():
    """The mission menu is up: its Déploiement button lit (selected by default), the AC view's lower right dark."""
    return 130 < luma(pixel((311, 222))) < 200 and luma(pixel((1300, 600))) < 80
def until(cond, timeout, every=0.2, screen=True):
    """Poll `cond` until true or `timeout` s. screen: it reads the screen, so the game is kept in front."""
    end = time.time() + timeout
    while True:
        if screen: focus()
        if cond(): return True
        if time.time() >= end: return False
        time.sleep(every)
