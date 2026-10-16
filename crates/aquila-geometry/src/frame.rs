//! Coordinate frames and planar transforms.
//!
//! `data-contracts.md` §3.1 fixes a single **home frame**: Cartesian, metres,
//! right-handed, `x` east, `y` north, `z` up, origin at the home's declared
//! reference point. A zone carries `origin_home_m` and `extent_home_m`; a node
//! carries `pos_home_m`.
//!
//! Geometry wraps that single frame with confidence-scored planar SE(2)
//! transforms. A [`Transform2`] places a frame in its parent: `apply` maps a
//! point from the frame into its parent, and `apply_inverse` maps it back.
//! Composition multiplies confidence ([`crate::chain`]); `Unknown` placement is
//! the identity with **zero** confidence, never a guessed transform
//! (`geometry-calibration.md` §2).

use aquila_types::{Confidence, NodeId, ZoneId};
use serde::{Deserialize, Serialize};

/// The confidence of a configured (definitional) frame such as a zone origin:
/// the frame *is* the survey, so there is nothing to be unsure about.
pub const FRAME_CONFIDENCE_CONFIGURED: Confidence = Confidence::ONE;

/// A planar, right-handed, metric transform placing a child frame in its
/// parent frame.
///
/// `apply(p_child) = origin + R(yaw) · p_child`, with `R` a rotation by
/// `yaw_rad` about the `z` axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform2 {
    /// The child frame's origin in the parent frame, in metres.
    pub origin: [f32; 2],
    /// The child frame's yaw in the parent frame, in radians.
    pub yaw_rad: f32,
    /// The confidence that the transform is correct.
    pub confidence: Confidence,
}

impl Transform2 {
    /// The identity transform, known exactly.
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            origin: [0.0, 0.0],
            yaw_rad: 0.0,
            confidence: Confidence::ONE,
        }
    }

    /// The identity transform marked **unknown**: the honest stand-in for an
    /// unsurveyed placement, with zero confidence.
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            origin: [0.0, 0.0],
            yaw_rad: 0.0,
            confidence: Confidence::ZERO,
        }
    }

    /// Construct a transform from a parent-frame origin, a yaw and a confidence.
    #[must_use]
    pub const fn new(origin: [f32; 2], yaw_rad: f32, confidence: Confidence) -> Self {
        Self {
            origin,
            yaw_rad,
            confidence,
        }
    }

    /// The transform placing a zone frame in the home frame, from the zone's
    /// configured origin.
    #[must_use]
    pub fn from_zone(zone: &aquila_types::Zone) -> Self {
        Self::new(
            [zone.origin_home_m[0], zone.origin_home_m[1]],
            0.0,
            FRAME_CONFIDENCE_CONFIGURED,
        )
    }

    /// Whether the transform is known (non-zero confidence).
    #[must_use]
    pub fn is_known(&self) -> bool {
        self.confidence.get() > 0.0
    }

    /// Map a point from the child frame into the parent frame.
    #[must_use]
    pub fn apply(&self, p: [f32; 2]) -> [f32; 2] {
        let (s, c) = self.yaw_rad.sin_cos();
        [
            self.origin[0] + c * p[0] - s * p[1],
            self.origin[1] + s * p[0] + c * p[1],
        ]
    }

    /// Map a point from the parent frame into the child frame.
    #[must_use]
    pub fn apply_inverse(&self, p: [f32; 2]) -> [f32; 2] {
        let (s, c) = (-self.yaw_rad).sin_cos();
        let dx = p[0] - self.origin[0];
        let dy = p[1] - self.origin[1];
        [c * dx - s * dy, s * dx + c * dy]
    }

    /// Compose two transforms: `self` places frame `A` in the parent, `child`
    /// places frame `B` in `A`; the result places `B` in the parent.
    ///
    /// The composed confidence is the product of the two, because a transform
    /// through an unsure placement is itself unsure.
    #[must_use]
    pub fn compose(&self, child: &Transform2) -> Transform2 {
        Transform2 {
            origin: self.apply(child.origin),
            yaw_rad: self.yaw_rad + child.yaw_rad,
            confidence: crate::confidence::chain(self.confidence, child.confidence),
        }
    }

    /// The inverse transform, mapping parent coordinates back into the child
    /// frame. Inversion preserves confidence.
    #[must_use]
    pub fn inverse(&self) -> Transform2 {
        Transform2 {
            origin: self.apply_inverse([0.0, 0.0]),
            yaw_rad: -self.yaw_rad,
            confidence: self.confidence,
        }
    }
}

/// A named coordinate frame.
///
/// `World` is the home frame from `data-contracts.md` §3.1; `Zone` and
/// `Node` are the nested frames. Zones are detection regions in home space;
/// nodes are sensor mount points. The non-physical `CsiBand` subcarrier space
/// is intentionally not a physical frame and is not modelled here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frame {
    /// The home frame: `x` east, `y` north, `z` up, metres.
    World,
    /// A zone frame in the home frame.
    Zone(ZoneId),
    /// A node mount frame in the home frame.
    Node(NodeId),
}
