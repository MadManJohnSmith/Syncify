//! Pure rules and invariant logic for cover art preservation in FLAC files.

use serde::{Deserialize, Serialize};

/// Classification of cover art image format and animation capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoverType {
    AnimatedWebp,
    StaticWebp,
    StaticJpeg,
    StaticPng,
    None,
    Unknown,
}

impl CoverType {
    pub fn is_webp(&self) -> bool {
        matches!(self, CoverType::AnimatedWebp | CoverType::StaticWebp)
    }

    pub fn is_animated(&self) -> bool {
        matches!(self, CoverType::AnimatedWebp)
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            CoverType::AnimatedWebp | CoverType::StaticWebp => "image/webp",
            CoverType::StaticJpeg => "image/jpeg",
            CoverType::StaticPng => "image/png",
            CoverType::None | CoverType::Unknown => "application/octet-stream",
        }
    }
}

/// Evaluation result for cover update decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverUpdateDecision {
    /// Overwrite or insert the incoming cover into CoverFront block.
    Overwrite,
    /// Preserve existing CoverFront block (e.g. existing WebP protected from static JPEG/PNG).
    PreserveExisting,
}

/// Pure cover preservation policy engine.
pub struct CoverPreservationPolicy;

impl CoverPreservationPolicy {
    /// Evaluate whether an existing CoverFront block should be preserved against an incoming payload.
    ///
    /// # Invariant:
    /// If the existing CoverFront is a WebP image (`CoverType::AnimatedWebp` or `CoverType::StaticWebp`),
    /// it must NEVER be overwritten by a static JPEG or PNG unless the incoming cover is also a WebP.
    pub fn evaluate(existing: CoverType, incoming: CoverType) -> CoverUpdateDecision {
        match (existing, incoming) {
            // If existing is WebP and incoming is static JPEG/PNG, preserve existing WebP!
            (CoverType::AnimatedWebp | CoverType::StaticWebp, CoverType::StaticJpeg | CoverType::StaticPng | CoverType::None | CoverType::Unknown) => {
                CoverUpdateDecision::PreserveExisting
            }
            // If incoming is WebP or no WebP cover exists currently, allow update
            _ => CoverUpdateDecision::Overwrite,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cover_preservation_invariant() {
        // Invariant: Animated WebP must NOT be overwritten by static JPEG
        assert_eq!(
            CoverPreservationPolicy::evaluate(CoverType::AnimatedWebp, CoverType::StaticJpeg),
            CoverUpdateDecision::PreserveExisting
        );

        // Invariant: Animated WebP must NOT be overwritten by static PNG
        assert_eq!(
            CoverPreservationPolicy::evaluate(CoverType::AnimatedWebp, CoverType::StaticPng),
            CoverUpdateDecision::PreserveExisting
        );

        // Invariant: Static WebP must NOT be overwritten by static JPEG
        assert_eq!(
            CoverPreservationPolicy::evaluate(CoverType::StaticWebp, CoverType::StaticJpeg),
            CoverUpdateDecision::PreserveExisting
        );

        // Overwrite is allowed if incoming is a new Animated WebP
        assert_eq!(
            CoverPreservationPolicy::evaluate(CoverType::AnimatedWebp, CoverType::AnimatedWebp),
            CoverUpdateDecision::Overwrite
        );

        // Overwrite is allowed if incoming is a WebP over static JPEG
        assert_eq!(
            CoverPreservationPolicy::evaluate(CoverType::StaticJpeg, CoverType::AnimatedWebp),
            CoverUpdateDecision::Overwrite
        );

        // Overwrite is allowed if existing was static JPEG and incoming is static PNG / JPEG
        assert_eq!(
            CoverPreservationPolicy::evaluate(CoverType::StaticJpeg, CoverType::StaticJpeg),
            CoverUpdateDecision::Overwrite
        );

        // Overwrite is allowed if existing was None
        assert_eq!(
            CoverPreservationPolicy::evaluate(CoverType::None, CoverType::StaticJpeg),
            CoverUpdateDecision::Overwrite
        );
    }
}
