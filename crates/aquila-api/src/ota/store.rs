//! The signed firmware image store (`firmware-ota.md` §3–§4).
//!
//! An image is admitted only after its SHA-256 digest is recomputed, its
//! Ed25519 signature over that digest verifies against the trusted firmware
//! key, and its declared target chip and core-compatibility range are accepted.
//! A rejected image is never written and therefore never offered to a node.
//!
//! The store lives under a configurable data directory (default
//! `/data/aquila/firmware/`, `firmware-ota.md` §3). Each image is one
//! `<id>.bin` file next to a `<id>.manifest.json` file, where `<id>` is the
//! lower-case hex SHA-256. The manifest is the durable, human-readable record
//! of version, size, digest, signature, target chip, compatibility and notes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use aquila_types::{sha256, to_hex};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

/// The default data directory for the signed firmware store
/// (`firmware-ota.md` §3).
pub const DEFAULT_FIRMWARE_DIR: &str = "/data/aquila/firmware/";

/// The inclusive core-version range an image is compatible with.
///
/// Versions are encoded as `major * 1000 + minor` (so core `1.0` is `1000`),
/// which is enough for the v1 `1.x` line and keeps the comparison total and
/// exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmwareCompat {
    /// The lowest compatible core version (inclusive).
    pub min_core: u32,
    /// The highest compatible core version (inclusive).
    pub max_core: u32,
}

impl FirmwareCompat {
    /// Construct a range, neutralising an inverted pair so `min <= max`.
    #[must_use]
    pub fn new(min_core: u32, max_core: u32) -> Self {
        if min_core <= max_core {
            Self { min_core, max_core }
        } else {
            Self {
                min_core: max_core,
                max_core: min_core,
            }
        }
    }

    /// Encode a `(major, minor)` pair as `major * 1000 + minor`.
    #[must_use]
    pub const fn encode(major: u16, minor: u16) -> u32 {
        (major as u32) * 1000 + (minor as u32)
    }

    /// Whether `core` falls inside the inclusive range.
    #[must_use]
    pub const fn accepts(&self, core: u32) -> bool {
        core >= self.min_core && core <= self.max_core
    }
}

/// The durable manifest of one stored image (`firmware-ota.md` §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmwareManifest {
    /// The lower-case hex SHA-256 of the image; also the store identity.
    pub sha256: String,
    /// The human-readable firmware version.
    pub version: String,
    /// The image size in bytes.
    pub size: u64,
    /// The Ed25519 signature over the digest, lower-case hex (128 chars).
    pub signature: String,
    /// The target chip the image runs on, for example `esp32s3`.
    pub target: String,
    /// The compatible core-version range.
    pub compatibility: FirmwareCompat,
    /// Free-form operator notes.
    pub notes: String,
}

impl FirmwareManifest {
    /// The store identity: the lower-case hex SHA-256.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.sha256
    }

    /// The digest as raw bytes, when the manifest is well formed.
    #[must_use]
    pub fn sha256_bytes(&self) -> Option<[u8; 32]> {
        decode_hex_fixed::<32>(&self.sha256)
    }

    /// The signature as raw bytes, when the manifest is well formed.
    #[must_use]
    pub fn signature_bytes(&self) -> Option<[u8; 64]> {
        decode_hex_fixed::<64>(&self.signature)
    }
}

/// An import request: the image bytes and the claimed metadata to verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRequest {
    /// The raw image bytes (a full firmware image, no deltas).
    pub image: Vec<u8>,
    /// The Ed25519 signature over the image digest.
    pub signature: [u8; 64],
    /// The human-readable firmware version.
    pub version: String,
    /// The target chip, for example `esp32s3`.
    pub target: String,
    /// The compatible core-version range.
    pub compatibility: FirmwareCompat,
    /// Free-form operator notes.
    pub notes: String,
}

/// A stored image: its manifest and its bytes, read back from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredImage {
    /// The verified manifest.
    pub manifest: FirmwareManifest,
    /// The image bytes.
    pub image: Vec<u8>,
}

/// Everything that can go wrong admitting or reading a firmware image.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A filesystem failure.
    #[error("firmware store io error: {0}")]
    Io(#[from] std::io::Error),
    /// The Ed25519 signature did not verify against the trusted key over the
    /// recomputed SHA-256 digest. The image is never written (`§4`).
    #[error("firmware image signature did not verify")]
    BadSignature,
    /// The image is empty or its metadata is malformed.
    #[error("invalid firmware image: {0}")]
    Invalid(String),
    /// The image's declared target chip is not the one expected.
    #[error("firmware target mismatch: expected {expected}, got {actual}")]
    TargetMismatch {
        /// The expected target chip.
        expected: String,
        /// The target the image declares.
        actual: String,
    },
    /// The image is not compatible with the running core version.
    #[error("firmware is incompatible with core version {core} (range {min}..={max})")]
    Incompatible {
        /// The core version being checked.
        core: u32,
        /// The lowest supported core version.
        min: u32,
        /// The highest supported core version.
        max: u32,
    },
    /// An image with the same digest already exists with different metadata.
    #[error("firmware image {id} already exists with different metadata")]
    Conflict {
        /// The conflicting image identity.
        id: String,
    },
}

/// The signed image store under a data directory.
#[derive(Debug)]
pub struct FirmwareStore {
    root: PathBuf,
    key: VerifyingKey,
    manifests: Mutex<BTreeMap<String, FirmwareManifest>>,
}

