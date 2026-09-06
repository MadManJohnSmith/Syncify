//! Tests for TASK-143: Purga de Sufijos Remaster Redundantes en Títulos de Pista
//!
//! Validates:
//! 1. Sufijo remaster redundante eliminado cuando el álbum contiene "Remaster", "Remastered", "Deluxe", "Anniversary", etc.
//! 2. Sufijo remaster conservado cuando el álbum NO contiene marcadores de remaster.
//! 3. Preservación intacta de variantes legítimas: (Live), (Remix), (Radio Edit), (Acoustic), (Extended Mix).
//! 4. Pistas complejas que tengan combinaciones como `(Live / 2011 Remaster)` -> `(Live)`.
//! 5. Integración con `import_cache::process_track_title`.

use syncify_core_domain::metadata::{has_album_remaster_marker, strip_redundant_remaster};
use syncify_tauri_lib::import_cache::process_track_title;

#[test]
fn test_album_remaster_marker_detection() {
    // True cases: album declares edition / remaster
    assert!(has_album_remaster_marker("Dziewczyna Szamana (2021 Remaster)"));
    assert!(has_album_remaster_marker("Heroes (Remastered)"));
    assert!(has_album_remaster_marker("Nevermind (30th Anniversary Super Deluxe)"));
    assert!(has_album_remaster_marker("OK Computer (Deluxe Edition)"));
    assert!(has_album_remaster_marker("Abbey Road (50th Anniversary)"));
    assert!(has_album_remaster_marker("Disintegration (Deluxe Edition)"));
    assert!(has_album_remaster_marker("Parklife (Special Edition)"));
    assert!(has_album_remaster_marker("The Joshua Tree (2007 Reissue)"));
    assert!(has_album_remaster_marker("In Rainbows (Expanded Edition)"));

    // False cases: standard studio / normal albums
    assert!(!has_album_remaster_marker("Dziewczyna Szamana"));
    assert!(!has_album_remaster_marker("Heroes"));
    assert!(!has_album_remaster_marker("Nevermind"));
    assert!(!has_album_remaster_marker("OK Computer"));
    assert!(!has_album_remaster_marker("Abbey Road"));
    assert!(!has_album_remaster_marker("Neon Golden"));
}

#[test]
fn test_redundant_remaster_stripped_when_album_declares_remaster() {
    // 1. Parenthesized suffixes (2009 Remaster), (Remastered), etc.
    assert_eq!(
        strip_redundant_remaster(
            "Dziewczyna Szamana (2021 Remaster)",
            "Dziewczyna Szamana (2021 Remaster)"
        ),
        "Dziewczyna Szamana"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes (Remastered)", "Heroes (Remastered)"),
        "Heroes"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes (2009 Remaster)", "Heroes (Deluxe Edition)"),
        "Heroes"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes (Remastered 2017)", "Heroes (2017 Remaster)"),
        "Heroes"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes (2021 Remastered Version)", "Heroes (Remastered)"),
        "Heroes"
    );

    // 2. Bracketed suffixes [2011 Remaster], [Remastered]
    assert_eq!(
        strip_redundant_remaster("Heroes [2011 Remaster]", "Heroes [Remastered]"),
        "Heroes"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes [Remastered]", "Heroes (Deluxe Edition)"),
        "Heroes"
    );

    // 3. Hyphen suffixes - 2011 Remaster, - Remastered
    assert_eq!(
        strip_redundant_remaster("Heroes - 2011 Remaster", "Heroes (2011 Remaster)"),
        "Heroes"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes - Remastered", "Heroes (Deluxe Edition)"),
        "Heroes"
    );
    assert_eq!(
        strip_redundant_remaster(
            "Heroes - 2009 Digital Remaster",
            "Heroes (30th Anniversary)"
        ),
        "Heroes"
    );
    assert_eq!(
        strip_redundant_remaster(
            "Heroes - 2021 Remastered Version",
            "Heroes (2021 Remaster)"
        ),
        "Heroes"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes - Remaster", "Heroes (Remastered)"),
        "Heroes"
    );
}

#[test]
fn test_remaster_suffix_preserved_when_album_has_no_remaster_marker() {
    // Remaster suffix MUST be preserved if the album doesn't declare it (not redundant)
    assert_eq!(
        strip_redundant_remaster("Dziewczyna Szamana (2021 Remaster)", "Dziewczyna Szamana"),
        "Dziewczyna Szamana (2021 Remaster)"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes (Remastered)", "Heroes"),
        "Heroes (Remastered)"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes - 2011 Remaster", "Heroes"),
        "Heroes - 2011 Remaster"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes [2011 Remaster]", "Heroes"),
        "Heroes [2011 Remaster]"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes (2021 Remastered Version)", "Heroes"),
        "Heroes (2021 Remastered Version)"
    );
}

