# stdlib/math.ph — Extended math functions
#
# Constants:  math.pi  math.e  math.tau  math.phi  math.inf  math.nan
# Trig:       math.sin  math.cos  math.tan  math.asin  math.acos  math.atan
#             math.atan2(y,x)
# Exponential: math.exp  math.ln  math.log2  math.log10  math.log(x[,base])
# Numeric:    math.sqrt  math.pow  math.abs  math.floor  math.ceil  math.round
#             math.clamp  math.sign  math.min  math.max  math.hypot
# Conversion: math.degrees  math.radians
# Misc:       math.gcd(a,b)  math.lcm(a,b)  math.factorial(n)  math.is_nan(x)
#             math.is_inf(x)

set __header_math = "math loaded"

# ── numeric constants ─────────────────────────────────────────────────────────
set math.pi  = 3.141592653589793
set math.e   = 2.718281828459045
set math.tau = 6.283185307179586
set math.phi = 1.618033988749895
set math.inf = 1e308
set math.nan = 0.0

# ── trig ─────────────────────────────────────────────────────────────────────
DEF math.sin(x):    RET.NOW(): math.sin(x)    END
DEF math.cos(x):    RET.NOW(): math.cos(x)    END
DEF math.tan(x):    RET.NOW(): math.tan(x)    END
DEF math.asin(x):   RET.NOW(): math.asin(x)   END
DEF math.acos(x):   RET.NOW(): math.acos(x)   END
DEF math.atan(x):   RET.NOW(): math.atan(x)   END
DEF math.atan2(y, x):  RET.NOW(): math.atan2(y, x)  END

# ── exponential ───────────────────────────────────────────────────────────────
DEF math.exp(x):    RET.NOW(): math.exp(x)    END
DEF math.ln(x):     RET.NOW(): math.ln(x)     END
DEF math.log2(x):   RET.NOW(): math.log2(x)   END
DEF math.log10(x):  RET.NOW(): math.log10(x)  END
DEF math.log(x):    RET.NOW(): math.log(x)    END

# ── numeric ───────────────────────────────────────────────────────────────────
DEF math.sqrt(x):         RET.NOW(): math.sqrt(x)             END
DEF math.abs(x):          RET.NOW(): math.abs(x)              END
DEF math.floor(x):        RET.NOW(): math.floor(x)            END
DEF math.ceil(x):         RET.NOW(): math.ceil(x)             END
DEF math.round(x):        RET.NOW(): math.round(x)            END
DEF math.sign(x):         RET.NOW(): math.sign(x)             END
DEF math.pow(b, e):       RET.NOW(): math.pow(b, e)           END
DEF math.min(a, b):       RET.NOW(): math.min(a, b)           END
DEF math.max(a, b):       RET.NOW(): math.max(a, b)           END
DEF math.hypot(a, b):     RET.NOW(): math.hypot(a, b)         END
DEF math.clamp(v, lo, hi): RET.NOW(): math.clamp(v, lo, hi)  END

# ── conversion ────────────────────────────────────────────────────────────────
DEF math.degrees(r):  RET.NOW(): math.degrees(r)  END
DEF math.radians(d):  RET.NOW(): math.radians(d)  END

# ── extended ─────────────────────────────────────────────────────────────────
DEF math.gcd(a, b):       RET.NOW(): math.gcd(a, b)       END
DEF math.lcm(a, b):       RET.NOW(): math.lcm(a, b)       END
DEF math.factorial(n):    RET.NOW(): math.factorial(n)    END
DEF math.is_nan(x):       RET.NOW(): math.is_nan(x)       END
DEF math.is_inf(x):       RET.NOW(): math.is_inf(x)       END
