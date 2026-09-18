import sys
import importlib.util
import numpy as np

spec = importlib.util.spec_from_file_location('ffi', 'python/mage/ffi.py')
ffi = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ffi)


def test_mage_vectorize_fallback():
    v1 = ffi.mage_vectorize('unit-test', dim=16)
    v2 = ffi.mage_vectorize('unit-test', dim=16)
    assert isinstance(v1, np.ndarray)
    assert v1.shape == (16,)
    # deterministic fallback should be identical across calls
    assert np.allclose(v1, v2)


def test_mage_retrieve_fallback():
    q = np.zeros(16, dtype=np.float32)
    res = ffi.mage_retrieve(q, k=3)
    assert isinstance(res, dict)
    assert 'results' in res
