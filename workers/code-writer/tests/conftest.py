"""Make the code_writer package importable when pytest is invoked outside uv."""

from __future__ import annotations

import sys
from pathlib import Path

# Make the package importable when running pytest without an editable install
# (e.g. when `product verify` invokes pytest from the repo root).
ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))
