//! Governance Module for KAIROS-ARK.
//!
//! Provides Human-in-the-Loop (HITL) primitives and audit verification.

pub mod approval;
pub mod verification;

pub use approval::*;
pub use verification::*;
