# stdlib/debug.ph — Diagnostics and introspection
#
# Exports:
#   debug.print(v)             print with [DEBUG] prefix
#   debug.type(v)    -> string  type name of v
#   debug.len(v)     -> number  length of list or string
#   debug.dump()               print entire environment to stdout
#   debug.trace(v)             print with [TRACE] prefix
#   debug.assert(cond[, msg])  error if cond is falsy
#   debug.backtrace()          print current call stack
#   debug.vars()     -> list   list of variable names in current scope

set __header_debug = "debug loaded"

DEF debug.print(v):          debug.print(v)                  END
DEF debug.type(v):           RET.NOW(): debug.type(v)        END
DEF debug.len(v):            RET.NOW(): debug.len(v)         END
DEF debug.dump():            debug.dump()                    END
DEF debug.trace(v):          debug.trace(v)                  END
DEF debug.assert(cond, msg): debug.assert(cond, msg)         END
DEF debug.backtrace():       debug.backtrace()               END
DEF debug.vars():            RET.NOW(): debug.vars()         END
