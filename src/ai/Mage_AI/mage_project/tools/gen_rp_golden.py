#!/usr/bin/env python3
"""Generate RP parity golden file matching C splitmix64 implementation.

Writes lines: key index sign
"""
import sys

MASK = (1 << 64) - 1

def splitmix64_mage(x):
    x = (x + 0x9e3779b97f4a7c15) & MASK
    z = x
    z = ((z ^ (z >> 30)) * 0xbf58476d1ce4e5b9) & MASK
    z = ((z ^ (z >> 27)) * 0x94d049bb133111eb) & MASK
    return (z ^ (z >> 31)) & MASK

def mage_hash_u64(x, seed):
    return splitmix64_mage((x + seed) & MASK)

def mage_hash_sign64(key, seed):
    h = mage_hash_u64(key, seed)
    return 1 if (h & 1) else -1

def mage_rp_index(key, dim, seed):
    if dim == 0:
        return 0
    h = mage_hash_u64(key ^ 0xAFFEDEADBEEF, seed)
    # multiply-shift mapping: floor(h * dim / 2^64)
    prod = (h * dim) & ((1 << 128) - 1)
    return (prod >> 64)

def main():
    out = 'tests/golden/rp_golden.txt'
    dim = 1024
    seed = 0x12345678
    start = 1
    end = 2000
    if len(sys.argv) > 1:
        out = sys.argv[1]
    if len(sys.argv) > 2:
        dim = int(sys.argv[2])
    if len(sys.argv) > 3:
        seed = int(sys.argv[3], 0)
    with open(out, 'w', encoding='utf-8') as f:
        for k in range(start, end + 1):
            idx = mage_rp_index(k, dim, seed)
            sign = mage_hash_sign64(k, seed ^ 0xDEADBEEF)
            f.write(f"{k} {idx} {sign}\n")
    print(f"Wrote {out} entries {start}-{end} dim={dim} seed=0x{seed:x}")

if __name__ == '__main__':
    main()
