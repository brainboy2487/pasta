# ═══════════════════════════════════════════════════════════════════════════
# stdlib/time.ph — PASTA Time and Timing Library
# ═══════════════════════════════════════════════════════════════════════════
# Version: 1.0
#
# Provides timing utilities for games, benchmarks, and scheduling.
# Uses native TIME_MS() and SLEEP() builtins.
#
# ═══════════════════════════════════════════════════════════════════════════

set __header_time = "time v1.0 loaded"

# ───────────────────────────────────────────────────────────────────────────
# TIMING FUNCTIONS (wrapper-free using builtins directly)
# ───────────────────────────────────────────────────────────────────────────

# Get current time in milliseconds - use TIME_MS() builtin directly
# Get elapsed time between two timestamps
DEF t_elapsed(start, end):
    RETURN end - start
END

# Create a timer object (returns start time)
DEF t_timer_start():
    RET.NOW(): TIME_MS()
END

# Get elapsed time since timer started
DEF t_timer_elapsed(start):
    set now = TIME_MS()
    RETURN now - start
END

# Check if duration has passed since start
DEF t_timer_expired(start, duration_ms):
    set now = TIME_MS()
    IF now - start >= duration_ms:
        RETURN True
    END
    RETURN False
END

# ───────────────────────────────────────────────────────────────────────────
# FRAME TIMING (for game loops)
# ───────────────────────────────────────────────────────────────────────────

# Calculate frames per second from frame time
DEF t_fps_from_delta(delta_ms):
    IF delta_ms <= 0:
        RETURN 0
    END
    RETURN 1000 / delta_ms
END

# Calculate target frame time for desired FPS
DEF t_frame_time(target_fps):
    IF target_fps <= 0:
        RETURN 16
    END
    RETURN 1000 / target_fps
END

# Sleep to maintain target frame rate
DEF t_frame_limit(frame_start, target_fps):
    set target_time = t_frame_time(target_fps)
    set elapsed = TIME_MS() - frame_start
    IF elapsed < target_time:
        set wait = target_time - elapsed
        SLEEP(wait)
    END
END

# ───────────────────────────────────────────────────────────────────────────
# BENCHMARK UTILITIES
# ───────────────────────────────────────────────────────────────────────────

# Measure execution time of a code block (returns elapsed ms)
# Usage: wrap your code between benchmark_start/end
DEF t_bench_start():
    RET.NOW(): TIME_MS()
END

DEF t_bench_end(start):
    set elapsed = TIME_MS() - start
    PRINT "[BENCH] Elapsed:", elapsed, "ms"
    RETURN elapsed
END

# ───────────────────────────────────────────────────────────────────────────
# TIME FORMATTING
# ───────────────────────────────────────────────────────────────────────────

# Format milliseconds as seconds with decimal
DEF t_format_seconds(ms):
    RETURN ms / 1000
END

# Format as MM:SS
DEF t_format_mmss(ms):
    set total_sec = floor(ms / 1000)
    set minutes = floor(total_sec / 60)
    set seconds = total_sec - minutes * 60
    RETURN [minutes, seconds]
END

# Format as HH:MM:SS  
DEF t_format_hhmmss(ms):
    set total_sec = floor(ms / 1000)
    set hours = floor(total_sec / 3600)
    set remaining = total_sec - hours * 3600
    set minutes = floor(remaining / 60)
    set seconds = remaining - minutes * 60
    RETURN [hours, minutes, seconds]
END

# ═══════════════════════════════════════════════════════════════════════════
# END OF TIME LIBRARY
# ═══════════════════════════════════════════════════════════════════════════
