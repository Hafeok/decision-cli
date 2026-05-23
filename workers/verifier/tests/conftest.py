"""Shared fixtures and helpers for verifier tests."""

from __future__ import annotations

import sys
from pathlib import Path

# Make the package importable when running pytest without an editable install.
ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))
