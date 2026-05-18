"""Python code-writer worker for decision-cli's implementer role.

Stateless contract per ADR-008: bundle in, CodeChange artifact out.
Workers MUST NOT talk to the graph. The harness owns reads and writes.
"""

__version__ = "0.1.0"
