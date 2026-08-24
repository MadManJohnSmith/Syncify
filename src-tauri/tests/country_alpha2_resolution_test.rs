//! S177 ISO 3166-1 Alpha-2 Resolution and Country Hardening Test Suite
//!
//! Validates:
//! 1. Resolution of ISO 3166-1 alpha-2 codes (including AL -> Albania, BH -> Bahrain).
//! 2. Resolution of alpha-3 codes (ALB -> Albania, BHR -> Bahrain).
//! 3. Rejection of unmapped / invalid 2-letter codes (e.g. XX, ZZ) in fuse_countries.
//! 4. Full canonical English country name output across all providers.

use syncify_metadata_domain::{fuse_countries, resolve_country, CountryResolution};

#[test]
fn test_alpha2_albania_and_bahrain_resolution() {
    // Direct resolution
    let res_al = resolve_country("AL");
    assert_eq!(
        res_al,
        CountryResolution::Country {
            iso_alpha2: "AL".to_string(),
            canonical_name: "Albania".to_string(),
        }
    );

    let res_bh = resolve_country("BH");
    assert_eq!(
        res_bh,
        CountryResolution::Country {
            iso_alpha2: "BH".to_string(),
            canonical_name: "Bahrain".to_string(),
        }
    );

    // Alpha-3 resolution
    let res_alb = resolve_country("ALB");
    assert_eq!(
        res_alb,
        CountryResolution::Country {
            iso_alpha2: "AL".to_string(),
            canonical_name: "Albania".to_string(),
        }
    );

    let res_bhr = resolve_country("BHR");
    assert_eq!(
        res_bhr,
        CountryResolution::Country {
            iso_alpha2: "BH".to_string(),
            canonical_name: "Bahrain".to_string(),
        }
    );
}

#[test]
fn test_fuse_countries_with_alpha2_codes() {
    let inputs_al = [("AL", "qobuz", 0.90)];
    assert_eq!(fuse_countries(&inputs_al), Some("Albania".to_string()));

    let inputs_bh = [("BH", "tidal", 0.90)];
    assert_eq!(fuse_countries(&inputs_bh), Some("Bahrain".to_string()));

    let inputs_us = [("US", "musicbrainz", 0.85)];
    assert_eq!(fuse_countries(&inputs_us), Some("United States".to_string()));
}

#[test]
fn test_unknown_alpha2_code_rejection() {
    let res_xx = resolve_country("XX");
    assert_eq!(res_xx, CountryResolution::Unknown("XX".to_string()));

    let res_zz = resolve_country("ZZ");
    assert_eq!(res_zz, CountryResolution::Unknown("ZZ".to_string()));

    // fuse_countries must reject unrecognized 2-letter codes instead of emitting raw code
    let inputs_xx = [("XX", "musicbrainz", 0.85)];
    assert_eq!(fuse_countries(&inputs_xx), None);
}

#[test]
fn test_comprehensive_alpha2_sample() {
    let test_cases = [
        ("ES", "Spain"),
        ("MX", "Mexico"),
        ("GB", "United Kingdom"),
        ("DE", "Germany"),
        ("FR", "France"),
        ("JP", "Japan"),
        ("CA", "Canada"),
        ("AU", "Australia"),
        ("IT", "Italy"),
        ("BR", "Brazil"),
        ("AR", "Argentina"),
        ("IS", "Iceland"),
        ("AD", "Andorra"),
        ("AG", "Antigua and Barbuda"),
        ("NZ", "New Zealand"),
    ];

    for (code, expected_name) in test_cases {
        let res = resolve_country(code);
        match res {
            CountryResolution::Country { canonical_name, .. } => {
                assert_eq!(canonical_name, expected_name, "Mismatch for code {}", code);
            }
            _ => panic!("Expected CountryResolution::Country for code {}", code),
        }
    }
}