impl FirmwareStore {
    /// Open (or create) a store at `root`, trusting `key`, and load any
    /// existing manifests.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Io`] if the directory cannot be created or read, or
    /// [`StoreError::Invalid`] if an existing manifest cannot be parsed.
    pub fn open(root: impl Into<PathBuf>, key: VerifyingKey) -> Result<Self, StoreError> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        let mut manifests = BTreeMap::new();
        for entry in std::fs::read_dir(&root)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            let manifest: FirmwareManifest = serde_json::from_str(&text)
                .map_err(|error| StoreError::Invalid(format!("{}: {error}", path.display())))?;
            manifests.insert(manifest.sha256.clone(), manifest);
        }
        Ok(Self {
            root,
            key,
            manifests: Mutex::new(manifests),
        })
    }

    /// Open a store with the default data directory and the trusted key.
    ///
    /// # Errors
    ///
    /// As [`open`](Self::open).
    pub fn open_default(key: VerifyingKey) -> Result<Self, StoreError> {
        Self::open(DEFAULT_FIRMWARE_DIR, key)
    }

    /// The store's data directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The trusted firmware verification key.
    #[must_use]
    pub fn key(&self) -> &VerifyingKey {
        &self.key
    }

    /// Import and verify an image, writing it only if every check passes.
    ///
    /// The checks are, in order: the digest is recomputed from the bytes; the
    /// Ed25519 signature verifies over that digest; the target chip matches
    /// `expected_target`; and `core_version` is inside the compatibility range.
    /// A bad signature (or any failure) is never written, so it is never
    /// offered (`firmware-ota.md` §4).
    ///
    /// # Errors
    ///
    /// Returns a typed [`StoreError`] for each failure mode.
    pub fn import(
        &self,
        core_version: u32,
        expected_target: &str,
        request: ImportRequest,
    ) -> Result<FirmwareManifest, StoreError> {
        if request.image.is_empty() {
            return Err(StoreError::Invalid("image is empty".to_string()));
        }
        if request.version.trim().is_empty() {
            return Err(StoreError::Invalid("version is empty".to_string()));
        }
        if request.target != expected_target {
            return Err(StoreError::TargetMismatch {
                expected: expected_target.to_string(),
                actual: request.target,
            });
        }
        if !request.compatibility.accepts(core_version) {
            return Err(StoreError::Incompatible {
                core: core_version,
                min: request.compatibility.min_core,
                max: request.compatibility.max_core,
            });
        }

        let digest = sha256(&request.image);
        let signature = Signature::from_bytes(&request.signature);
        self.key
            .verify_strict(&digest, &signature)
            .map_err(|_| StoreError::BadSignature)?;

        let manifest = FirmwareManifest {
            sha256: to_hex(&digest),
            version: request.version,
            size: request.image.len() as u64,
            signature: to_hex(&request.signature),
            target: request.target,
            compatibility: request.compatibility,
            notes: request.notes,
        };

        let mut manifests = lock(&self.manifests);
        if let Some(existing) = manifests.get(&manifest.sha256) {
            if *existing == manifest {
                return Ok(existing.clone());
            }
            return Err(StoreError::Conflict {
                id: manifest.sha256,
            });
        }

        let image_path = self.root.join(format!("{}.bin", manifest.sha256));
        let manifest_path = self.root.join(format!("{}.manifest.json", manifest.sha256));
        std::fs::write(&image_path, &request.image)?;
        let text = serde_json::to_string_pretty(&manifest)
            .map_err(|error| StoreError::Invalid(error.to_string()))?;
        std::fs::write(&manifest_path, text)?;
        manifests.insert(manifest.sha256.clone(), manifest.clone());
        Ok(manifest)
    }

    /// Every stored manifest, in canonical id order.
    #[must_use]
    pub fn list(&self) -> Vec<FirmwareManifest> {
        lock(&self.manifests).values().cloned().collect()
    }

    /// Whether an image with `id` is stored.
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        lock(&self.manifests).contains_key(id)
    }

    /// The manifest for `id`, if stored.
    #[must_use]
    pub fn manifest(&self, id: &str) -> Option<FirmwareManifest> {
        lock(&self.manifests).get(id).cloned()
    }

    /// Read a stored image back from disk.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Io`] if the file cannot be read.
    pub fn get(&self, id: &str) -> Result<Option<StoredImage>, StoreError> {
        let manifest = match self.manifest(id) {
            Some(manifest) => manifest,
            None => return Ok(None),
        };
        let image = std::fs::read(self.root.join(format!("{id}.bin")))?;
        Ok(Some(StoredImage { manifest, image }))
    }

    /// Whether `id` may be offered to a node: it exists, targets `node_target`
    /// and is compatible with `core_version`.
    #[must_use]
    pub fn offers(&self, id: &str, node_target: &str, core_version: u32) -> bool {
        match self.manifest(id) {
            Some(manifest) => {
                manifest.target == node_target && manifest.compatibility.accepts(core_version)
            }
            None => false,
        }
    }
}

/// Decode exactly `N` bytes of lower-case or upper-case hex.
fn decode_hex_fixed<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    let bytes = text.as_bytes();
    for index in 0..N {
        let high = (bytes[index * 2] as char).to_digit(16)?;
        let low = (bytes[index * 2 + 1] as char).to_digit(16)?;
        out[index] = ((high << 4) | low) as u8;
    }
    Some(out)
}

/// Lock a mutex, recovering from a poisoned lock rather than panicking.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
