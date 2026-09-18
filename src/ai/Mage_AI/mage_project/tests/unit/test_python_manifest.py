import importlib.util
spec = importlib.util.spec_from_file_location('ffi', 'python/mage/ffi.py')
ffi = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ffi)

# create a small manifest file
p = 'build/tests/manifest_test.jsonl'
with open(p, 'w') as f:
    f.write('{"index": 2, "id": "doc2", "title": "Two"}\n')
    f.write('{"index": 7, "id": "doc7", "path": "/tmp/doc7"}\n')

h = ffi.load_manifest(p)
if h is None:
    print('manifest load failed (ffi)')
else:
    e2 = ffi.manifest_get_entry(h, 2)
    e7 = ffi.manifest_get_entry(h, 7)
    assert e2['id'] == 'doc2'
    assert e7['path'] == '/tmp/doc7'
    ffi.manifest_free_handle(h)
    print('python manifest ffi OK')