#[test]
fn test_preservation_of_legitimate_variants() {
    let remaster_album = "Heroes (2011 Remaster)";

    // (Live)
    assert_eq!(
        strip_redundant_remaster("Heroes (Live)", remaster_album),
        "Heroes (Live)"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes - Live", remaster_album),
        "Heroes - Live"
    );

    // (Remix)
    assert_eq!(
        strip_redundant_remaster("Heroes (Remix)", remaster_album),
        "Heroes (Remix)"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes - Remix", remaster_album),
        "Heroes - Remix"
    );

    // (Radio Edit)
    assert_eq!(
        strip_redundant_remaster("Heroes (Radio Edit)", remaster_album),
        "Heroes (Radio Edit)"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes - Radio Edit", remaster_album),
        "Heroes - Radio Edit"
    );

    // (Acoustic)
    assert_eq!(
        strip_redundant_remaster("Heroes (Acoustic)", remaster_album),
        "Heroes (Acoustic)"
    );
    assert_eq!(
        strip_redundant_remaster("Heroes - Acoustic", remaster_album),
        "Heroes - Acoustic"
    );

    // (Extended Mix)
    assert_eq!(
        strip_redundant_remaster("Heroes (Extended Mix)", remaster_album),
        "Heroes (Extended Mix)"
    );
}

#[test]
fn test_complex_track_title_remaster_purging() {
    let remaster_album = "Heroes (2011 Remaster)";

    // (Live / 2011 Remaster) -> (Live)
    assert_eq!(
        strip_redundant_remaster("Heroes (Live / 2011 Remaster)", remaster_album),
        "Heroes (Live)"
    );

    // (2011 Remaster / Live) -> (Live)
    assert_eq!(
        strip_redundant_remaster("Heroes (2011 Remaster / Live)", remaster_album),
        "Heroes (Live)"
    );

    // (Live - 2011 Remaster) -> (Live)
    assert_eq!(
        strip_redundant_remaster("Heroes (Live - 2011 Remaster)", remaster_album),
        "Heroes (Live)"
    );

    // (Live, 2011 Remaster) -> (Live)
    assert_eq!(
        strip_redundant_remaster("Heroes (Live, 2011 Remaster)", remaster_album),
        "Heroes (Live)"
    );

    // (Live; 2011 Remaster) -> (Live)
    assert_eq!(
        strip_redundant_remaster("Heroes (Live; 2011 Remaster)", remaster_album),
        "Heroes (Live)"
    );

    // [Live / 2011 Remaster] -> [Live]
    assert_eq!(
        strip_redundant_remaster("Heroes [Live / 2011 Remaster]", remaster_album),
        "Heroes [Live]"
    );

    // (Remix / 2020 Remaster) -> (Remix)
    assert_eq!(
        strip_redundant_remaster("Heroes (Remix / 2020 Remaster)", remaster_album),
        "Heroes (Remix)"
    );

    // (Radio Edit / Remastered) -> (Radio Edit)
    assert_eq!(
        strip_redundant_remaster("Heroes (Radio Edit / Remastered)", remaster_album),
        "Heroes (Radio Edit)"
    );

    // (Acoustic / 2008 Remaster) -> (Acoustic)
    assert_eq!(
        strip_redundant_remaster("Heroes (Acoustic / 2008 Remaster)", remaster_album),
        "Heroes (Acoustic)"
    );

    // (Extended Mix / 2014 Remaster) -> (Extended Mix)
    assert_eq!(
        strip_redundant_remaster("Heroes (Extended Mix / 2014 Remaster)", remaster_album),
        "Heroes (Extended Mix)"
    );

    // (Live) (2011 Remaster) -> (Live)
    assert_eq!(
        strip_redundant_remaster("Heroes (Live) (2011 Remaster)", remaster_album),
        "Heroes (Live)"
    );

    // (2011 Remaster) (Live) -> (Live)
    assert_eq!(
        strip_redundant_remaster("Heroes (2011 Remaster) (Live)", remaster_album),
        "Heroes (Live)"
    );

    // (Live) - 2011 Remaster -> (Live)
    assert_eq!(
        strip_redundant_remaster("Heroes (Live) - 2011 Remaster", remaster_album),
        "Heroes (Live)"
    );

    // - Live / 2011 Remaster -> - Live
    assert_eq!(
        strip_redundant_remaster("Heroes - Live / 2011 Remaster", remaster_album),
        "Heroes - Live"
    );
}

#[test]
fn test_import_cache_process_track_title() {
    // 1. With remaster album
    assert_eq!(
        process_track_title(
            "Dziewczyna Szamana (2021 Remaster)",
            Some("Dziewczyna Szamana (2021 Remaster)")
        ),
        "Dziewczyna Szamana"
    );

    // 2. Without album (None)
    assert_eq!(
        process_track_title("Heroes (2011 Remaster)", None),
        "Heroes (2011 Remaster)"
    );

    // 3. With non-remaster album
    assert_eq!(
        process_track_title("Heroes (2011 Remaster)", Some("Heroes")),
        "Heroes (2011 Remaster)"
    );

    // 4. With dirty whitespace and HTML entities
    assert_eq!(
        process_track_title(
            "  Heroes &amp; Villains  (2011 Remaster)  ",
            Some("Heroes & Villains (Deluxe Edition)")
        ),
        "Heroes & Villains"
    );
}
