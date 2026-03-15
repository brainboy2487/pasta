# stdlib/rand.ph — Random number generation
#
# Exports:
#   rand.int([lo [, hi]])  -> number   random integer; no args -> [0,32767]
#   rand.float()           -> number   uniform float in [0, 1)
#   rand.range(lo, hi)     -> number   float in [lo, hi)
#   rand.seed(n)                       seed the RNG (future; no-op now)
#   rand.choice(list)      -> value    random element from list
#   rand.ls(n)             -> list     list of n random floats in [0,1)
#   rand.shuffle(list)     -> list     new list with shuffled order
#   rand.sample(list, k)   -> list     k unique random elements

set __header_rand = "rand loaded"

DEF rand.int():
    RET.NOW(): rand.int()
END

DEF rand.float():
    RET.NOW(): rand.float()
END

DEF rand.range(lo, hi):
    RET.NOW(): rand.range(lo, hi)
END

DEF rand.seed(n):
    rand.seed(n)
END

DEF rand.choice(lst):
    RET.NOW(): rand.choice(lst)
END

DEF rand.ls(n):
    RET.NOW(): rand.ls(n)
END

DEF rand.shuffle(lst):
    RET.NOW(): rand.shuffle(lst)
END

DEF rand.sample(lst, k):
    RET.NOW(): rand.sample(lst, k)
END
