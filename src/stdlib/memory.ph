# stdlib/memory.ph — Low-level memory allocation helpers
#
# In the PASTA managed runtime, these are cooperative hints.
# The GC handles actual reclamation; alloc/free guide layout.
#
# Exports:
#   memory.alloc(n)        -> handle string  allocate n bytes
#   memory.free(handle)                      release handle
#   memory.copy(src, dst)                    copy contents
#   memory.set(handle, v)                    fill with byte value
#   memory.size(handle)    -> number         size hint
#   memory.buffer(n)       -> handle         typed byte buffer

set __header_memory = "memory loaded"

DEF memory.alloc(n):         RET.NOW(): memory.alloc(n)        END
DEF memory.free(handle):     memory.free(handle)               END
DEF memory.copy(src, dst):   memory.copy(src, dst)             END
DEF memory.set(handle, v):   memory.set(handle, v)             END
DEF memory.size(handle):     RET.NOW(): memory.size(handle)    END
DEF memory.buffer(n):        RET.NOW(): memory.buffer(n)       END
