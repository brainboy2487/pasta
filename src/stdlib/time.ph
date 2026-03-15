# stdlib/time.ph — High-resolution timing helpers
#
# Exports:
#   time.now()         -> number   epoch milliseconds (f64)
#   time.now_ns()      -> number   epoch nanoseconds  (f64)
#   time.sleep(ms)                 sleep ms milliseconds
#   time.format(ms)    -> string   human-readable timestamp
#   time.delta(a, b)   -> number   b - a (ms difference)

set __header_time = "time loaded"

DEF time.now():
    RET.NOW(): time.now()
END

DEF time.now_ns():
    RET.NOW(): time.now_ns()
END

DEF time.sleep(ms):
    time.sleep(ms)
END

DEF time.format(ms):
    RET.NOW(): time.format(ms)
END

DEF time.delta(a, b):
    RET.NOW(): b - a
END
