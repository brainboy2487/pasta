# ═══════════════════════════════════════════════════════════════════════════
# stdlib/tensor.ph — PASTA Tensor/Array Library
# ═══════════════════════════════════════════════════════════════════════════
# Version: 1.0
#
# Provides multi-dimensional array operations and linear algebra basics.
#
# ═══════════════════════════════════════════════════════════════════════════

set __header_tensor = "tensor v1.0 loaded"

set tensor.pi = 3.141592653589793

# ───────────────────────────────────────────────────────────────────────────
# 1D ARRAY OPERATIONS
# ───────────────────────────────────────────────────────────────────────────

# Create array of zeros
DEF arr_zeros(n):
    set result = []
    FOR i IN range(n):
        set result = result + [0]
    END
    RETURN result
END

# Create array of ones
DEF arr_ones(n):
    set result = []
    FOR i IN range(n):
        set result = result + [1]
    END
    RETURN result
END

# Create array filled with value
DEF arr_fill(n, value):
    set result = []
    FOR i IN range(n):
        set result = result + [value]
    END
    RETURN result
END

# Create range array
DEF arr_range(start, stop, step):
    set result = []
    set i = start
    IF step > 0:
        WHILE i < stop:
            set result = result + [i]
            set i = i + step
        END
    ELSE:
        WHILE i > stop:
            set result = result + [i]
            set i = i + step
        END
    END
    RETURN result
END

# Element-wise addition
DEF arr_add(a, b):
    set result = []
    FOR i IN range(len(a)):
        set result = result + [a[i] + b[i]]
    END
    RETURN result
END

# Element-wise subtraction
DEF arr_sub(a, b):
    set result = []
    FOR i IN range(len(a)):
        set result = result + [a[i] - b[i]]
    END
    RETURN result
END

# Element-wise multiplication
DEF arr_mul(a, b):
    set result = []
    FOR i IN range(len(a)):
        set result = result + [a[i] * b[i]]
    END
    RETURN result
END

# Scalar multiplication
DEF arr_scale(arr, scalar):
    set result = []
    FOR i IN range(len(arr)):
        set result = result + [arr[i] * scalar]
    END
    RETURN result
END

# Dot product
DEF arr_dot(a, b):
    set total = 0
    FOR i IN range(len(a)):
        set total = total + a[i] * b[i]
    END
    RETURN total
END

# Sum of array
DEF arr_sum(arr):
    set total = 0
    FOR x IN arr:
        set total = total + x
    END
    RETURN total
END

# Mean of array
DEF arr_mean(arr):
    RETURN arr_sum(arr) / len(arr)
END

# Max of array
DEF arr_max(arr):
    set result = arr[0]
    FOR x IN arr:
        IF x > result:
            set result = x
        END
    END
    RETURN result
END

# Min of array
DEF arr_min(arr):
    set result = arr[0]
    FOR x IN arr:
        IF x < result:
            set result = x
        END
    END
    RETURN result
END

# ───────────────────────────────────────────────────────────────────────────
# 2D MATRIX OPERATIONS
# ───────────────────────────────────────────────────────────────────────────

# Create 2D matrix of zeros (as list of lists)
DEF mat_zeros(rows, cols):
    set result = []
    FOR r IN range(rows):
        set row = []
        FOR c IN range(cols):
            set row = row + [0]
        END
        set result = result + [row]
    END
    RETURN result
END

# Create identity matrix
DEF mat_identity(n):
    set result = []
    FOR r IN range(n):
        set row = []
        FOR c IN range(n):
            IF r == c:
                set row = row + [1]
            ELSE:
                set row = row + [0]
            END
        END
        set result = result + [row]
    END
    RETURN result
END

# Matrix addition
DEF mat_add(a, b):
    set rows = len(a)
    set cols = len(a[0])
    set result = []
    FOR r IN range(rows):
        set row = []
        FOR c IN range(cols):
            set row = row + [a[r][c] + b[r][c]]
        END
        set result = result + [row]
    END
    RETURN result
END

# Matrix scalar multiplication
DEF mat_scale(mat, scalar):
    set rows = len(mat)
    set cols = len(mat[0])
    set result = []
    FOR r IN range(rows):
        set row = []
        FOR c IN range(cols):
            set row = row + [mat[r][c] * scalar]
        END
        set result = result + [row]
    END
    RETURN result
END

# Matrix multiplication
DEF mat_mul(a, b):
    set rows_a = len(a)
    set cols_a = len(a[0])
    set cols_b = len(b[0])
    set result = []
    FOR r IN range(rows_a):
        set row = []
        FOR c IN range(cols_b):
            set sum = 0
            FOR k IN range(cols_a):
                set sum = sum + a[r][k] * b[k][c]
            END
            set row = row + [sum]
        END
        set result = result + [row]
    END
    RETURN result
END

# Matrix transpose
DEF mat_transpose(mat):
    set rows = len(mat)
    set cols = len(mat[0])
    set result = []
    FOR c IN range(cols):
        set row = []
        FOR r IN range(rows):
            set row = row + [mat[r][c]]
        END
        set result = result + [row]
    END
    RETURN result
END

# ═══════════════════════════════════════════════════════════════════════════
# END OF TENSOR LIBRARY
# ═══════════════════════════════════════════════════════════════════════════
