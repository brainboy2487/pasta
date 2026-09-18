import importlib.util
spec = importlib.util.spec_from_file_location('ffi', 'python/mage/ffi.py')
ffi = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ffi)
import numpy as np
q = np.zeros(16, dtype=np.float32)
print('calling mage_retrieve...')
res = ffi.mage_retrieve(q, k=3)
print('returned:', res)
