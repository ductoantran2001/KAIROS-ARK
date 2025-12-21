"""
KAIROS-ARK: A deterministic multi-threaded scheduler for agentic AI workflows.

This package provides a high-performance execution kernel with support for:
- Conditional branching (Branch nodes)
- Parallel execution (Fork/Join)
- Deterministic replay through logical clocks
- System-level tracing (Audit Ledger)
- Policy engine with capability model (Phase 2)
"""

from .agent import Agent, Policy, Cap

# Import the native Rust extension
try:
    from ._core import PyKernel, PyEvent, PyNode, PyPolicy, PyCap
except ImportError as e:
    import sys
    print(f"Warning: Failed to import native extension: {e}", file=sys.stderr)
    print("Make sure to build with: maturin develop", file=sys.stderr)
    PyKernel = None
    PyEvent = None
    PyNode = None
    PyPolicy = None
    PyCap = None

__version__ = "0.2.0"
__author__ = "YASSERRMD"

__all__ = [
    "Agent",
    "Policy",
    "Cap",
    "PyKernel",
    "PyEvent", 
    "PyNode",
    "PyPolicy",
    "PyCap",
]
