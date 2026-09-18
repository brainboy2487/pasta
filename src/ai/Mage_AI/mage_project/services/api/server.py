#!/usr/bin/env python3
"""Minimal WSGI JSON API for MAGE (no external deps).

Usage: python3 services/api/server.py services/api/config.json

This is a scaffold: handlers return dummy responses suitable for integration tests.
"""
import sys
import json
from wsgiref.simple_server import make_server
from io import BytesIO
import importlib.util
from pathlib import Path


def load_config(path):
    try:
        with open(path, 'r') as f:
            return json.load(f)
    except Exception:
        return {"online": False, "host": "127.0.0.1", "port": 8080}


def read_json(environ):
    try:
        length = int(environ.get('CONTENT_LENGTH', 0) or 0)
        body = environ['wsgi.input'].read(length) if length > 0 else b''
        if not body:
            return None
        return json.loads(body.decode('utf-8'))
    except Exception:
        return None


def json_response(start_response, status_code, obj):
    body = json.dumps(obj).encode('utf-8')
    start_response(f"{status_code} OK", [
        ('Content-Type', 'application/json'),
        ('Content-Length', str(len(body)))
    ])
    return [body]


def app_factory(config):
    # load python/mage/ffi.py dynamically to avoid package import side-effects
    ffi_path = Path(__file__).parent.parent / 'python' / 'mage' / 'ffi.py'
    ffi = None
    try:
        spec = importlib.util.spec_from_file_location('mage_ffi', str(ffi_path))
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        ffi = mod
    except Exception:
        ffi = None

    def app(environ, start_response):
        path = environ.get('PATH_INFO', '/')
        method = environ.get('REQUEST_METHOD', 'GET')
        # simple routing
        if path == '/vectorize' and method == 'POST':
            body = read_json(environ)
            text = (body.get('text') if isinstance(body, dict) else None)
            dim = int(body.get('dim', 128)) if isinstance(body, dict) and body.get('dim') else 128
            try:
                if ffi is not None:
                    vec = ffi.mage_vectorize(text, dim=dim).tolist()
                else:
                    # fallback deterministic python vector
                    from math import floor
                    vec = [0.0] * dim
            except Exception:
                vec = [0.0] * dim
            return json_response(start_response, 200, { 'vector': vec })
        if path == '/retrieve' and method == 'POST':
            body = read_json(environ)
            k = int(body.get('k', 5)) if isinstance(body, dict) and body.get('k') else 5
            try:
                # Accept either a provided vector or text to vectorize
                qvec = None
                if isinstance(body, dict) and body.get('vector'):
                    qvec = body.get('vector')
                elif isinstance(body, dict) and body.get('text') and ffi is not None:
                    qvec = ffi.mage_vectorize(body.get('text'))
                if ffi is not None and qvec is not None:
                    # ensure numpy array input for ffi
                    import numpy as _np
                    q = _np.asarray(qvec, dtype=_np.float32)
                    res = ffi.mage_retrieve(q, k=k)
                    return json_response(start_response, 200, res)
            except Exception:
                pass
            return json_response(start_response, 200, { 'results': [] })
        if path == '/generate' and method == 'POST':
            body = read_json(environ)
            prompt = (body.get('prompt') if isinstance(body, dict) else '')
            return json_response(start_response, 200, { 'generated': '...' })
        if path == '/tap/parse' and method == 'POST':
            body = read_json(environ)
            text = (body.get('text') if isinstance(body, dict) else '')
            # scaffold: return simple parse trace
            return json_response(start_response, 200, { 'shortlist': [], 'budget': 0, 'trace': [] })
        if path == '/admin/indices' and method == 'GET':
            return json_response(start_response, 200, { 'indices': [] })
        # not found
        start_response('404 Not Found', [('Content-Type', 'text/plain')])
        return [b'Not Found']
    return app


def run_server(cfg_path):
    cfg = load_config(cfg_path)
    if not cfg.get('online'):
        print('API server configured as offline (online=false) in', cfg_path)
        return 0
    host = cfg.get('host', '127.0.0.1')
    port = int(cfg.get('port', 8080))
    app = app_factory(cfg)
    with make_server(host, port, app) as httpd:
        print(f"Serving on http://{host}:{port} ... (CTRL+C to stop)")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print('Shutting down')
    return 0


if __name__ == '__main__':
    cfg = sys.argv[1] if len(sys.argv) > 1 else 'services/api/config.json'
    sys.exit(run_server(cfg))
