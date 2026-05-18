"""Smoke test asserting the worker module imports.

This is the only test that should ever pass on a green worker before the
implementation lands. Real worker tests live in the TC bash scripts that
drive the worker as a subprocess.
"""

import code_writer


def test_module_imports() -> None:
    assert code_writer.__version__ == "0.1.0"
