"""
MAGE Python Package
===================
Python utilities and FFI bindings for the MAGE system.

Generated: 2026-01-23 15:32:35
"""

__version__ = "1.0.0"

from .ffi import mage_vectorize, mage_retrieve, lib_available
try:
    from .golden_tests import run_golden_tests
except Exception:
    run_golden_tests = None

try:
    from .visualization import plot_tier_performance
except Exception:
    plot_tier_performance = None

__all__ = [
    'mage_vectorize',
    'mage_retrieve',
    'lib_available',
]
if run_golden_tests is not None:
    __all__.append('run_golden_tests')
if plot_tier_performance is not None:
    __all__.append('plot_tier_performance')
