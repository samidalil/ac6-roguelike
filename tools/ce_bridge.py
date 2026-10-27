"""ce_bridge: execute Lua inside the running (non-elevated) Cheat Engine from Python, MCP-style.
Server side (once per CE session, in CE's Lua Engine):   openLuaServer("CELUASERVER")
Client side: loads CE's luaclient-x86_64.dll and calls CELUA_ExecuteFunction(script, param).
The script runs as a function body inside CE (`parameter` is its argument, return value -> integer).
Text results: the script writes them to RESULT (a file), which run() returns.
Usage:  python ce_bridge.py 'return 42'          -> prints the integer
        python ce_bridge.py path/to/file.lua      -> dofile inside CE, captures ac6.log output since start
"""
import ctypes, os, sys, time
DLL = r"C:\Program Files\Cheat Engine 7.5\luaclient-x86_64.dll"
LOG = os.path.join(os.environ["USERPROFILE"], "ac6_re_log.txt")
_lib = None
def connect(name="CELUASERVER"):
    global _lib
    if _lib is None:
        _lib = ctypes.WinDLL(DLL)
        _lib.CELUA_Initialize.argtypes = [ctypes.c_char_p]; _lib.CELUA_Initialize.restype = ctypes.c_int
        _lib.CELUA_ExecuteFunction.argtypes = [ctypes.c_char_p, ctypes.c_uint64]; _lib.CELUA_ExecuteFunction.restype = ctypes.c_uint64
        _lib.CELUA_ExecuteFunctionAsync.argtypes = [ctypes.c_char_p, ctypes.c_uint64]; _lib.CELUA_ExecuteFunctionAsync.restype = ctypes.c_uint64
    ok = _lib.CELUA_Initialize(name.encode())
    if not ok: raise SystemExit("CELUA_Initialize failed: run  openLuaServer(\"%s\")  in CE's Lua Engine first" % name)
    return _lib
def lua(script, param=0, wait=True):
    """run `script` (a function body) inside CE; returns the integer it returns"""
    lib = connect()
    fn = lib.CELUA_ExecuteFunction if wait else lib.CELUA_ExecuteFunctionAsync
    return fn(script.encode("utf-8"), param)
def run_file(path, timeout=30):
    """dofile(path) inside CE and return the ac6_re_log.txt lines appended meanwhile"""
    start = os.path.getsize(LOG) if os.path.exists(LOG) else 0
    p = path.replace("\\", "/")
    rc = lua("local ok, err = pcall(dofile, [[%s]]); if not ok then ac6.log('BRIDGE ERROR: %%s', tostring(err)) end; return ok and 1 or 0" % p)
    time.sleep(0.2)
    with open(LOG, "rb") as f: f.seek(start); out = f.read().decode("utf-8", "replace")
    return rc, out
if __name__ == "__main__":
    arg = sys.argv[1] if len(sys.argv) > 1 else "return 1"
    if os.path.exists(arg):
        rc, out = run_file(os.path.abspath(arg)); print(out.rstrip()); print("[rc=%d]" % rc)
    else:
        print(lua(arg))
