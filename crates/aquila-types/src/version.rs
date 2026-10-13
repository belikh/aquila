//! The contract-set version.
//!
//! There are exactly two version numbers in Aquila and they move together: the
//! **contract version** (this type, carried by every `Session`, `LedgerRecord`
//! and session archive) and the **wire version** (owned by `aquila-wire`). A
//! contract is append-only within a major version.

use core::fmt;

use serde::{Deserialize, Serialize};

/// The contract-set version: semver with a major and a minor component only;
/// there is no patch component.
///
/// A **minor** bump is additive only: new defaulted fields and new enum
/// variants where an `Unknown` fallback already exists. A **major** bump is any
/// removal, retype, unit change, meaning change or reordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ContractVersion {
    /// The major component. A change here breaks older readers.
    pub major: u16,
    /// The minor component. A change here is additive and ignorable.
    pub minor: u16,
}

impl ContractVersion {
    /// Construct a contract version from its components.
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Whether two versions share a major component, and so are compatible
    /// under the append-only rule.
    #[must_use]
    pub const fn is_compatible_with(&self, other: &Self) -> bool {
        self.major == other.major
    }
}

impl fmt::Display for ContractVersion {
    /// Render as `"<major>.<minor>"`, for example `"1.0"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// The current contract version, `1.0`.
///
/// Every `Session`, `LedgerRecord` and session archive pins this value; a
/// reader must never silently reinterpret a value recorded under an older
/// contract.
pub const CONTRACT_VERSION: ContractVersion = ContractVersion { major: 1, minor: 0 };

/// The major component of [`CONTRACT_VERSION`].
pub const CONTRACT_VERSION_MAJOR: u16 = CONTRACT_VERSION.major;

/// The minor component of [`CONTRACT_VERSION`].
pub const CONTRACT_VERSION_MINOR: u16 = CONTRACT_VERSION.minor;
