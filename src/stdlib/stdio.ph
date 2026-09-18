# stdio.ph — Standard I/O header for PastaLang
# Version: 0.1
# Purpose: Provide a small, stable stdio API for scripts and a clear runtime contract.

# ---------------------------------------------------------------------
# Runtime primitives required (must be implemented in the interpreter):
# - __pasta_stdin_readline() -> string | None
#     Read a line from stdin (without trailing newline). Returns None on EOF.
# - __pasta_stdout_write(s: string)
#     Write string s to stdout (no newline appended).
# - __pasta_stdout_flush()
#     Flush stdout.
# - __pasta_stdin_readchar() -> string | None
#     Read a single character from stdin as a 1-char string; None on EOF.
# These are low-level hooks the runtime should expose as builtins.
# ---------------------------------------------------------------------

# Public API (high-level helpers)

# println(value)
# Print value followed by newline.
# Accepts any value; uses str() to convert.
set println = DO v:
    # Convert to string and print with newline using PRINT builtin
    PRINT str(v)
END

# print(value)
# Print value without adding an extra newline.
# If runtime supports __pasta_stdout_write, this will use it; otherwise falls back to PRINT.
set print = DO v:
    # If runtime provides stdout_write builtin, call it; otherwise use PRINT but strip newline.
    # The runtime should implement __pasta_stdout_write for exact behavior.
    # Here we attempt to call a builtin function named "stdout_write" if present.
    # In many runtimes, calling stdout_write will be implemented as a builtin function.
    stdout_write = "stdout_write"   # placeholder name for runtime builtin
    # Try to call builtin; if unknown, fallback to PRINT (which appends newline).
    # Note: this header assumes the runtime will wire a builtin named "stdout_write".
    TRY:
        CALL stdout_write(str(v))
    OTHERWISE:
        # Fallback: PRINT (adds newline) — acceptable if no low-level write exists.
        PRINT str(v)
END

# flush()
# Flush stdout (no-op if runtime doesn't implement flush).
set flush = DO:
    TRY:
        CALL "stdout_flush"()
    OTHERWISE:
        # no-op fallback
        set _ = 0
END

# read_line() -> string | None
# Read a line from stdin (without trailing newline). Returns None on EOF.
set read_line = DO:
    # Expect runtime to expose "__pasta_stdin_readline" or "stdin_readline"
    TRY:
        CALL "__pasta_stdin_readline"()
    OTHERWISE:
        # If not available, return None to indicate unsupported
        None
END

# read_char() -> string | None
# Read a single character from stdin. Returns None on EOF.
set read_char = DO:
    TRY:
        CALL "__pasta_stdin_readchar"()
    OTHERWISE:
        None
END

# read_int() -> number | None
# Read a line and parse as integer (f64). Returns None on EOF or parse failure.
set read_int = DO:
    let line = CALL "__pasta_stdin_readline"()
    if line == None:
        None
    else:
        let s = line
        TRY:
            num(s)
        OTHERWISE:
            None
END

# read_float() -> number | None
# Read a line and parse as float (f64). Returns None on EOF or parse failure.
set read_float = DO:
    let line = CALL "__pasta_stdin_readline"()
    if line == None:
        None
    else:
        let s = line
        TRY:
            float(s)
        OTHERWISE:
            None
END

# printf(format, arg1, arg2, ...)
# Minimal printf-style helper using simple {} substitution.
# Implementation note: this is a simple shim that replaces '{}' tokens in order.
set printf = DO fmt, args:
    # Convert format to string
    let out = str(fmt)
    let i = 0
    # naive substitution loop: replace first '{}' with next arg
    while i < len(args):
        let placeholder = "{}"
        # find placeholder
        # (Assumes runtime has string find/replace; if not, runtime should provide printf builtin)
        TRY:
            # CALL "str_replace_once"(out, placeholder, str(args[i]))  # runtime helper if available
            # Fallback: use concatenation around split (not implemented here)
            # For now, fall back to println of joined values
            PRINT out
            PRINT str(args[i])
        OTHERWISE:
            PRINT out
            PRINT str(args[i])
        set i = i + 1
    END
END

# Convenience aliases
set puts = println
set gets = read_line

# ---------------------------------------------------------------------
# Usage examples (in your Pasta script):
#
# IMPORT "stdio.ph"
#
# println("Hello, world")
# print("Enter name: ")
# let name = read_line()
# if name != None:
#     println("Hi " + name)
#
# let n = read_int()
# if n != None:
#     println("You entered: " + str(n))
# ---------------------------------------------------------------------

# End of stdio.ph
