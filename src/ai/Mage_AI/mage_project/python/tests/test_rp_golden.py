#!/usr/bin/env python3
"""Golden parity test for rp_project using splitmix64 sign-hash.

This script builds a small 3x3 matrix, computes reference RP in Python,
loads `lib/libmage.so` and calls `rp_project`, then compares results.
"""
import ctypes
import os
import sys
from ctypes import c_size_t, c_uint32, c_double, POINTER, Structure

HERE = os.path.dirname(os.path.dirname(__file__))
LIB_PATH = os.path.normpath(os.path.join(HERE, '..', 'lib', 'libmage.so'))
if not os.path.exists(LIB_PATH):
    print('libmage.so not found at', LIB_PATH, file=sys.stderr)
    sys.exit(2)

# splitmix64 constants mirrored from C
def splitmix64(x):
    x = (x + 0x9e3779b97f4a7c15) & ((1<<64)-1)
    z = x
    z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9 & ((1<<64)-1)
    z = (z ^ (z >> 27)) * 0x94d049bb133111eb & ((1<<64)-1)
    return (z ^ (z >> 31)) & ((1<<64)-1)

# Python reference sign-hash projection
def python_rp_project(matrix, n, rp_dim, seed):
    stride = 0x9e3779b97f4a7c15
    out = [0.0] * rp_dim
    for i in range(n):
        for j in range(n):
            v = matrix[i * n + j]
            if v == 0.0:
                continue
            base = ((i & 0xffffffff) << 32) ^ (j & 0xffffffff)
            mz = splitmix64(base ^ seed)
            for d in range(rp_dim):
                h = splitmix64((mz + (d * stride)) & ((1<<64)-1))
                s = 1.0 if (h & 1) else -1.0
                out[d] += s * v
    return out

# Define ctypes struct for rp_config_t
class RPConfig(Structure):
    _fields_ = [('rp_dim', c_size_t), ('hash_seed', c_uint32), ('projection', POINTER(c_double))]

lib = ctypes.CDLL(LIB_PATH)
# declare rp_project
lib.rp_project.argtypes = [POINTER(c_double), c_size_t, ctypes.POINTER(RPConfig)]
lib.rp_project.restype = ctypes.c_int

# small test matrix (3x3)
n = 3
matrix = [
    1.0, 0.5, -0.2,
    0.0, 1.2, 0.3,
    -0.1, 0.0, 0.7
]
rp_dim = 16
seed = 12345

# prepare projection buffer
proj_buf = (c_double * rp_dim)()
config = RPConfig(rp_dim, seed, ctypes.cast(proj_buf, POINTER(c_double)))

# call C implementation
mat_buf = (c_double * (n*n))(*matrix)
ret = lib.rp_project(mat_buf, n, ctypes.byref(config))
if ret != 0:
    print('rp_project failed with code', ret, file=sys.stderr)
    sys.exit(3)

c_out = [proj_buf[i] for i in range(rp_dim)]
py_out = python_rp_project(matrix, n, rp_dim, seed)

# compare
ok = True
for i, (a,b) in enumerate(zip(c_out, py_out)):
    # allow tiny floating mismatch
    if abs(a - b) > 1e-12:
        print(f'mismatch at dim {i}: C={a} PY={b}')
        ok = False

if ok:
    print('RP golden parity: PASSED')
    sys.exit(0)
else:
    print('RP golden parity: FAILED')
    sys.exit(4)
