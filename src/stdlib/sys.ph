# stdlib/sys.ph — System and environment access
# Loaded automatically at interpreter startup from stdlib/
#
# Exports:
#   sys.env(name)      -> string   read environment variable
#   sys.exit([code])              exit with optional code (default 0)
#   sys.args()         -> list     command-line arguments as a List
#   sys.platform()     -> string   "linux" | "macos" | "windows"
#   sys.sleep(ms)                  sleep for ms milliseconds
#   sys.getcwd()       -> string   current working directory

set __header_sys = "sys loaded"

# ── constants ────────────────────────────────────────────────────────────────
set SYS_LINUX   = "linux"
set SYS_MACOS   = "macos"
set SYS_WINDOWS = "windows"

# ── function wrappers ─────────────────────────────────────────────────────────
# Each wrapper is a zero-boilerplate passthrough to the builtin.
# Having named DEFs means callers get proper call-site errors.

DEF sys.env(name):
    RET.NOW(): sys.env(name)
END

DEF sys.args():
    RET.NOW(): sys.args()
END

DEF sys.platform():
    RET.NOW(): sys.platform()
END

DEF sys.getcwd():
    RET.NOW(): sys.getcwd()
END

DEF sys.sleep(ms):
    sys.sleep(ms)
END

DEF sys.exit(code):
    sys.exit(code)
END
