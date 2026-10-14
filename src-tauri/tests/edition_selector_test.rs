use syncify_core_domain::quality::AudioTier;
use syncify_tauri_lib::download::resolver::{
    select_edition, CandidateAccount, DiscoveryBudget, DiscoveryCoverage, DiscoveryLedger,
    EditionCandidate, EditionDecision, EditionFacts, EditionRejection, EditionScope,
    EditionSelectionIntent, ProviderItemLock, ReleaseKind,
};

fn facts(version: Option<&str>) -> EditionFacts {
    EditionFacts {
        isrc: Some("GB-ABC-12-34567".into()),
        qualified_title: Some("Song (Radio Edit)".into()),
        artist: Some("Artist".into()),
        duration_ms: Some(200_000),
        declared_version: version.map(str::to_owned),
        confirmed_version: version.map(str::to_owned),
        track_number: Some(1),
        disc_number: Some(1),
    }
}
fn candidate(id: &str, version: Option<&str>, kind: ReleaseKind) -> EditionCandidate {
    EditionCandidate {
        item: ProviderItemLock {
            service_id: 2,
            service_track_id: id.into(),
            provider_release_id: Some(id.into()),
        },
        facts: facts(version),
        verified_release_id: Some(id.into()),
        release_kind: kind,
        available: true,
        advertised_tier: Some(AudioTier::HiRes),
        accounts: vec![account(12), account(13)],
    }
}
fn account(id: i64) -> CandidateAccount {
    CandidateAccount {
        account_id: id,
        is_active: true,
        streaming_entitled: Some(1),
    }
}
fn intent(scope: EditionScope) -> EditionSelectionIntent {
    EditionSelectionIntent {
        scope,
        reference_facts: None,
        preferred_account_id: None,
        strict_quality: true,
        minimum_tier: AudioTier::Lossless,
        replacement_authorized: false,
    }
}
#[test]
fn different_versions_with_same_isrc_never_merge() {
    let reference = facts(Some("radio edit"));
    let pool = [
        candidate("edit", Some("radio edit"), ReleaseKind::Single),
        candidate("live", Some("live"), ReleaseKind::Album),
    ];
    let selected = select_edition(
        &intent(EditionScope::Recording(reference)),
        DiscoveryCoverage::Complete,
        &pool,
    );
    assert!(
        matches!(selected, EditionDecision::Selected { item, .. } if item.service_track_id == "edit")
    );
    let unknown = [candidate("unconfirmed", None, ReleaseKind::Single)];
    assert_eq!(
        select_edition(
            &intent(EditionScope::Recording(facts(Some("radio edit")))),
            DiscoveryCoverage::Complete,
            &unknown
        ),
        EditionDecision::NeedsReview {
            reason: EditionRejection::MissingEvidence
        }
    );
}
#[test]
fn material_ambiguity_and_incomplete_coverage_are_review() {
    let pool = [
        candidate("album", Some("radio edit"), ReleaseKind::Album),
        candidate("single", Some("radio edit"), ReleaseKind::Single),
    ];
    let request = intent(EditionScope::Recording(facts(Some("radio edit"))));
    assert_eq!(
        select_edition(&request, DiscoveryCoverage::Complete, &pool),
        EditionDecision::NeedsReview {
            reason: EditionRejection::AmbiguousSource
        }
    );
    assert_eq!(
        select_edition(&request, DiscoveryCoverage::Partial, &pool),
        EditionDecision::NeedsReview {
            reason: EditionRejection::PartialCoverage
        }
    );
    assert_eq!(
        select_edition(&request, DiscoveryCoverage::Failed, &pool),
        EditionDecision::NeedsReview {
            reason: EditionRejection::PartialCoverage
        }
    );
}
#[test]
fn only_verified_equivalent_versions_can_prefer_non_compilation() {
    let album = candidate("album", Some("radio edit"), ReleaseKind::Album);
    let compilation = candidate("compilation", Some("radio edit"), ReleaseKind::Compilation);
    let mut request = intent(EditionScope::Recording(facts(Some("radio edit"))));
    request.replacement_authorized = true;
    assert!(
        matches!(select_edition(&request, DiscoveryCoverage::Complete, &[album.clone(), compilation.clone()]),
        EditionDecision::Selected { item, .. } if item.service_track_id == "album")
    );
    let mut unconfirmed = album;
    unconfirmed.facts.confirmed_version = None;
    assert!(
        matches!(select_edition(&request, DiscoveryCoverage::Complete, &[unconfirmed, compilation]),
        EditionDecision::Selected { item, .. } if item.service_track_id == "compilation")
    );
}
#[test]
fn exact_item_and_release_locks_reject_substitution() {
    let fixed = candidate("fixed", Some("radio edit"), ReleaseKind::Compilation);
    let other = candidate("other", Some("radio edit"), ReleaseKind::Album);
    let mut request = intent(EditionScope::Item(fixed.item.clone()));
    assert!(
        matches!(select_edition(&request, DiscoveryCoverage::Complete, &[other.clone(),fixed.clone()]),
        EditionDecision::Selected { item, .. } if item.service_track_id == "fixed")
    );
    request.replacement_authorized = true;
    assert_eq!(
        select_edition(&request, DiscoveryCoverage::Complete, &[other]),
        EditionDecision::NeedsReview {
            reason: EditionRejection::ItemLockMismatch
        }
    );
    let release = intent(EditionScope::Release {
        service_id: 2,
        provider_release_id: "fixed".into(),
    });
    assert!(matches!(
        select_edition(&release, DiscoveryCoverage::Complete, &[fixed]),
        EditionDecision::Selected { .. }
    ));
}
#[test]
fn declared_only_version_does_not_confirm_and_locked_conflict_blocks() {
    let mut offered = candidate("fixed", Some("radio edit"), ReleaseKind::Single);
    offered.facts.confirmed_version = None;
    let mut request = intent(EditionScope::Item(offered.item.clone()));
    request.reference_facts = Some(facts(Some("radio edit")));
    assert_eq!(
        select_edition(&request, DiscoveryCoverage::Complete, &[offered.clone()]),
        EditionDecision::NeedsReview {
            reason: EditionRejection::MissingEvidence
        }
    );
    offered.facts.confirmed_version = Some("live".into());
    assert_eq!(
        select_edition(&request, DiscoveryCoverage::Complete, &[offered]),
        EditionDecision::NeedsReview {
            reason: EditionRejection::RecordingConflict
        }
    );
}
#[test]
fn release_track_position_only_matters_inside_verified_release() {
    let mut offered = candidate("release", Some("radio edit"), ReleaseKind::Album);
    let mut request = intent(EditionScope::Release {
        service_id: 2,
        provider_release_id: "release".into(),
    });
    let mut reference = facts(Some("radio edit"));
    reference.track_number = Some(4);
    request.reference_facts = Some(reference);
    assert_eq!(
        select_edition(&request, DiscoveryCoverage::Complete, &[offered.clone()]),
        EditionDecision::NeedsReview {
            reason: EditionRejection::RecordingConflict
        }
    );
    offered.facts.track_number = Some(4);
    assert!(matches!(
        select_edition(&request, DiscoveryCoverage::Complete, &[offered]),
        EditionDecision::Selected { .. }
    ));
}
#[test]
fn quality_and_account_order_follow_verified_item() {
    let mut offered = candidate("fixed", Some("radio edit"), ReleaseKind::Unknown);
    offered.advertised_tier = Some(AudioTier::Lossy);
    let mut request = intent(EditionScope::Item(offered.item.clone()));
    assert_eq!(
        select_edition(&request, DiscoveryCoverage::Complete, &[offered.clone()]),
        EditionDecision::Rejected {
            reason: EditionRejection::QualityBelowFloor
        }
    );
    request.strict_quality = false;
    assert!(
        matches!(select_edition(&request, DiscoveryCoverage::Complete, &[offered.clone()]),
        EditionDecision::Selected { account, allowed_tier: AudioTier::Lossy, .. } if account.account_id == 12)
    );
    request.preferred_account_id = Some(13);
    assert!(
        matches!(select_edition(&request, DiscoveryCoverage::Complete, &[offered.clone()]),
        EditionDecision::Selected { account, .. } if account.account_id == 13)
    );
    offered.accounts[0].streaming_entitled = Some(0);
    request.preferred_account_id = None;
    assert!(
        matches!(select_edition(&request, DiscoveryCoverage::Complete, &[offered]),
        EditionDecision::Selected { account, .. } if account.account_id == 13)
    );
}
#[test]
fn paged_discovery_and_detail_calls_have_hard_limits() {
    let mut ledger = DiscoveryLedger::new(DiscoveryBudget {
        max_pages: 2,
        max_detail_calls: 1,
        max_candidates: 3,
    });
    assert!(ledger.observe_page(2, true));
    assert!(!ledger.observe_page(1, true));
    assert_eq!(ledger.coverage, DiscoveryCoverage::Partial);
    assert!(!ledger.observe_page(1, false));
    assert!(!ledger.reserve_detail(false));
    assert!(ledger.reserve_detail(true));
    assert!(!ledger.reserve_detail(true));
    ledger.fail();
    assert_eq!(ledger.coverage, DiscoveryCoverage::Failed);
}
