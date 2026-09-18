# ═══════════════════════════════════════════════════════════════════════════
# stdlib/math.ph — PASTA Mathematics Library
# ═══════════════════════════════════════════════════════════════════════════
# Version: 1.0
# 
# This module provides mathematical constants and functions.
# All functions use native builtins where available for performance.
#
# Usage:
#   FROM math USE: sin, cos, pi END
#   result = sin(pi / 4)
#
# ═══════════════════════════════════════════════════════════════════════════

set __header_math = "math v1.0 loaded"

# ───────────────────────────────────────────────────────────────────────────
# MATHEMATICAL CONSTANTS
# ───────────────────────────────────────────────────────────────────────────

set math.pi    = 3.141592653589793
set math.e     = 2.718281828459045
set math.tau   = 6.283185307179586
set math.phi   = 1.618033988749895
set math.sqrt2 = 1.4142135623730951
set math.sqrt3 = 1.7320508075688772
set math.ln2   = 0.6931471805599453
set math.ln10  = 2.302585092994046

set PI  = 3.141592653589793
set TAU = 6.283185307179586
set E   = 2.718281828459045

# ───────────────────────────────────────────────────────────────────────────
# BASIC ARITHMETIC FUNCTIONS
# ───────────────────────────────────────────────────────────────────────────

DEF m_abs(x):
    IF x < 0:
        RETURN 0 - x
    END
    RETURN x
END

DEF m_sign(x):
    IF x > 0:
        RETURN 1
    END
    IF x < 0:
        RETURN 0 - 1
    END
    RETURN 0
END

DEF m_clamp(v, lo, hi):
    IF v < lo:
        RETURN lo
    END
    IF v > hi:
        RETURN hi
    END
    RETURN v
END

DEF m_min(a, b):
    IF a < b:
        RETURN a
    END
    RETURN b
END

DEF m_max(a, b):
    IF a > b:
        RETURN a
    END
    RETURN b
END

DEF m_min_list(lst):
    set result = lst[0]
    FOR item IN lst:
        IF item < result:
            set result = item
        END
    END
    RETURN result
END

DEF m_max_list(lst):
    set result = lst[0]
    FOR item IN lst:
        IF item > result:
            set result = item
        END
    END
    RETURN result
END

# ───────────────────────────────────────────────────────────────────────────
# ROUNDING FUNCTIONS
# ───────────────────────────────────────────────────────────────────────────

DEF m_floor(x):
    RET.NOW(): floor(x)
END

DEF m_ceil(x):
    set f = floor(x)
    IF x > f:
        RETURN f + 1
    END
    RETURN f
END

DEF m_round(x):
    RETURN floor(x + 0.5)
END

DEF m_round_to(x, decimals):
    set factor = 1
    FOR i IN range(decimals):
        set factor = factor * 10
    END
    RETURN floor(x * factor + 0.5) / factor
END

DEF m_trunc(x):
    IF x >= 0:
        RETURN floor(x)
    END
    RETURN m_ceil(x)
END

# ───────────────────────────────────────────────────────────────────────────
# MODULAR ARITHMETIC
# ───────────────────────────────────────────────────────────────────────────

DEF m_mod(a, b):
    RETURN a - floor(a / b) * b
END

DEF m_gcd(a, b):
    set x = m_abs(a)
    set y = m_abs(b)
    WHILE y > 0:
        set temp = y
        set y = m_mod(x, y)
        set x = temp
    END
    RETURN x
END

DEF m_lcm(a, b):
    IF a == 0 OR b == 0:
        RETURN 0
    END
    RETURN m_abs(a * b) / m_gcd(a, b)
END

DEF m_factorial(n):
    IF n <= 1:
        RETURN 1
    END
    set result = 1
    FOR i IN range(n):
        set result = result * (i + 1)
    END
    RETURN result
END

DEF m_binomial(n, k):
    IF k > n OR k < 0:
        RETURN 0
    END
    IF k == 0 OR k == n:
        RETURN 1
    END
    RETURN m_factorial(n) / (m_factorial(k) * m_factorial(n - k))
END

# ───────────────────────────────────────────────────────────────────────────
# POWER AND ROOT FUNCTIONS
# ───────────────────────────────────────────────────────────────────────────

DEF m_pow(base, exp):
    RETURN base ** exp
END

DEF m_sqrt(x):
    IF x < 0:
        RETURN 0 - 1
    END
    IF x == 0:
        RETURN 0
    END
    set guess = x / 2
    FOR i IN range(20):
        set guess = (guess + x / guess) / 2
    END
    RETURN guess
END

DEF m_cbrt(x):
    IF x < 0:
        RETURN 0 - m_pow(0 - x, 1/3)
    END
    RETURN m_pow(x, 1/3)
END

DEF m_hypot(a, b):
    RETURN m_sqrt(a*a + b*b)
END

DEF m_dist(x1, y1, x2, y2):
    set dx = x2 - x1
    set dy = y2 - y1
    RETURN m_sqrt(dx*dx + dy*dy)
END

# ───────────────────────────────────────────────────────────────────────────
# TRIGONOMETRIC FUNCTIONS
# ───────────────────────────────────────────────────────────────────────────

DEF m_sin(x):
    set pi = 3.141592653589793
    set x = m_mod(x + pi, 2 * pi) - pi
    
    set result = x
    set term = x
    set x2 = x * x
    
    FOR n IN range(1, 12):
        set k = 2 * n
        set term = 0 - term * x2 / (k * (k + 1))
        set result = result + term
    END
    RETURN result
END

