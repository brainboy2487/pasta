# stdlib/thread.ph — Concurrency primitives
#
# PASTA-native concurrency uses DO blocks with OVER priorities.
# thread.* provides lower-level access for interop and diagnostics.
#
# Exports:
#   thread.id()      -> number   OS thread ID of calling thread
#   thread.count()   -> number   available logical CPUs
#   thread.yield()              cooperative yield hint to scheduler
#   thread.sleep(ms)            sleep ms milliseconds
#   thread.spawn(fn)            spawn fn in new thread (stub)
#   thread.join(h)              join thread handle (stub)

set __header_thread = "thread loaded"

DEF thread.id():        RET.NOW(): thread.id()       END
DEF thread.count():     RET.NOW(): thread.count()    END
DEF thread.yield():     thread.yield()               END
DEF thread.sleep(ms):   thread.sleep(ms)             END
DEF thread.spawn(fn):   RET.NOW(): thread.spawn(fn)  END
DEF thread.join(h):     RET.NOW(): thread.join(h)    END
