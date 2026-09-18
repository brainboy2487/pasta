#!/usr/bin/env python3
"""Minimal HTTP JSON API for Mage CLI.

Endpoints:
- POST /generate  -> {"prompt": "..."} returns JSON generated string
- POST /train     -> {"input": "path/to/input.jsonl", "py": true, "snapshot": true}

This is intentionally dependency-free (uses stdlib).
"""
import json
from http.server import HTTPServer, BaseHTTPRequestHandler
import subprocess
import threading
import sys

class Handler(BaseHTTPRequestHandler):
    def _send(self, code, obj):
        b = json.dumps(obj, ensure_ascii=False).encode('utf-8')
        self.send_response(code)
        self.send_header('Content-Type', 'application/json; charset=utf-8')
        self.send_header('Content-Length', str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def do_POST(self):
        length = int(self.headers.get('content-length', '0'))
        data = self.rfile.read(length).decode('utf-8') if length else ''
        try:
            obj = json.loads(data) if data else {}
        except Exception as e:
            return self._send(400, {'error': 'invalid json', 'detail': str(e)})

        if self.path == '/generate':
            prompt = obj.get('prompt')
            if not prompt:
                return self._send(400, {'error': 'missing prompt'})
            # call local CLI generate
            try:
                proc = subprocess.run(['./bin/mage_cli', 'generate', prompt], capture_output=True, text=True, timeout=20)
                out = proc.stdout.strip()
                return self._send(200, {'ok': True, 'result': out})
            except Exception as e:
                return self._send(500, {'error': str(e)})

        elif self.path == '/train':
            inp = obj.get('input')
            if not inp:
                return self._send(400, {'error': 'missing input path'})
            flags = []
            if obj.get('py'):
                flags.append('--py')
            if obj.get('snapshot'):
                flags.append('--snapshot')
            manifest = obj.get('manifest')
            cmd = ['./bin/mage_cli', 'train', inp] + flags
            if manifest:
                cmd += ['--manifest', manifest]
            try:
                proc = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
                return self._send(200, {'ok': proc.returncode == 0, 'stdout': proc.stdout, 'stderr': proc.stderr})
            except Exception as e:
                return self._send(500, {'error': str(e)})

        else:
            return self._send(404, {'error': 'not found'})

def run(host='127.0.0.1', port=8888):
    server = HTTPServer((host, port), Handler)
    print(f'Serving on http://{host}:{port}')
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print('\nShutting down')
        server.server_close()

if __name__ == '__main__':
    run()
