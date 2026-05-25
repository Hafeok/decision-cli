"""Pytest setup for the pipeline-worker SDK test suite."""

from __future__ import annotations

import sys
from pathlib import Path

# Make ``pipeline_worker_sdk`` importable without an editable install.
HERE = Path(__file__).resolve()
SRC = HERE.parent.parent / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))
