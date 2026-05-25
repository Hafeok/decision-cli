"""Driver abstraction surface re-exporting the Protocol plus the FakeDriver double."""

from .base import Driver
from .fake import FakeDispatch, FakeDriver

__all__ = ["Driver", "FakeDispatch", "FakeDriver"]
