"""Python FFI Bindings for libmage (ctypes).

This module exposes a small, safe wrapper around the C API implemented in
`lib/libmage.so`. If the shared library is not available, functions fall back
to pure-Python deterministic behavior where possible to support tests.
"""
import ctypes
import json
import os
from pathlib import Path
import numpy as np


_mage = None
_libc = None
_lib_path = Path(__file__).parent.parent.parent / "lib" / "libmage.so"
if _lib_path.exists():
    try:
        _mage = ctypes.CDLL(str(_lib_path))
    except Exception:
        _mage = None

# libc free for deallocating C malloc'd strings
try:
    _libc = ctypes.CDLL("libc.so.6")
    _libc.free.argtypes = [ctypes.c_void_p]
except Exception:
    _libc = None


def lib_available():
    return _mage is not None


# Bindings: mage_api_vectorize_text(const char*, float*, uint32_t) -> int
if _mage is not None:
    try:
        _mage.mage_api_vectorize_text.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_float), ctypes.c_uint32]
        _mage.mage_api_vectorize_text.restype = ctypes.c_int

        _mage.mage_api_retrieve.argtypes = [ctypes.POINTER(ctypes.c_float), ctypes.c_uint32, ctypes.c_uint32]
        # Return raw pointer to malloc'd C string; use c_void_p to avoid
        # automatic ctypes conversion which would make manual free unsafe.
        _mage.mage_api_retrieve.restype = ctypes.c_void_p
    except Exception:
        # If function not present, fall back to None and log
        import sys, traceback
        traceback.print_exc(file=sys.stderr)
        _mage = None

try:
    if _mage is not None:
        _mage.mage_api_manifest_load.argtypes = [ctypes.c_char_p]
        _mage.mage_api_manifest_load.restype = ctypes.c_void_p
        _mage.mage_api_manifest_get.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
        _mage.mage_api_manifest_get.restype = ctypes.c_void_p
        _mage.mage_api_manifest_free.argtypes = [ctypes.c_void_p]
        _mage.mage_api_manifest_free.restype = None
except Exception:
    pass


def mage_vectorize(text: str, dim: int = 128) -> np.ndarray:
    """Return a deterministic float32 vector for `text` of length `dim`.

    Uses the C `mage_api_vectorize_text` when available, otherwise a local
    deterministic fallback.
    """
    out = np.zeros(dim, dtype=np.float32)
    if _mage is None:
        # Pure-Python deterministic fallback matching C placeholder logic
        h = 1469598101
        if text:
            for ch in text.encode('utf-8'):
                h = (h ^ ch) * 16777619 & 0xFFFFFFFF
        for i in range(dim):
            out[i] = float(((h >> (i % 24)) & 0xFF) - 128) / 128.0
        return out

    buf = (ctypes.c_float * dim)()
    s = text.encode('utf-8') if text is not None else None
    rc = _mage.mage_api_vectorize_text(s, buf, ctypes.c_uint32(dim))
    if rc != 0:
        return out
    # copy into numpy
    for i in range(dim):
        out[i] = buf[i]
    return out


def mage_retrieve(query_vector: np.ndarray, k: int = 5):
    """Retrieve nearest neighbors for `query_vector`.

    Returns parsed JSON (usually a dict with `results` list). When the C API
    is available, calls `mage_api_retrieve` which returns a malloc'd JSON C string.
    """
    if query_vector is None or query_vector.size == 0:
        return {"results": []}
    dim = ctypes.c_uint32(query_vector.size)
    if _mage is None:
        # No C backend: return empty results placeholder
        return {"results": []}

    # ensure float32 contiguous
    q = np.ascontiguousarray(query_vector, dtype=np.float32)
    ptr = q.ctypes.data_as(ctypes.POINTER(ctypes.c_float))
    res_ptr = _mage.mage_api_retrieve(ptr, dim, ctypes.c_uint32(k))
    if not res_ptr:
        return {"results": []}
    try:
        js = ctypes.cast(res_ptr, ctypes.c_char_p).value.decode('utf-8')
        return json.loads(js)
    finally:
        if _libc is not None:
            _libc.free(res_ptr)


def load_manifest(path: str):
    """Load manifest and return opaque handle (None on failure)."""
    if _mage is None:
        return None
    p = _mage.mage_api_manifest_load(path.encode('utf-8'))
    if not p:
        return None
    return p


def manifest_get_entry(handle, idx: int):
    """Return a dict with fields for manifest entry at index `idx`, or None."""
    if _mage is None or not handle:
        return None
    entp = _mage.mage_api_manifest_get(handle, ctypes.c_uint32(idx))
    if not entp:
        return None
    # The C struct layout is: uint32 index; char* id; char* title; char* path
    # Read memory via ctypes
    class CEntry(ctypes.Structure):
        _fields_ = [('index', ctypes.c_uint32), ('id', ctypes.c_char_p), ('title', ctypes.c_char_p), ('path', ctypes.c_char_p)]
    ce = ctypes.cast(entp, ctypes.POINTER(CEntry)).contents
    return {
        'index': int(ce.index),
        'id': ce.id.decode('utf-8') if ce.id else None,
        'title': ce.title.decode('utf-8') if ce.title else None,
        'path': ce.path.decode('utf-8') if ce.path else None,
    }


def manifest_free_handle(handle):
    if _mage is None or not handle:
        return
    _mage.mage_api_manifest_free(handle)


__all__ = ["lib_available", "mage_vectorize", "mage_retrieve", "load_manifest", "manifest_get_entry", "manifest_free_handle"]
