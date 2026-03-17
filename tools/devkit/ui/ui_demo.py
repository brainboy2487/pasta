#!/usr/bin/env python3
# tools/devkit/ui/ui_demo.py
import sys, time
from pathlib import Path
if __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
from progress import Spinner, AnimatedDots, ProgressBar

def demo_spinner():
    s = Spinner('Building pasta')
    s.start()
    time.sleep(2.2)
    s.stop()
    print('Build step done')

def demo_dots():
    d = AnimatedDots('Running smoke tests', max_dots=4, interval=0.4)
    d.start()
    time.sleep(3.0)
    d.stop()
    print('Smoke tests finished')

def demo_progress():
    p = ProgressBar(total=20, prefix='Testing')
    for _ in range(20):
        time.sleep(0.12)
        p.update()
    p.finish()

if __name__ == '__main__':
    print('Spinner demo')
    demo_spinner()
    print('Dots demo')
    demo_dots()
    print('Progress bar demo')
    demo_progress()
