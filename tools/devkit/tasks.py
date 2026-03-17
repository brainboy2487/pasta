#!/usr/bin/env python3
import sys, subprocess
from pathlib import Path
ROOT = Path('.').resolve()
ART = ROOT / 'artifacts'
ART.mkdir(exist_ok=True)

def run(cmd):
    print('> ' + ' '.join(cmd))
    p = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    print(p.stdout)
    return p.returncode, p.stdout

def build():
    return run(['cargo', 'build', '--release'])

def smoke():
    rc, out = build()
    if rc != 0:
        (ART / 'EATME_build.txt').write_text(out)
        return rc
    scripts = list((ROOT / 'tests').glob('*.ps'))
    for s in scripts:
        rc, out = run([str(ROOT / 'target' / 'release' / 'pasta'), str(s)])
        if rc != 0:
            (ART / f'EATME_{s.name}.txt').write_text(out)
            return rc
    return 0

def golden_check():
    return run(['python3', 'tools/devkit/check_golden.py'])

def install():
    return run(['bash', 'tools/devkit/install_pasta.sh'])

if __name__ == '__main__':
    cmd = sys.argv[1] if len(sys.argv) > 1 else 'help'
    if cmd == 'build':
        sys.exit(build()[0])
    elif cmd == 'smoke':
        sys.exit(smoke())
    elif cmd == 'golden-check':
        sys.exit(golden_check()[0])
    elif cmd == 'install':
        sys.exit(install()[0])
    else:
        print('Usage: tasks.py [build|smoke|golden-check|install]')
        sys.exit(2)
