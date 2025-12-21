//! KAIROS-ARK Core Library
//! 
//! A deterministic multi-threaded scheduler for agentic AI workflows
//! with support for conditional branching, parallel execution, and
//! bit-for-bit identical replayability.

pub mod core;
pub mod adapters;
pub mod governance;

use pyo3::prelude::*;
use crate::core::{PyKernel, PyEvent, PyNode, PyPolicy, PyCap};

/// KAIROS-ARK Python module
#[pymodule]
fn _core(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyKernel>()?;
    m.add_class::<PyEvent>()?;
    m.add_class::<PyNode>()?;
    m.add_class::<PyPolicy>()?;
    m.add_class::<PyCap>()?;
    Ok(())
}
