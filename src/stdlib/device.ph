# stdlib/device.ph — Hardware probing
#
# Exports:
#   device.arch()      -> string   "x86_64" | "aarch64" | "arm" | "wasm32" | "unknown"
#   device.cpu()       -> string   "cpu"
#   device.cores()     -> number   logical CPU count
#   device.ram()       -> number   total RAM bytes (0 if unavailable)
#   device.gpu()       -> string   "none" or device name
#   device.features()  -> list     CPU feature strings (avx2, sse4.1, neon …)
#   device.name()      -> string   friendly device name

set __header_device = "device loaded"

DEF device.arch():      RET.NOW(): device.arch()      END
DEF device.cpu():       RET.NOW(): device.cpu()       END
DEF device.cores():     RET.NOW(): device.cores()     END
DEF device.ram():       RET.NOW(): device.ram()       END
DEF device.gpu():       RET.NOW(): device.gpu()       END
DEF device.features():  RET.NOW(): device.features()  END
DEF device.name():      RET.NOW(): device.name()      END
