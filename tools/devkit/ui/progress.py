# tools/devkit/ui/progress.py
import sys, time, threading
from typing import Optional

class Spinner:
    def __init__(self, text: str = "", interval: float = 0.08):
        self.text = text
        self.interval = interval
        self._running = False
        self._thread: Optional[threading.Thread] = None
        self._frames = ["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"]

    def start(self):
        if self._running: return
        self._running = True
        def run():
            i = 0
            while self._running:
                frame = self._frames[i % len(self._frames)]
                sys.stdout.write(f"\r{frame} {self.text}")
                sys.stdout.flush()
                time.sleep(self.interval)
                i += 1
            sys.stdout.write("\r" + " " * (len(self.text) + 4) + "\r")
            sys.stdout.flush()
        self._thread = threading.Thread(target=run, daemon=True)
        self._thread.start()

    def stop(self):
        self._running = False
        if self._thread:
            self._thread.join()
            self._thread = None

class AnimatedDots:
    def __init__(self, text: str = "", max_dots: int = 3, interval: float = 0.5):
        self.text = text
        self.max_dots = max_dots
        self.interval = interval
        self._running = False
        self._thread: Optional[threading.Thread] = None

    def start(self):
        if self._running: return
        self._running = True
        def run():
            i = 0
            while self._running:
                dots = "." * (i % (self.max_dots + 1))
                sys.stdout.write(f"\r{self.text}{dots}{' ' * (self.max_dots - len(dots))}")
                sys.stdout.flush()
                time.sleep(self.interval)
                i += 1
            sys.stdout.write("\r" + " " * (len(self.text) + self.max_dots) + "\r")
            sys.stdout.flush()
        self._thread = threading.Thread(target=run, daemon=True)
        self._thread.start()

    def stop(self):
        self._running = False
        if self._thread:
            self._thread.join()
            self._thread = None

class ProgressBar:
    def __init__(self, total: int, width: int = 40, prefix: str = ""):
        self.total = max(1, total)
        self.width = width
        self.prefix = prefix
        self.current = 0
        self.start_time = time.time()

    def update(self, step: int = 1):
        self.current = min(self.total, self.current + step)
        frac = self.current / self.total
        filled = int(self.width * frac)
        bar = "█" * filled + "-" * (self.width - filled)
        elapsed = time.time() - self.start_time
        rate = self.current / elapsed if elapsed > 0 else 0.0
        eta = (self.total - self.current) / rate if rate > 0 else float("inf")
        eta_str = f"{int(eta)}s" if eta != float("inf") else "--"
        sys.stdout.write(f"\r{self.prefix} |{bar}| {self.current}/{self.total} ETA:{eta_str}")
        sys.stdout.flush()
        if self.current >= self.total:
            sys.stdout.write("\n")

    def finish(self):
        if self.current < self.total:
            self.update(self.total - self.current)
