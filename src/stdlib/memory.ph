# ═══════════════════════════════════════════════════════════════════════════
# stdlib/memory.ph — PASTA Memory and Pointer Utilities
# ═══════════════════════════════════════════════════════════════════════════
# Version: 1.0
#
# Provides utilities for working with the PASTA pointer system.
# Uses ALLOC.MEM, GOTO, PUSH, PULL, SEEK, FREE, SWAP builtins.
#
# ═══════════════════════════════════════════════════════════════════════════

set __header_memory = "memory v1.0 loaded"

# ───────────────────────────────────────────────────────────────────────────
# BUFFER UTILITIES
# ───────────────────────────────────────────────────────────────────────────

# Create a zeroed buffer of size n
DEF mem_zeros(size):
    ALLOC.MEM(size) -> buf
    GOTO buf:
        FOR i IN range(size):
            PUSH.BYTE 0
        END
    END
    RETURN buf
END

# Create a buffer filled with a value
DEF mem_fill(size, value):
    ALLOC.MEM(size) -> buf
    GOTO buf:
        FOR i IN range(size):
            PUSH.BYTE value
        END
    END
    RETURN buf
END

# Copy data from one buffer to another
DEF mem_copy(src, dst, size):
    FOR i IN range(size):
        GOTO src:
            SEEK src, i
            PULL.BYTE -> val
        END
        GOTO dst:
            SEEK dst, i
            PUSH.BYTE val
        END
    END
END

# ───────────────────────────────────────────────────────────────────────────
# BUFFER READING
# ───────────────────────────────────────────────────────────────────────────

# Read a byte at offset
DEF mem_get_byte(buf, offset):
    GOTO buf:
        SEEK buf, offset
        PULL.BYTE -> val
    END
    RETURN val
END

# Write a byte at offset
DEF mem_set_byte(buf, offset, value):
    GOTO buf:
        SEEK buf, offset
        PUSH.BYTE value
    END
END

# Read an int at offset
DEF mem_get_int(buf, offset):
    GOTO buf:
        SEEK buf, offset
        PULL.INT -> val
    END
    RETURN val
END

# Write an int at offset
DEF mem_set_int(buf, offset, value):
    GOTO buf:
        SEEK buf, offset
        PUSH.INT value
    END
END

# ───────────────────────────────────────────────────────────────────────────
# GRID/2D BUFFER UTILITIES (for Game of Life, etc.)
# ───────────────────────────────────────────────────────────────────────────

# Allocate a 2D grid as flat buffer
DEF mem_grid(width, height):
    set size = width * height
    RETURN mem_zeros(size)
END

# Get value from 2D grid
DEF mem_grid_get(grid, width, x, y):
    set offset = y * width + x
    RETURN mem_get_byte(grid, offset)
END

# Set value in 2D grid
DEF mem_grid_set(grid, width, x, y, value):
    set offset = y * width + x
    mem_set_byte(grid, offset, value)
END

# ═══════════════════════════════════════════════════════════════════════════
# END OF MEMORY LIBRARY
# ═══════════════════════════════════════════════════════════════════════════