DEF m_cos(x):
    set pi = 3.141592653589793
    RETURN m_sin(x + pi/2)
END

DEF m_tan(x):
    set c = m_cos(x)
    IF m_abs(c) < 0.0000001:
        RETURN 999999999
    END
    RETURN m_sin(x) / c
END

# ───────────────────────────────────────────────────────────────────────────
# INVERSE TRIGONOMETRIC FUNCTIONS
# ───────────────────────────────────────────────────────────────────────────

DEF m_atan(x):
    set pi_2 = 1.5707963267948966
    
    IF x > 1:
        RETURN pi_2 - m_atan(1/x)
    END
    IF x < -1:
        RETURN 0 - pi_2 - m_atan(1/x)
    END
    
    set result = x
    set term = x
    set x2 = x * x
    
    FOR n IN range(1, 20):
        set term = 0 - term * x2
        set result = result + term / (2 * n + 1)
    END
    RETURN result
END

DEF m_atan2(y, x):
    set pi = 3.141592653589793
    
    IF x > 0:
        RETURN m_atan(y / x)
    END
    IF x < 0:
        IF y >= 0:
            RETURN m_atan(y / x) + pi
        END
        RETURN m_atan(y / x) - pi
    END
    IF y > 0:
        RETURN pi / 2
    END
    IF y < 0:
        RETURN 0 - pi / 2
    END
    RETURN 0
END

# ───────────────────────────────────────────────────────────────────────────
# ANGLE CONVERSION
# ───────────────────────────────────────────────────────────────────────────

DEF m_degrees(radians):
    RETURN radians * 180 / 3.141592653589793
END

DEF m_radians(deg):
    RETURN deg * 3.141592653589793 / 180
END

# ───────────────────────────────────────────────────────────────────────────
# EXPONENTIAL AND LOGARITHMIC FUNCTIONS
# ───────────────────────────────────────────────────────────────────────────

DEF m_exp(x):
    set result = 1
    set term = 1
    
    FOR n IN range(1, 30):
        set term = term * x / n
        set result = result + term
    END
    RETURN result
END

DEF m_ln(x):
    IF x <= 0:
        RETURN 0 - 999999999
    END
    
    set scale = 0
    set y = x
    WHILE y > 2:
        set y = m_sqrt(y)
        set scale = scale + 1
    END
    WHILE y < 0.5:
        set y = y * 2.718281828459045
        set scale = scale - 1
    END
    
    set t = y - 1
    set result = 0
    set term = t
    
    FOR n IN range(1, 30):
        IF m_mod(n, 2) == 1:
            set result = result + term / n
        ELSE:
            set result = result - term / n
        END
        set term = term * t
    END
    
    set result = result * m_pow(2, scale)
    RETURN result
END

DEF m_log10(x):
    RETURN m_ln(x) / 2.302585092994046
END

DEF m_log2(x):
    RETURN m_ln(x) / 0.6931471805599453
END

# ───────────────────────────────────────────────────────────────────────────
# INTERPOLATION FUNCTIONS
# ───────────────────────────────────────────────────────────────────────────

DEF m_lerp(a, b, t):
    RETURN a + (b - a) * t
END

DEF m_inv_lerp(a, b, v):
    IF a == b:
        RETURN 0
    END
    RETURN (v - a) / (b - a)
END

DEF m_remap(v, in_min, in_max, out_min, out_max):
    set t = m_inv_lerp(in_min, in_max, v)
    RETURN m_lerp(out_min, out_max, t)
END

DEF m_smoothstep(edge0, edge1, x):
    set t = m_clamp((x - edge0) / (edge1 - edge0), 0, 1)
    RETURN t * t * (3 - 2 * t)
END

# ───────────────────────────────────────────────────────────────────────────
# STATISTICAL FUNCTIONS
# ───────────────────────────────────────────────────────────────────────────

DEF m_sum(lst):
    set total = 0
    FOR x IN lst:
        set total = total + x
    END
    RETURN total
END

DEF m_mean(lst):
    RETURN m_sum(lst) / len(lst)
END

DEF m_variance(lst):
    set m = m_mean(lst)
    set total = 0
    FOR x IN lst:
        set diff = x - m
        set total = total + diff * diff
    END
    RETURN total / len(lst)
END

DEF m_std(lst):
    RETURN m_sqrt(m_variance(lst))
END

# ───────────────────────────────────────────────────────────────────────────
# 2D VECTOR MATH
# ───────────────────────────────────────────────────────────────────────────

DEF m_vec2(x, y):
    RETURN [x, y]
END

DEF m_vec2_add(a, b):
    RETURN [a[0] + b[0], a[1] + b[1]]
END

DEF m_vec2_sub(a, b):
    RETURN [a[0] - b[0], a[1] - b[1]]
END

DEF m_vec2_scale(v, s):
    RETURN [v[0] * s, v[1] * s]
END

DEF m_vec2_dot(a, b):
    RETURN a[0] * b[0] + a[1] * b[1]
END

DEF m_vec2_len(v):
    RETURN m_sqrt(v[0] * v[0] + v[1] * v[1])
END

DEF m_vec2_normalize(v):
    set m = m_vec2_len(v)
    IF m == 0:
        RETURN [0, 0]
    END
    RETURN [v[0] / m, v[1] / m]
END

DEF m_vec2_rotate(v, angle):
    set c = m_cos(angle)
    set s = m_sin(angle)
    RETURN [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
END

# ═══════════════════════════════════════════════════════════════════════════
# END OF MATH LIBRARY
# ═══════════════════════════════════════════════════════════════════════════
