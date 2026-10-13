//! The identity seam: `MacAddr` ↔ `NodeId` (`ADR-0003` §3).
//!
//! The wire carries the node's dense contract identity, but the home model
//! names a node by its factory base MAC. There is no arithmetic mapping between
//! the two, so the seam is an explicit, bidirectional mapping that
//! `aquila-sources` maintains from the committed topology and consults on
//! ingest. This module owns that mapping and its consistency invariants.

use std::collections::BTreeMap;

use aquila_types::{MacAddr, NodeId};

use crate::error::WireError;

/// A bidirectional, conflict-checked mapping between factory MACs and dense
/// node identities.
///
/// The mapping is injective in both directions: each MAC maps to at most one
/// [`NodeId`] and each [`NodeId`] maps to at most one MAC. A conflicting insert
/// is rejected with [`WireError::IdentityConflict`] rather than silently
/// overwriting an existing binding.
#[derive(Debug, Clone, Default)]
pub struct NodeIdentityMap {
    by_mac: BTreeMap<[u8; 6], NodeId>,
    by_id: BTreeMap<NodeId, [u8; 6]>,
}

impl NodeIdentityMap {
    /// A fresh, empty mapping.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind `mac` to `node`, if both are free.
    ///
    /// Re-inserting an identical pair is a no-op and succeeds. Binding either
    /// half to a different counterpart fails with
    /// [`WireError::IdentityConflict`], naming which half conflicted.
    pub fn insert(&mut self, mac: MacAddr, node: NodeId) -> Result<(), WireError> {
        if let Some(existing) = self.by_mac.get(&mac.0) {
            if *existing != node {
                return Err(WireError::IdentityConflict { kind: "mac" });
            }
        }
        if let Some(existing) = self.by_id.get(&node) {
            if *existing != mac.0 {
                return Err(WireError::IdentityConflict { kind: "node id" });
            }
        }
        self.by_mac.insert(mac.0, node);
        self.by_id.insert(node, mac.0);
        Ok(())
    }

    /// The dense identity bound to `mac`, if any.
    #[must_use]
    pub fn node_id(&self, mac: MacAddr) -> Option<NodeId> {
        self.by_mac.get(&mac.0).copied()
    }

    /// The factory MAC bound to `node`, if any.
    #[must_use]
    pub fn mac(&self, node: NodeId) -> Option<MacAddr> {
        self.by_id.get(&node).copied().map(MacAddr)
    }

    /// Whether `mac` is bound.
    #[must_use]
    pub fn contains_mac(&self, mac: MacAddr) -> bool {
        self.by_mac.contains_key(&mac.0)
    }

    /// Whether `node` is bound.
    #[must_use]
    pub fn contains_node(&self, node: NodeId) -> bool {
        self.by_id.contains_key(&node)
    }

    /// Unbind `mac`, returning the identity it named.
    pub fn remove_mac(&mut self, mac: MacAddr) -> Option<NodeId> {
        let node = self.by_mac.remove(&mac.0)?;
        self.by_id.remove(&node);
        Some(node)
    }

    /// Unbind `node`, returning the MAC it named.
    pub fn remove_node(&mut self, node: NodeId) -> Option<MacAddr> {
        let mac = self.by_id.remove(&node)?;
        self.by_mac.remove(&mac);
        Some(MacAddr(mac))
    }

    /// The number of bound pairs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_mac.len()
    }

    /// Whether the mapping is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_mac.is_empty()
    }
}
