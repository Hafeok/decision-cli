"""Driver abstraction surface re-exporting the Protocol plus FakeDriver/EventDriver."""

from .base import Driver
from .event_driver import EventDriver
from .fake import FakeDispatch, FakeDriver

__all__ = ["Driver", "EventDriver", "FakeDispatch", "FakeDriver"]
