"""pytest configuration for tc-author worker tests."""

import sys
from pathlib import Path

# Make the tc_author package importable without requiring an install.
_SRC = Path(__file__).parent.parent / "src"
if _SRC.exists() and str(_SRC) not in sys.path:
    sys.path.insert(0, str(_SRC))
