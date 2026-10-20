//! Geometry error type.
//!
//! Loading a topology is the only fallible operation in this crate: the contract
//! `aquila_types::Topology` is validated before it is indexed, so a topology
//! that reaches a [`crate::Topology`] satisfies every referential-integrity rule
//! in `data-contracts.md` §3. Queries are total over nodes that belong to the
//! topology and return `Scored::unknown()` for anything else; they never panic.

use aquila_types::ValidationError;

/// The error returned by [`crate::Topology::load`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GeometryError {
    /// The contract topology failed `aquila-types` referential integrity.
    #[error("invalid topology: {0}")]
    InvalidTopology(#[from] ValidationError),
}
