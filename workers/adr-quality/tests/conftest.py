"""pytest configuration for adr-quality worker tests."""

import sys
from pathlib import Path

# Make the adr_quality package importable without requiring an install.
_SRC = Path(__file__).parent.parent / "src"
if _SRC.exists() and str(_SRC) not in sys.path:
    sys.path.insert(0, str(_SRC))

# Make shared test helpers importable as `_helpers` from sibling test modules.
_TESTS = Path(__file__).parent
if str(_TESTS) not in sys.path:
    sys.path.insert(0, str(_TESTS))
