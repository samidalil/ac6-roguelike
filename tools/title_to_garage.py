"""title_to_garage.py: from the title screen to the garage menu, keys only. Enter = OK on the "session not ended
properly" warning when it is shown, Enter = CONTINUER once the title menu is up (both pixel probes); done when the DLL
logs "garage opened" and the garage menu has faded in (pixel probe). Start it right after tools/relaunch.ps1 (tools/restore_save.ps1 first for
the test save)."""
import sys
from menu_drive import *

def main():
    focus()
    mark = log_size()
    if not until(lambda: on_warning() or on_title_menu(), 120): sys.exit("title screen not reached (pixel probe)")
    if on_warning():
        step("warning dialog")
        key("enter")
        if not until(lambda: on_title_menu() and not on_warning(), 20): sys.exit("title menu not back after the warning")
    time.sleep(0.5)
    key("enter")  # CONTINUER
    if not wait_log("garage opened", mark, 120): sys.exit("no garage after Continue")
    if not until(on_garage_menu, 30): sys.exit("garage menu not shown (pixel probe)")
    step("garage menu")

if __name__ == "__main__":
    main()
