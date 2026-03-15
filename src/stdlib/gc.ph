# stdlib/gc.ph — Garbage collector hooks
#
# Exports:
#   gc.collect()   -> number   run GC; returns objects reclaimed
#   gc.count()     -> number   currently live heap objects
#   gc.stats()     -> string   human-readable GC status line
#   gc.pause()               disable automatic GC (future)
#   gc.resume()              re-enable automatic GC (future)

set __header_gc = "gc loaded"

DEF gc.collect():  RET.NOW(): gc.collect()  END
DEF gc.count():    RET.NOW(): gc.count()    END
DEF gc.stats():    RET.NOW(): gc.stats()    END
DEF gc.pause():    gc.pause()               END
DEF gc.resume():   gc.resume()              END
