# ═══════════════════════════════════════════════════════════════════════════
# stdlib/debug.ph — PASTA Debug and Diagnostics Library
# ═══════════════════════════════════════════════════════════════════════════
# Version: 1.0
#
# Provides debugging utilities, logging, and introspection tools.
#
# ═══════════════════════════════════════════════════════════════════════════

set __header_debug = "debug v1.0 loaded"

# ───────────────────────────────────────────────────────────────────────────
# LOGGING LEVELS
# ───────────────────────────────────────────────────────────────────────────

set DEBUG_LEVEL_TRACE = 0
set DEBUG_LEVEL_DEBUG = 1
set DEBUG_LEVEL_INFO  = 2
set DEBUG_LEVEL_WARN  = 3
set DEBUG_LEVEL_ERROR = 4

set debug.current_level = 1

# ───────────────────────────────────────────────────────────────────────────
# LOGGING FUNCTIONS
# ───────────────────────────────────────────────────────────────────────────

DEF d_log(level_name, msg):
    PRINT "[", level_name, "]", msg
END

DEF d_trace(msg):
    IF debug.current_level <= 0:
        PRINT "[TRACE]", msg
    END
END

DEF d_debug(msg):
    IF debug.current_level <= 1:
        PRINT "[DEBUG]", msg
    END
END

DEF d_info(msg):
    IF debug.current_level <= 2:
        PRINT "[INFO]", msg
    END
END

DEF d_warn(msg):
    IF debug.current_level <= 3:
        PRINT "[WARN]", msg
    END
END

DEF d_error(msg):
    IF debug.current_level <= 4:
        PRINT "[ERROR]", msg
    END
END

# Set minimum log level
DEF d_set_level(level):
    set debug.current_level = level
END

# ───────────────────────────────────────────────────────────────────────────
# ASSERTION AND VALIDATION
# ───────────────────────────────────────────────────────────────────────────

DEF d_assert(condition, msg):
    IF NOT condition:
        PRINT "[ASSERT FAILED]", msg
    END
END

DEF d_assert_eq(a, b, msg):
    IF a != b:
        PRINT "[ASSERT FAILED]", msg, "expected:", b, "got:", a
    END
END

DEF d_assert_ne(a, b, msg):
    IF a == b:
        PRINT "[ASSERT FAILED]", msg, "values should not be equal:", a
    END
END

DEF d_assert_gt(a, b, msg):
    IF NOT (a > b):
        PRINT "[ASSERT FAILED]", msg, a, "should be >", b
    END
END

DEF d_assert_lt(a, b, msg):
    IF NOT (a < b):
        PRINT "[ASSERT FAILED]", msg, a, "should be <", b
    END
END

# ───────────────────────────────────────────────────────────────────────────
# VALUE INSPECTION
# ───────────────────────────────────────────────────────────────────────────

DEF d_inspect(name, value):
    PRINT "[INSPECT]", name, "=", value
END

DEF d_inspect_list(name, lst):
    PRINT "[INSPECT]", name, "= ["
    FOR i IN range(len(lst)):
        PRINT "  [", i, "]:", lst[i]
    END
    PRINT "]"
END

DEF d_type_name(v):
    # Returns a string representation of the type
    # This is a best-effort since PASTA doesn't have typeof
    IF v == True OR v == False:
        RETURN "boolean"
    END
    IF v == None:
        RETURN "none"
    END
    # Try to check if it's a list
    # (This is tricky without typeof, simplified approach)
    RETURN "unknown"
END

# ───────────────────────────────────────────────────────────────────────────
# TIMING DEBUG
# ───────────────────────────────────────────────────────────────────────────

DEF d_time_start(label):
    PRINT "[TIME START]", label
    RET.NOW(time_ms())
END

DEF d_time_end(label, start):
    set elapsed = time_ms() - start
    PRINT "[TIME END]", label, ":", elapsed, "ms"
    RET.NOW(elapsed)
END

# ───────────────────────────────────────────────────────────────────────────
# BREAKPOINT SIMULATION
# ───────────────────────────────────────────────────────────────────────────

DEF d_breakpoint(msg):
    PRINT "═════════════════════════════════════════"
    PRINT "[BREAKPOINT]", msg
    PRINT "═════════════════════════════════════════"
END

DEF d_checkpoint(num):
    PRINT "[CHECKPOINT", num, "]"
END

# ═══════════════════════════════════════════════════════════════════════════
# END OF DEBUG LIBRARY
# ═══════════════════════════════════════════════════════════════════════════
