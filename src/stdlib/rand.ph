# stdlib/rand.ph — Random number generation
#
# All rand.* symbols are implemented as Rust builtins in executor.rs.
# This file only sets the header sentinel so scripts can detect that
# the rand module has been initialised.
#
# Available builtins (no import required):
#   rand.int([lo [, hi]])  -> number   random integer
#   rand.float()           -> number   uniform float in [0, 1)
#   rand.range(lo, hi)     -> number   float in [lo, hi)
#   rand.seed(n)                       seed the RNG (no-op currently)
#   rand.choice(list)      -> value    random element from list
#   rand.ls(n)             -> list     list of n random floats in [0,1)
#   rand.shuffle(list)     -> list     new list with shuffled order
#   rand.sample(list, k)   -> list     k unique random elements

set __header_rand = "rand loaded"
