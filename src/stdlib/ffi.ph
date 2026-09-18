# stdlib/ffi.ph — Foreign Function Interface
#
# NOTE: Requires the "ffi" Cargo feature and a compatible shared library.
# Stubs return errors when the feature is absent.
#
# Exports:
#   ffi.load(path)          -> handle   load shared library
#   ffi.symbol(lib, name)   -> handle   get function pointer
#   ffi.call(sym, args)     -> value    call foreign function
#   ffi.close(lib)                      unload library

set __header_ffi = "ffi loaded"

DEF ffi.load(path):           RET.NOW(): ffi.load(path)             END
DEF ffi.symbol(lib, name):    RET.NOW(): ffi.symbol(lib, name)      END
DEF ffi.call(sym, args):      RET.NOW(): ffi.call(sym, args)        END
DEF ffi.close(lib):           ffi.close(lib)                        END
