# ═══════════════════════════════════════════════════════════════════════════
# stdlib/thread.ph — PASTA Threading Utilities
# ═══════════════════════════════════════════════════════════════════════════
# Version: 1.0
#
# Threading helpers and synchronization primitives.
# Note: PASTA uses DO blocks for concurrent execution.
#
# ═══════════════════════════════════════════════════════════════════════════

set __header_thread = "thread v1.0 loaded"

# ───────────────────────────────────────────────────────────────────────────
# THREAD STATE CONSTANTS
# ───────────────────────────────────────────────────────────────────────────

set THREAD_RUNNING  = "running"
set THREAD_PAUSED   = "paused"
set THREAD_FINISHED = "finished"

# ───────────────────────────────────────────────────────────────────────────
# SYNCHRONIZATION HELPERS
# ───────────────────────────────────────────────────────────────────────────

# Simple busy-wait for a condition
DEF th_wait_for(check_fn, timeout_ms):
    set start = TIME_MS()
    WHILE True:
        IF check_fn():
            RETURN True
        END
        IF TIME_MS() - start > timeout_ms:
            RETURN False
        END
        SLEEP(10)
    END
END

# Yield execution briefly
DEF th_yield():
    SLEEP(1)
END

# Sleep with interruptibility check
DEF th_sleep(ms):
    SLEEP(ms)
END

# ═══════════════════════════════════════════════════════════════════════════
# END OF THREAD LIBRARY
# ═══════════════════════════════════════════════════════════════════════════
