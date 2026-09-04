-- Auto-generated backfill script for downloads and lyrics
BEGIN TRANSACTION;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    12463, 2, '/home/alan/Music/Syncify/Wolfmother/New Crown/07 - _I Ain''t Got No_.flac', 'FLAC', 28857239,
    '90e569b886859c26e38c50d0abf6b2619568e64207c2807eb2675188586734a8', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'bd8e7754-dc0a-48cd-b26c-5920ae34e638', CURRENT_TIMESTAMP,
    'qobuz', '29820734', 'qobuz', '29820734',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    15342, 2, '/home/alan/Music/Syncify/Ileana Cotrubas/Verdi_ La Traviata/Disc 1/16 - _Ah! Dite alla giovine_.flac', 'FLAC', 16380190,
    'f5c242e409c647ccd221cb19797a40454fca0c552ee058a6c176ba6d0f954bb8', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'cb22770c-8050-4f7c-9b78-2e029defed4d', CURRENT_TIMESTAMP,
    'qobuz', '613116', 'qobuz', '613116',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    22990, 2, '/home/alan/Music/Syncify/Various Artists/100 Classical Favourites/Disc 5/03 - Rundfunkchor Leipzig - _Treulich geführt ziehet dahin_.flac', 'FLAC', 15715464,
    '81098b1bfad50595308354c59b0fbdc9c21743e3920e2281ae7d5b18b282db2f', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '9ae4d344-e3ca-46ea-a937-7a7b9a588a4c', CURRENT_TIMESTAMP,
    'qobuz', '559972', 'qobuz', '559972',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    2006, 2, '/home/alan/Music/Syncify/Various Artists/Synth Pop/31 - Sandra - (I''ll Never Be) Maria Magdalena.flac', 'FLAC', 27451348,
    'bb6092841c564d67ad420c24710efb96c198d77f852a16fd8d619edc546990b8', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '4625675f-be2b-48a7-8deb-b898e1ce8c16', CURRENT_TIMESTAMP,
    'qobuz', '795122', 'qobuz', '795122',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    946, 2, '/home/alan/Music/Syncify/Various Artists/Girls Night In/Disc 2/01 - Bill Medley & Jennifer Warnes - (I''ve Had) The Time of My Life.flac', 'FLAC', 34709996,
    '402a224f50d2e208b8c502225703089d38ab93ef2db9d11503465d52ea88c426', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'ecebd1a6-8265-4c2b-b616-6b833118a925', CURRENT_TIMESTAMP,
    'qobuz', '6373570', 'qobuz', '6373570',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    5853, 2, '/home/alan/Music/Syncify/David Bowie/_Heroes_/03 - _Heroes_.flac', 'FLAC', 44654891,
    '760ffcbfd16b326b0e866174dc2366c2b436911441bbd921e8f107eb75c07632', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '27d85cb5-05ff-4351-821c-e78d294bc14a', CURRENT_TIMESTAMP,
    'qobuz', '47254171', 'qobuz', '47254171',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    23, 3, '/home/alan/Music/Syncify/Garbage/2024 - Absolute Garbage (Special Edition)/06 - #1 Crush.m4a', 'AAC', 11676195,
    '3bfe08fdc18e02d7be96d70b705c09d1591f76a98a38320732a6dd11183fa5ec', NULL, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, NULL, CURRENT_TIMESTAMP,
    'tidal', NULL, 'tidal', NULL,
    NULL, 'isrc', 1.0, NULL,
    'high', 'high', 'AAC', 'AAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    5080, 2, '/home/alan/Music/Syncify/Garbage/Garbage/Disc 1/18 - #1 Crush.flac', 'FLAC', 109468394,
    '6c810bb4e3e1e6f915e1e61157c7090735fc40bc4ee1d40e4e35bd01057216ef', 24, 96000, 100, CURRENT_TIMESTAMP,
    NULL, 0, '77a2c1e8-c2bf-4eb1-a62d-f554723b50a6', CURRENT_TIMESTAMP,
    'qobuz', '387853180', 'qobuz', '387853180',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    14595, 2, '/home/alan/Music/Syncify/Garbage/Garbage/Disc 1/18 - #1 Crush (1).flac', 'FLAC', 35759802,
    'c15532ee84d1ac7686fe6d8f0842c536cdc61055c2500d4dc5b4cd11c7b68857', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '77a2c1e8-c2bf-4eb1-a62d-f554723b50a6', CURRENT_TIMESTAMP,
    'qobuz', '387762074', 'qobuz', '387762074',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    23353, 2, '/home/alan/Music/Syncify/Christina Pluhar/Handel goes Wild/02 - _Venti, turbini_ (From Rinaldo, HWV 7b) [Arr. Pluhar].flac', 'FLAC', 120018052,
    '886a5bab27cc4b5afb1a03dcc08a98b726a962ae439a8d0e5c606eb071136e95', 24, 96000, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'bc41bb35-b4d8-4b9a-84d1-46b772adf753', CURRENT_TIMESTAMP,
    'qobuz', '42458361', 'qobuz', '42458361',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    124, 2, '/home/alan/Music/Syncify/Riverside/Love, Fear and the Time Machine/03 - #Addicted.flac', 'FLAC', 32510736,
    '0eb8fecd96351b979fff7015848be37b64cbb28a4bb79bc3253101f34601d7dd', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'ce1f83d5-a01e-4ed9-8359-6d27c741600f', CURRENT_TIMESTAMP,
    'qobuz', '28657241', 'qobuz', '28657241',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    11792, 2, '/home/alan/Music/Syncify/Sir Sly/Don''t You Worry, Honey/03 - &Run.flac', 'FLAC', 45473867,
    'e366beb8626fa7c9011b2f242c09ef9976bac25490e54505e3188a35ba56f420', 24, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'b03e4f0a-0f51-40d4-8a84-b0d242f2bdcb', CURRENT_TIMESTAMP,
    'qobuz', '41936933', 'qobuz', '41936933',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    11509, 2, '/home/alan/Music/Syncify/The Neighbourhood/#000000 & #FFFFFF (No DJ Version)/11 - #icanteven.flac', 'FLAC', 24253603,
    'b98de129aaf3a48c0ae88629041f36b91714944d7eebdc4ada9de885b631db8d', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '759c0248-6192-4932-a295-1696d73b1a47', CURRENT_TIMESTAMP,
    'qobuz', '45380752', 'qobuz', '45380752',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    8296, 2, '/home/alan/Music/Syncify/The Neighbourhood/#000000 & #FFFFFF (No DJ Version)/01 - _NSTYNCT.flac', 'FLAC', 26930788,
    '404a705a5e8161164978014149e798b0929863a2a90c4cfe11999b4fc1926a04', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '759c0248-6192-4932-a295-1696d73b1a47', CURRENT_TIMESTAMP,
    'qobuz', '45380742', 'qobuz', '45380742',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    11512, 2, '/home/alan/Music/Syncify/The Neighbourhood/#000000 & #FFFFFF (No DJ Version)/05 - 1 of those Weaks.flac', 'FLAC', 19683535,
    '592f20f964abc555dfa2ab0616cc048bfbc3866f4b85d28ae4b8d97410180d1c', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '759c0248-6192-4932-a295-1696d73b1a47', CURRENT_TIMESTAMP,
    'qobuz', '45380746', 'qobuz', '45380746',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    1917, 2, '/home/alan/Music/Syncify/Billie Eilish/dont smile at me/09 - &burn.flac', 'FLAC', 34477919,
    '770eec4529d51159e6413b7e9fbeffb0d7361d266e8ed0223be596c8233f2232', 24, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '7f6c1ffb-a17d-427c-9f3e-b942737c6572', CURRENT_TIMESTAMP,
    'qobuz', '89930714', 'qobuz', '89930714',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    20851, 2, '/home/alan/Music/Syncify/The Connells/Ring/03 - ''74-''75.flac', 'FLAC', 33023754,
    '1cbf354c4cddcaa7a3ec328ffe9d2e68e136e7ca65dc5469c08f8a0e3e15c0dd', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '953fc015-9f61-422b-a085-c80b694ae896', CURRENT_TIMESTAMP,
    'qobuz', '19200745', 'qobuz', '19200745',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    15824, 2, '/home/alan/Music/Syncify/Amy Winehouse/Frank/Disc 2/07 - ''Round Midnight.flac', 'FLAC', 27827969,
    '5f34768a555c887d51f31e315b13620a6f0a771c83a96917efd2c2a502b3bb16', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '557830c3-3ad3-4c20-be0e-a1e14c1ef773', CURRENT_TIMESTAMP,
    'qobuz', '10874625', 'qobuz', '10874625',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    21412, 2, '/home/alan/Music/Syncify/New York Dolls/''Cause I Sez So/01 - ''Cause I Sez So.flac', 'FLAC', 26515903,
    '0efa47a35eaa89d363c6f9547d9df1bc26158cf1e008dc758cbc30253cd9e8f1', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '7b20271d-e4a5-47c2-bf64-6175230a8e9b', CURRENT_TIMESTAMP,
    'qobuz', '45243422', 'qobuz', '45243422',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    21419, 2, '/home/alan/Music/Syncify/New York Dolls/In Too Much Too Soon/04 - (There''s Gonna Be A) Showdown.flac', 'FLAC', 87947569,
    '9a7db896a8e0445984e695699e6a6edda359f4ae7fa736aee0483ce728277fd1', 24, 96000, 100, CURRENT_TIMESTAMP,
    NULL, 0, '6c1e2f43-d2fb-4974-a483-652a6f3be114', CURRENT_TIMESTAMP,
    'qobuz', '18925060', 'qobuz', '18925060',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    22882, 2, '/home/alan/Music/Syncify/Anderson .Paak/''Til It''s Over/01 - ''Til It''s Over.flac', 'FLAC', 23901981,
    'dad453eca13492cc10dc8454c2f22098f7e689955fd6d6fd073f971061743560', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'fd458ca9-1624-465a-84fe-d076ae8b02f3', CURRENT_TIMESTAMP,
    'qobuz', '55893798', 'qobuz', '55893798',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    8605, 2, '/home/alan/Music/Syncify/Alexandra Savior/Belladonna of Sadness/09 - ''Til You''re Mine.flac', 'FLAC', 22666295,
    '13a028c0c701a0b07f0fa54e3c5f974bd4d7236ee6e9e8562bfbebad01901d67', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '55bbbf4d-c2ee-4183-9b10-9835d6d0c90b', CURRENT_TIMESTAMP,
    'qobuz', '39720123', 'qobuz', '39720123',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    21393, 2, '/home/alan/Music/Syncify/Mott The Hoople/The Hoople/13 - (Do You Remember) The Saturday Gigs_.flac', 'FLAC', 31385809,
    'a90deb0bb2e2994918567a1d1dc0bba647006c257eaf6fa36e4b9ba31aef3599', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '5ea0411e-97a5-4195-9dd9-20e4a81c4872', CURRENT_TIMESTAMP,
    'qobuz', '31892552', 'qobuz', '31892552',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    11664, 2, '/home/alan/Music/Syncify/Blue Öyster Cult/Agents Of Fortune/03 - (Don''t Fear) The Reaper.flac', 'FLAC', 117444757,
    'ea9192555208b1f6c590d37b50f59ab8785675540c47c91d57f5b8fe944ee0fe', 24, 96000, 100, CURRENT_TIMESTAMP,
    NULL, 0, '784b85e6-1b4e-4f12-a8ec-38f791f5cb96', CURRENT_TIMESTAMP,
    'qobuz', '33443762', 'qobuz', '33443762',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    15709, 2, '/home/alan/Music/Syncify/Bryan Adams/Waking Up The Neighbours/12 - (Everything I Do) I Do It For You.flac', 'FLAC', 42469757,
    'e8743ef3955dabeb1784774653fd719d44947eca330828b94ebd47dd8f8a7b4a', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'a1c8889b-1c4d-4336-965c-f28abf561f91', CURRENT_TIMESTAMP,
    'qobuz', '647366', 'qobuz', '647366',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    3878, 2, '/home/alan/Music/Syncify/YACHT/(Downtown) Dancing/01 - (Downtown) Dancing.flac', 'FLAC', 65919174,
    '78fa3843918fe5dd9af694017a4509c44ea328c60079ec94dfe6a16969120bed', 24, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'd44a3934-e547-40f1-8cb9-95af4659fdac', CURRENT_TIMESTAMP,
    'qobuz', '62521816', 'qobuz', '62521816',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    20232, 2, '/home/alan/Music/Syncify/Poison/Poison''s Greatest Hits 1986-1996/12 - (Flesh & Blood) Sacrifice.flac', 'FLAC', 39733689,
    'aecbec590d3cd3009de62d7733d19998e8b7cf84bac05a8b04973167e94ca3c1', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '680fdbc7-297a-4974-8ea5-cee9b5343dab', CURRENT_TIMESTAMP,
    'qobuz', '1971075', 'qobuz', '1971075',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    19843, 2, '/home/alan/Music/Syncify/Madeleine Peyroux/Dreamland/04 - (Getting Some) Fun out of Life.flac', 'FLAC', 19989199,
    '14d8a409e131cacc9b5442e49931409b2f22be2a004cf8424ed4c187fdbf1b31', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'b578f83a-a8d7-49e7-82f0-7f6e6d5c0671', CURRENT_TIMESTAMP,
    'qobuz', '2798336', 'qobuz', '2798336',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    5214, 2, '/home/alan/Music/Syncify/Johnny Cash/Silver/05 - (Ghost) Riders in the Sky.flac', 'FLAC', 24646589,
    'b7cc111a7baa13fee78d0f36a3f25fad0d8dd03be708d02a8bff055fbcd9c060', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'cc624430-544c-4436-a5dd-d6437f7f6040', CURRENT_TIMESTAMP,
    'qobuz', '14894133', 'qobuz', '14894133',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    21208, 2, '/home/alan/Music/Syncify/Weezer/Weezer (White Album)/04 - (Girl We Got A) Good Thing.flac', 'FLAC', 26529762,
    'feee2fecdfae9cbd4e96f28f5a614a4afcd8581d0c264d008cd2ae1a0db608cb', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'f77baffd-2c4b-4ce5-800f-d446b0295f22', CURRENT_TIMESTAMP,
    'qobuz', '35470436', 'qobuz', '35470436',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    6128, 2, '/home/alan/Music/Syncify/The Rolling Stones/Rolling Stones Chronicles/02 - (I Can''t Get No) Satisfaction.flac', 'FLAC', 14352015,
    '19e399a85df2fd8ed70026038859b2fadbd705849c2113daf5ac24de401dc055', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '93854c40-ba7b-4ce1-ae25-f98316161073', CURRENT_TIMESTAMP,
    'qobuz', '200685755', 'qobuz', '200685755',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    15841, 2, '/home/alan/Music/Syncify/UB40/The Very Best Of UB40/12 - (I Can''t Help) Falling In Love With You.flac', 'FLAC', 24912729,
    '77f1235c2dc54083a5c16d760102f73b648a4ae2f2c7114fde056687d69f7966', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'cfac2c4e-a7cd-4b47-a332-85276d1a5a21', CURRENT_TIMESTAMP,
    'qobuz', '795595', 'qobuz', '795595',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    14653, 2, '/home/alan/Music/Syncify/Kay Kyser/Best Of The Big Bands/13 - (I Got Spurs That) Jingle, Jangle, Jingle.flac', 'FLAC', 15536630,
    '57aca11e33aad9f852dfc907eed7b511ff2b32fec81e1853c5500f92e5a5b003', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '143e0978-70dd-450e-a358-e96d7b59a83d', CURRENT_TIMESTAMP,
    'qobuz', '58277', 'qobuz', '58277',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    4722, 2, '/home/alan/Music/Syncify/Cutting Crew/The Best Of Cutting Crew/01 - (I Just) Died In Your Arms.flac', 'FLAC', 30853238,
    'b66e720aa26ec233ed369c83d1fc59083ea9cf24d038d52b13e22b8d6dc7988e', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '8251ad69-d06c-41c3-b899-304cf19941a6', CURRENT_TIMESTAMP,
    'qobuz', '1942318', 'qobuz', '1942318',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    2195, 2, '/home/alan/Music/Syncify/The Monkees/More of The Monkees/06 - (I''m Not Your) Steppin'' Stone.flac', 'FLAC', 18768224,
    '5a16e1359c0716bf417c8cdc85ba167dca77fd7115752c582ee0bfe524719037', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'f9963412-ad64-4a37-b28e-d0e7fcda35a3', CURRENT_TIMESTAMP,
    'qobuz', '11677159', 'qobuz', '11677159',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    6233, 2, '/home/alan/Music/Syncify/Nick Waterhouse/Time''s All Gone/06 - (If) You Want Trouble.flac', 'FLAC', 19492621,
    'e465d9d5c981f391a599d273de79d01823c5ea8f18e094d2e5056b2e796a1bcd', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '88b8ae87-4d8b-4d75-a39a-361152f591da', CURRENT_TIMESTAMP,
    'qobuz', '145993844', 'qobuz', '145993844',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    5073, 2, '/home/alan/Music/Syncify/John Lennon/Double Fantasy_ Stripped Down/Disc 2/01 - (Just Like) Starting Over.flac', 'FLAC', 26466915,
    '47deba1a80f22f237b67836af99dd77d6691e5eb040434b330badc7b27bcb35c', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '5dfcab3e-8bce-43cb-9c4b-80609d939073', CURRENT_TIMESTAMP,
    'qobuz', '2493982', 'qobuz', '2493982',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    7216, 2, '/home/alan/Music/Syncify/Elvis Presley/Elvis 30 #1 Hits/07 - (Let Me Be Your) Teddy Bear.flac', 'FLAC', 7000693,
    '77dd16ebef9de3341b33c2ccfb87e8f127d5d6af13083bd49377ee5c0077ca98', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'e177c292-11b5-4b1e-8690-d671394015f5', CURRENT_TIMESTAMP,
    'qobuz', '2719203', 'qobuz', '2719203',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    7689, 2, '/home/alan/Music/Syncify/Elvis Presley/Elvis 30 #1 Hits/19 - (Marie''s The Name) His Latest Flame.flac', 'FLAC', 15962885,
    '9a845707c1b37786a9012db163990da56111bc9f59802acf174dbb049a332d83', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '4171ca6f-81b9-49bd-a9ab-c697dd49d296', CURRENT_TIMESTAMP,
    'qobuz', '2719215', 'qobuz', '2719215',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    8287, 2, '/home/alan/Music/Syncify/Elvis Presley/Elvis 30 #1 Hits/12 - (Now and Then There''s) A Fool Such as I.flac', 'FLAC', 10032015,
    '161487b41eebd773bc8b071ec02aa1200a6e2d22933a843b5422c2f068a497d3', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '61f690d0-0ea9-414e-93ce-1f3d453e829a', CURRENT_TIMESTAMP,
    'qobuz', '2719208', 'qobuz', '2719208',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    4869, 2, '/home/alan/Music/Syncify/Elvis Presley/Elvis 30 #1 Hits/24 - (You''re The) Devil in Disguise.flac', 'FLAC', 17196738,
    '017bfe01150c59857e83b9d575bc6de870365a440cf5abd1ccfb3a188259f3e0', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '09bdfb37-56ce-46b5-977e-05b6406b9d5d', CURRENT_TIMESTAMP,
    'qobuz', '146861423', 'qobuz', '146861423',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    5468, 2, '/home/alan/Music/Syncify/Radiohead/The Bends/06 - (Nice Dream).flac', 'FLAC', 28135668,
    '4cb9795879f24d72ee8e173a4cb5891421ada25a724e8db4880c000759456790', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '3e8e8360-aaa6-464e-831f-2cb0e1fdcf87', CURRENT_TIMESTAMP,
    'qobuz', '33933965', 'qobuz', '33933965',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    6297, 2, '/home/alan/Music/Syncify/Louis Prima/The Wildest!/02 - (Nothing''s Too Good) For My Baby.flac', 'FLAC', 14402355,
    '9e77868a8b6be26e95e9055f7b8ac332815abd1b95ae082b00732fef98e41da7', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '9f3a759d-514d-4866-a0ce-25a1b1d3a123', CURRENT_TIMESTAMP,
    'qobuz', '1008340', 'qobuz', '1008340',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    990, 2, '/home/alan/Music/Syncify/Cypress Hill/Skull & Bones/Disc 2/06 - (Rock) Superstar.flac', 'FLAC', 37824371,
    '7f6294e97f444663c434fccafe8e789f821d97851c6fd4d1d862055cec4e645c', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '65f933e3-5194-3b3c-87d1-5fffd8458f17', CURRENT_TIMESTAMP,
    'qobuz', '52619', 'qobuz', '52619',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    1074, 2, '/home/alan/Music/Syncify/Otis Redding/The Dock of the Bay/01 - (Sittin'' On) the Dock of the Bay.flac', 'FLAC', 32930619,
    '60ba35842ecca9558a2597cfe13dc925cbababfdcf30818f9a7fab5220e632ca', 24, 96000, 100, CURRENT_TIMESTAMP,
    NULL, 0, '482e68c4-d5d2-4ea1-bc64-a1c51a080b03', CURRENT_TIMESTAMP,
    'qobuz', '21421004', 'qobuz', '21421004',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    15536, 2, '/home/alan/Music/Syncify/Otis Redding/Dock of the Bay/01 - (Sittin'' On) The Dock of the Bay.flac', 'FLAC', 12911501,
    '24e1be708348989932f3d9aa8c008259821e39bfe775e7f381d3e6f9890188db', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'e32ac3ab-4360-4879-b31b-a5055532bc6c', CURRENT_TIMESTAMP,
    'qobuz', '173752794', 'qobuz', '173752794',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    7169, 2, '/home/alan/Music/Syncify/Otis Redding/Definitive Soul_ Otis Redding/23 - (Sittin'' On) the Dock of the Bay.flac', 'FLAC', 11375473,
    'ca448502c48240e63ba70a39b86e88bf29c2e441ca73eb4c1a9b09bb143d7bda', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'a72275ed-2719-44d9-91ca-f8d4e0fca1b3', CURRENT_TIMESTAMP,
    'qobuz', '5875458', 'qobuz', '5875458',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    6717, 2, '/home/alan/Music/Syncify/Sandie Shaw/Sandie/15 - (There''s) Always Something There To Remind Me.flac', 'FLAC', 17456146,
    'bdd7abf6d7db9fb4ff9c84e531474f05abf79aa6a0c9ed9ce1f7acb80a247940', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '6f944855-0423-4a58-8e7d-ff0e42ec8607', CURRENT_TIMESTAMP,
    'qobuz', '87210643', 'qobuz', '87210643',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    9832, 2, '/home/alan/Music/Syncify/The Carpenters/Close To You/06 - (They Long To Be) Close To You.flac', 'FLAC', 27388920,
    '122c048b0bda7dc37fd99caae2559524e2a1cd78db3aa57de668db61b6d9ed2f', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '81292343-61f0-4d08-9b65-477f170f05a0', CURRENT_TIMESTAMP,
    'qobuz', '52400743', 'qobuz', '52400743',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    22520, 2, '/home/alan/Music/Syncify/The Like/Are You Thinking What I''m Thinking_/04 - (So I''ll Sit Here) Waiting.flac', 'FLAC', 29761768,
    'd9b1c976664d07bec6630c31c6bf292aa03558b777bfd452532192c38483afd1', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'c639e41f-00df-4810-99b2-ccb46941086f', CURRENT_TIMESTAMP,
    'qobuz', '54907350', 'qobuz', '54907350',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    17580, 2, '/home/alan/Music/Syncify/The Lovin'' Spoonful/Revelation_ Revolution ''69/06 - (Till I) Run With You.flac', 'FLAC', 12996815,
    '39e4b53817eb044bad0f63f8fe501743c3045b8eb2512f91cbf8129d7637a228', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '3de6ee25-be03-4aa9-b19b-80b1ccd7527d', CURRENT_TIMESTAMP,
    'qobuz', '495528', 'qobuz', '495528',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    7234, 2, '/home/alan/Music/Syncify/Buddy Holly/Giant/07 - (Ummmm, Oh Yeah) Dearest.flac', 'FLAC', 12016570,
    '3f202e629d37328e0f20959d641e2f5c01c9537657e4baf91b6297fb762adde4', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '2b5a0980-e1d3-4c95-ae4b-aa2a84a69599', CURRENT_TIMESTAMP,
    'qobuz', '57390501', 'qobuz', '57390501',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    7612, 2, '/home/alan/Music/Syncify/Sam Cooke/The Man Who Invented Soul/Disc 1/21 - (What A) Wonderful World.flac', 'FLAC', 7916412,
    '9b8ab740d8d4d4aab0ca7788d55bb8c3658c848470f0682e3da35a0b9e6a9310', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '54dca4e9-62ad-424e-af19-992674dcd43d', CURRENT_TIMESTAMP,
    'qobuz', '176715', 'qobuz', '176715',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    7613, 2, '/home/alan/Music/Syncify/Sam Cooke/The Wonderful World Of Sam Cooke/01 - (What A) Wonderful World.flac', 'FLAC', 35652783,
    '3c3b22d8f9ae56a58d4ab496e1e8335cef2f3d1a8eedcf4d2271a87773dca321', 24, 96000, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'a95b14e2-4d98-4bdb-b323-04f627670aad', CURRENT_TIMESTAMP,
    'qobuz', '85432682', 'qobuz', '85432682',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    22192, 2, '/home/alan/Music/Syncify/The Blues Magoos/Psychedelic Lollipop/01 - (We Ain''t Got) Nothin'' Yet.flac', 'FLAC', 13631319,
    'af3cf2213c262d54f45e67c393e30b9edd293ddb06f994f7766f9309ba8f75ce', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, NULL, CURRENT_TIMESTAMP,
    'qobuz', '54424600', 'qobuz', '54424600',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    8799, 2, '/home/alan/Music/Syncify/Nancy Wilson/How Glad I Am/01 - (You Don''t Know) How Glad I Am.flac', 'FLAC', 18161707,
    'eb2356c59f6ccb1a72eabf08ab1b326554f438e81a6ddfe8037221cc8f73843e', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '1b184609-e7be-4b5e-8571-048426d391c0', CURRENT_TIMESTAMP,
    'qobuz', '44209305', 'qobuz', '44209305',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    14661, 2, '/home/alan/Music/Syncify/The Roues Brothers/(You Let the Blues Move In) Now I''m Movin'' Out/01 - (You Let the Blues Move In) Now I''m Movin'' Out.flac', 'FLAC', 17118315,
    'd9e7d050d54a816fa831c2cc41049de198ce9e7f904e41b7f1698118bded2737', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '37cd3009-3fee-4ab1-b9dd-4c4915bee4a0', CURRENT_TIMESTAMP,
    'qobuz', '74555441', 'qobuz', '74555441',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    2870, 2, '/home/alan/Music/Syncify/Britney Spears/Baby One More Time (Deluxe Version)/02 - (You Drive Me) Crazy.flac', 'FLAC', 25675779,
    '2dce015b33196be5c68cfaafb9b441f99785dff8f8c27e186354e480bb8a1e79', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'e904dbad-f5cd-4f73-a261-392087b6053b', CURRENT_TIMESTAMP,
    'qobuz', '421366', 'qobuz', '421366',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    914, 2, '/home/alan/Music/Syncify/Britney Spears/Baby One More Time (Deluxe Version)/01 - Baby One More Time.flac', 'FLAC', 26731343,
    '03f047631a93c55826d98b1d47de1d445bbe3e52a28902d64c92718a4b6ee2f3', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'e904dbad-f5cd-4f73-a261-392087b6053b', CURRENT_TIMESTAMP,
    'qobuz', '421365', 'qobuz', '421365',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    4670, 2, '/home/alan/Music/Syncify/Beastie Boys/Licensed To Ill/07 - (You Gotta) Fight For Your Right (To Party!).flac', 'FLAC', 22197520,
    '05b8957cd4e87851741903d0cd247e3900b12422b7c3701f40b4962f3706ad50', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '923a72ed-ae69-4ef8-b6f6-28c2e9775cb4', CURRENT_TIMESTAMP,
    'qobuz', '764172', 'qobuz', '764172',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    3986, 2, '/home/alan/Music/Syncify/Marilyn Manson/The Golden Age Of Grotesque/07 - (s)AINT (Album Version).flac', 'FLAC', 29469730,
    '4da33f5fa57ca6ebf9657cadae9ea0e38e68c603b505c7f8daba53fafebdaa21', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '745325d7-9121-41d1-ba8e-8023ce6bc011', CURRENT_TIMESTAMP,
    'qobuz', '63448570', 'qobuz', '63448570',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    19701, 2, '/home/alan/Music/Syncify/Aretha Franklin/Lady Soul/05 - (You Make Me Feel Like) A Natural Woman.flac', 'FLAC', 64011153,
    'b8358f50ce9a1f52c4e5d68ee1bcc809e5486dc76e058d7ad9b44e344d6b73b6', 24, 96000, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'b454a0f0-4948-4da1-a2af-53a11817b9b2', CURRENT_TIMESTAMP,
    'qobuz', '6006111', 'qobuz', '6006111',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    6248, 2, '/home/alan/Music/Syncify/Com Truise/Iteration/01 - Of Your Fake Dimension.flac', 'FLAC', 20638236,
    '571911cc48e5c9341102c778b5061b403e5d5c0fcf136ed9ff5cedb54bdbabc0', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '34922e4e-c103-4a38-8b20-ec354aaccd34', CURRENT_TIMESTAMP,
    'qobuz', '41790930', 'qobuz', '41790930',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    5983, 2, '/home/alan/Music/Syncify/Queens Of The Stone Age/Like Clockwork/10 - Like Clockwork.flac', 'FLAC', 31250137,
    'b26554635068d3b811c22c660dafe438effa4d492aff7050c9183eaf8705d52f', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '303e07e3-cb74-49fb-838a-6b4209d8b55f', CURRENT_TIMESTAMP,
    'qobuz', '9390853', 'qobuz', '9390853',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    8820, 2, '/home/alan/Music/Syncify/Taylor Swift/reputation/01 - Ready For It_.flac', 'FLAC', 43663757,
    'cf40807f80d2e9649496ddeec5bfb48c2ec2a13c4756338f6cc7c575ff38bb87', 24, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'c220f474-58e1-40ae-827c-24f899cf3e05', CURRENT_TIMESTAMP,
    'qobuz', '78056526', 'qobuz', '78056526',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    11634, 2, '/home/alan/Music/Syncify/Bunbury/Flamingos/15 - Y al final.flac', 'FLAC', 26490454,
    '88fd3cd18310edb218849d1e008c18476eb346f4302d59d0145b8939070b0080', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, NULL, CURRENT_TIMESTAMP,
    'qobuz', '4745376', 'qobuz', '4745376',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    12867, 2, '/home/alan/Music/Syncify/Alexisonfire/Alexisonfire/01 - 44 Caliber Love Letter.flac', 'FLAC', 60046948,
    '7da2566a645f7eeb86dcc5e0ad1f346b8abe67b16ff2663ceda35f9847d5206a', 24, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '35630d36-b176-3e43-a58c-843f50cab1bb', CURRENT_TIMESTAMP,
    'qobuz', '80047860', 'qobuz', '80047860',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    957, 2, '/home/alan/Music/Syncify/Dead Poet Society/-!-/11 - CoDA.flac', 'FLAC', 83809106,
    'a82fbc19b11072df18a8bb6ce23136a9e1b9e5fa95efdca3a08954e02e039644', 24, 96000, 100, CURRENT_TIMESTAMP,
    NULL, 0, '19c6e9d5-b1d4-4a88-b2f8-d2729f4dd5d6', CURRENT_TIMESTAMP,
    'qobuz', '193533073', 'qobuz', '193533073',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    22679, 2, '/home/alan/Music/Syncify/Desmond Dekker & The Aces/007 Shanty Town/01 - 007 (Shanty Town).flac', 'FLAC', 13255135,
    '2d878ce662a454fb3a32e56d5c4f4e347f902b9ff3b0975c9155e8defcd51949', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'e9f5ccc4-90af-4176-a277-373ba64f754d', CURRENT_TIMESTAMP,
    'qobuz', '385656727', 'qobuz', '385656727',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    11005, 2, '/home/alan/Music/Syncify/SHINee/1 of 1 - The 5th Album/02 - 1 of 1.flac', 'FLAC', 27782644,
    '48637674f57f062eb925b5b974b625895cdfcc4d8f54a060dd3fbcf64b1036be', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '79712f64-8dc2-4c7f-a3c5-d1b509a23ef6', CURRENT_TIMESTAMP,
    'qobuz', '66661822', 'qobuz', '66661822',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    7, 3, '/home/alan/Music/Syncify/Morat/2018 - Balas Perdidas/12 - 11 Besos.m4a', 'AAC', 4995792,
    '8e2963041a42e8782f490ddfd66f8c48a5b326f41141280b6fb404ebdc01ce42', NULL, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, NULL, CURRENT_TIMESTAMP,
    'tidal', NULL, 'tidal', NULL,
    NULL, 'isrc', 1.0, NULL,
    'high', 'high', 'AAC', 'AAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    14863, 2, '/home/alan/Music/Syncify/Yard Act/The Overload/11 - 100% Endurance.flac', 'FLAC', 43764112,
    '4b4ed754fafd6763bd1e16d88e305fb80203da6298a9d8d169a36bf392ecb658', 24, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '1ea21bb7-a576-4db6-832b-d253f7589f88', CURRENT_TIMESTAMP,
    'qobuz', '127997325', 'qobuz', '127997325',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    11432, 2, '/home/alan/Music/Syncify/Rodrigo y Gabriela/Area 52/04 - 11_11.flac', 'FLAC', 51081032,
    '9a699ac821fe01c404caf340bdf089bce48bc6f8b37177d60efa82c18fbdf906', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '70f0f6b5-c354-4409-a013-85c2aea06cf5', CURRENT_TIMESTAMP,
    'qobuz', '45114030', 'qobuz', '45114030',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    11014, 2, '/home/alan/Music/Syncify/Jackson Wang/100 Ways/01 - 100 Ways.flac', 'FLAC', 62899649,
    '327057c073710a63f05a14949d710de991c5c4e9cc0238611478fae2e637a81b', 24, 96000, 100, CURRENT_TIMESTAMP,
    NULL, 0, '2928e33e-e600-4f1f-8477-bbbd5a694ce9', CURRENT_TIMESTAMP,
    'qobuz', '246970903', 'qobuz', '246970903',
    NULL, 'isrc', 1.0, NULL,
    'hires', 'hires', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    18162, 2, '/home/alan/Music/Syncify/Julian Casablancas/Phrazes For The Young/03 - 11th Dimension.flac', 'FLAC', 32569291,
    '6ad08c098c7e85dacb53933d807fc5267bd860b99c70d5e25b8e91f36d6c68fc', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, '0259e9a8-3313-4f3d-abe1-9254728a26ea', CURRENT_TIMESTAMP,
    'qobuz', '94463', 'qobuz', '94463',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    12191, 2, '/home/alan/Music/Syncify/Catch 22/Keasbey Nights/14 - 1234 1234.flac', 'FLAC', 50812663,
    '0c9701430777de2b598775a68419097c5a007eab8e7c5161b505d49d775a8e45', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, 'd44bf25f-08c7-478c-856d-a1619658d6b0', CURRENT_TIMESTAMP,
    'qobuz', '118265531', 'qobuz', '118265531',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    12293, 2, '/home/alan/Music/Syncify/Dinosaur Pile-Up/11_11/02 - 11_11.flac', 'FLAC', 24527621,
    '9930ce62386195e9bd769e660d607df19eeae6735db287179a95c9a13f7ba56e', 16, 44100, 100, CURRENT_TIMESTAMP,
    NULL, 0, NULL, CURRENT_TIMESTAMP,
    'qobuz', '32442272', 'qobuz', '32442272',
    NULL, 'isrc', 1.0, NULL,
    'lossless', 'lossless', 'FLAC', 'FLAC',
    'direct_match', 0, 0, 'reconciliation_backfill', NULL
)
ON CONFLICT(track_id) DO UPDATE SET
    source_service_id = excluded.source_service_id,
    file_path = excluded.file_path,
    file_format = excluded.file_format,
    file_size_bytes = excluded.file_size_bytes,
    file_hash = excluded.file_hash,
    bit_depth = excluded.bit_depth,
    sample_rate = excluded.sample_rate,
    metadata_completeness = excluded.metadata_completeness,
    updated_at = CURRENT_TIMESTAMP,
    origin_service = excluded.origin_service,
    origin_service_track_id = excluded.origin_service_track_id,
    effective_service = excluded.effective_service,
    effective_service_track_id = excluded.effective_service_track_id,
    match_method = excluded.match_method,
    match_confidence = excluded.match_confidence,
    requested_quality = excluded.requested_quality,
    effective_quality = excluded.effective_quality,
    requested_format = excluded.requested_format,
    effective_format = excluded.effective_format,
    quality_decision = excluded.quality_decision,
    decision_reason = excluded.decision_reason;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (12463, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>I<00:00.09> <00:00.60>ain''t<00:00.73> <00:00.87>got<00:01.20> <00:01.32>no<00:01.44> <00:04.08>reservation
[00:07.65]<00:07.65>I''m<00:07.77> <00:07.89>just<00:08.13> <00:08.67>sitting<00:09.26> <00:09.84>here<00:10.47> <00:11.01>listening<00:11.27> <00:11.52>to<00:11.58> <00:11.64>you
[00:14.94]<00:14.94>I<00:15.22> <00:15.51>ain''t<00:15.55> <00:15.60>got<00:15.96> <00:16.11>no<00:17.28> <00:18.72>reservation
[00:22.26]<00:22.26>I''m<00:22.39> <00:22.53>just<00:22.65> <00:22.95>sitting<00:23.77> <00:24.60>here<00:25.17> <00:25.71>listening<00:25.92> <00:26.13>to<00:26.25> <00:26.37>you
[00:38.40]<00:38.40>Talk<00:38.59> <00:38.79>is<00:38.97> <00:39.15>cheap<00:39.57> <00:42.15>if<00:42.28> <00:42.42>you<00:42.60> <00:42.78>can''t<00:43.35> <00:47.46>walk<00:47.69> <00:47.91>the<00:48.12> <00:48.33>walk
[00:52.95]<00:52.95>I<00:53.04> <00:53.13>listened<00:53.37> <00:53.61>to<00:53.72> <00:53.82>you<00:54.27> <00:56.61>I<00:56.69> <00:56.76>heard<00:56.91> <00:57.06>what<00:57.18> <00:57.30>you<00:57.36> <00:57.42>said
[01:00.48]<01:00.48>Now<01:00.61> <01:00.75>you<01:00.82> <01:00.90>got<01:01.08> <01:01.26>to<01:01.30> <01:01.35>do<01:01.50> <01:01.65>it<01:01.86> <01:02.79>today
[01:06.21]<01:06.21>I<01:06.36> <01:06.51>ain''t<01:06.69> <01:06.87>got<01:07.23> <01:09.96>no<01:10.05> <01:10.14>reservation
[01:13.62]<01:13.62>I''m<01:13.76> <01:13.89>just<01:14.10> <01:14.31>sitting<01:15.12> <01:15.93>here<01:16.65> <01:17.10>listening<01:17.32> <01:17.55>to<01:17.66> <01:17.76>you
[01:21.00]<01:21.00>I<01:21.27> <01:21.54>ain''t<01:21.59> <01:21.63>got<01:21.86> <01:22.08>no<01:23.16> <01:24.87>reservation
[01:28.38]<01:28.38>I''m<01:28.65> <01:28.89>just<01:29.04> <01:29.43>sitting<01:30.04> <01:30.66>here<01:31.20> <01:31.83>listening<01:32.04> <01:32.25>to<01:32.35> <01:32.46>you
[01:44.37]<01:44.37>I<01:44.45> <01:44.52>listened<01:44.74> <01:44.97>to<01:45.09> <01:45.21>you,<01:45.75> <01:48.09>I<01:48.21> <01:48.27>heard<01:48.39> <01:48.51>what<01:48.63> <01:48.75>you<01:48.81> <01:48.87>said
[01:51.93]<01:51.93>It<01:52.05> <01:52.17>don''t<01:52.29> <01:52.41>mean<01:52.62> <01:52.83>nothing<01:53.28> <01:53.40>if<01:53.51> <01:53.61>it''s<01:53.69> <01:53.76>just<01:53.94> <01:54.12>in<01:54.21> <01:54.30>your<01:54.45> <01:54.54>head
[01:57.78]<01:57.78>I<01:57.90> <01:58.02>ain''t<01:58.18> <01:58.35>go<01:58.50> <01:58.83>no<02:00.15> <02:01.65>reservation
[02:05.13]<02:05.13>I''m<02:05.26> <02:05.40>just<02:05.85> <02:06.24>sitting<02:06.82> <02:07.41>here<02:08.01> <02:08.52>listening<02:08.74> <02:08.97>to<02:09.07> <02:09.18>you
[02:12.48]<02:12.48>I<02:12.72> <02:12.96>ain''t<02:13.05> <02:13.14>go<02:13.29> <02:13.44>no<02:13.50> <02:16.35>reservation
[02:19.89]<02:19.89>I''m<02:20.01> <02:20.13>just<02:20.25> <02:20.37>sitting<02:20.88> <02:21.03>here<02:22.68> <02:23.28>listening<02:23.52> <02:23.76>to<02:23.82> <02:23.88>you
[02:28.20]<02:28.20>I<02:28.27> <02:28.35>listened<02:28.57> <02:28.80>to<02:28.95> <02:29.10>the<02:29.31> <02:29.52>words<02:29.79> <02:30.06>you<02:30.21> <02:30.36>said<02:30.70> <02:31.05>and<02:31.17> <02:31.29>I<02:31.36> <02:31.44>know<02:31.89> <02:32.34>just<02:32.49> <02:32.64>how<02:32.76> <02:32.88>you<02:33.08> <02:33.27>feel
[02:35.94]<02:35.94>You<02:36.05> <02:36.15>can''t<02:36.27> <02:36.39>preach<02:36.57> <02:36.75>to<02:36.81> <02:36.87>the<02:36.96> <02:37.05>converted<02:37.78> <02:38.52>I<02:38.88> <02:39.30>know<02:40.00> <02:40.71>the<02:40.75> <02:40.80>deal
[02:43.38]<02:43.38>Now<02:43.56> <02:43.74>it''s<02:43.81> <02:43.89>coming<02:44.22> <02:44.55>to<02:44.85> <02:45.15>the<02:45.31> <02:45.48>time<02:45.78> <02:46.08>when<02:46.20> <02:46.32>you<02:46.39> <02:46.47>gotta<02:47.40> <02:47.58>walk<02:47.76> <02:47.94>the<02:48.12> <02:48.30>walk
[02:50.85]<02:50.85>You<02:50.91> <02:50.97>gotta<02:51.18> <02:51.39>make<02:51.57> <02:51.75>it<02:51.95> <02:52.14>happen<02:52.59> <02:53.04>man<02:53.31> <02:53.58>I<02:53.67> <02:53.76>can''t<02:53.98> <02:54.21>listen<02:54.90> <03:05.88>to<03:06.90> <03:07.11>you<03:07.77> <03:08.16>talk
[03:12.18]<03:12.18>I<03:12.42> <03:12.66>ain''t<03:12.75> <03:12.84>go<03:12.96> <03:13.32>no<03:14.31> <03:16.05>reservation
[03:19.77]<03:19.77>I''m<03:19.84> <03:19.92>just<03:19.99> <03:20.07>sitting<03:20.92> <03:21.78>here<03:22.53> <03:22.98>listening<03:23.19> <03:23.40>to<03:23.52> <03:23.64>you
[03:26.91]<03:26.91>I<03:27.03> <03:30.54>ain''t<03:30.87> <03:31.20>go<03:31.29> <03:31.62>no<03:31.68> <03:31.74>reservation
[03:34.20]<03:34.20>I''m<03:34.32> <03:34.44>just<03:34.89> <03:35.28>sitting<03:37.02> <03:53.10>here<03:55.32> <03:56.28>listening<03:56.71> <03:57.15>to<03:57.85> <03:58.56>you
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (2006, 'lrc', 'line', 'local_lrc', '[00:28.95]<00:28.95>You <00:32.33>take <00:32.83>my <00:33.33>love
[00:37.90]<00:37.90>You <00:41.40>want <00:41.84>my <00:42.40>soul
[00:47.28]<00:47.28>I <00:47.71>would <00:48.03>be <00:48.34>crazy <00:49.40>to <00:49.84>share <00:50.09>your <00:50.59>life
[00:51.78]<00:51.78>Why <00:52.21>can''t <00:52.71>you <00:53.09>see <00:53.53>what <00:53.90>I <00:54.28>am
[00:56.34]<00:56.34>Sharpen <00:56.78>the <00:57.21>senses <00:58.03>and <00:58.84>turn <00:59.34>the <00:59.78>knife
[01:00.90]<01:00.90>Hurt <01:01.46>me <01:01.90>and <01:02.46>you''ll <01:03.03>understand
[01:04.96]<01:04.96>I''ll <01:05.21>never <01:05.53>be <01:07.09>Maria <01:08.23>Magdalena
[01:09.17]<01:09.17>(You''re <01:09.48>a <01:09.73>creature <01:09.92>of <01:10.11>the <01:10.29>night)
[01:11.67]<01:11.67>Maria <01:11.92>Magdalena
[01:13.29]<01:13.29>(you''re <01:13.60>a <01:13.79>victim <01:14.17>of <01:14.48>the <01:14.73>fight)
[01:16.48]<01:16.48>(you <01:16.73>need <01:16.98>love)
[01:18.23]<01:18.23>Promised <01:18.67>me <01:18.92>delight
[01:20.98]<01:20.98>(You <01:21.23>need <01:21.48>love)
[01:23.35]<01:23.35>I''ll <01:23.54>never <01:23.79>be <01:25.45>Maria <01:25.70>Magdalena
[01:27.32]<01:27.32>(You''re <01:27.57>a <01:27.76>creature <01:28.01>of <01:28.26>the <01:28.45>night)
[01:30.14]<01:30.14>Maria <01:30.32>Magdalena
[01:31.70]<01:31.70>(you''re <01:31.95>a <01:32.14>victim <01:32.39>of <01:32.70>the <01:32.95>fight)
[01:34.76]<01:34.76>(you <01:35.14>need <01:35.39>love)
[01:36.64]<01:36.64>Promised <01:36.89>me <01:37.33>delight
[01:39.39]<01:39.39>(You <01:39.64>need <01:40.01>love)
[01:45.80]<01:45.80>Album:Reflections
[01:46.23]<01:46.23>Sandra-Maria <01:47.05>Magdalena
[01:51.86]<01:51.86>Why <01:55.11>must <01:55.61>I <01:56.17>lie
[02:00.86]<02:00.86>Find <02:04.36>any <02:05.42>prize
[02:10.23]<02:10.23>When <02:10.61>will <02:10.92>you <02:11.23>wake <02:11.67>up <02:12.23>and <02:12.48>realize
[02:14.80]<02:14.80>I <02:15.17>can''t <02:15.67>surrender <02:16.30>to <02:16.92>you
[02:19.23]<02:19.23>Play <02:19.80>for <02:20.23>affection <02:20.80>and
[02:21.74]<02:21.74>Win <02:22.30>the <02:22.73>prize
[02:24.05]<02:24.05>I <02:24.42>know <02:24.92>those <02:25.42>party <02:25.92>games <02:26.30>too
[02:27.98]<02:27.98>I''ll <02:28.23>never <02:28.42>be <02:29.98>Maria <02:30.23>Magdalena
[02:31.42]<02:31.42>(You''re <02:31.73>a <02:31.92>creature <02:32.24>of <02:32.55>the <02:32.86>night)
[02:34.67]<02:34.67>Maria <02:34.98>Magdalena
[02:36.05]<02:36.05>(you''re <02:36.30>a <02:36.48>victim <02:36.80>of <02:37.17>the <02:37.49>fight)
[02:39.48]<02:39.48>(you <02:39.73>need <02:39.92>love)
[02:41.23]<02:41.23>Promised <02:41.48>me <02:41.80>delight
[02:43.80]<02:43.80>(You <02:44.17>need <02:44.42>love)
[02:46.30]<02:46.30>I''ll <02:46.48>never <02:46.73>be <02:48.53>Maria <02:48.85>Magdalena
[02:50.53]<02:50.53>(You''re <02:50.72>a <02:50.91>creature <02:51.10>of <02:51.35>the <02:51.72>night)
[02:53.16]<02:53.16>Maria <02:53.41>Magdalena
[02:54.47]<02:54.47>(you''re <02:54.79>a <02:55.04>victim <02:55.35>of <02:55.66>the <02:55.91>fight)
[02:57.66]<02:57.66>(you <02:58.10>need <02:58.41>love)
[02:59.72]<02:59.72>Promised <03:00.04>me <03:00.29>delight
[03:02.29]<03:02.29>(You <03:02.62>need <03:02.86>love)
[03:06.88]<03:06.88>Album:Reflections
[03:07.32]<03:07.32>Sandra-Maria <03:08.38>Magdalena
[03:23.32]<03:23.32>I''ll <03:23.57>never <03:23.75>be <03:25.44>Maria <03:25.76>Magdalena
[03:27.13]<03:27.13>(You''re <03:27.32>a <03:27.50>creature <03:27.76>of <03:28.07>the <03:28.38>night)
[03:29.94]<03:29.94>Maria <03:30.26>Magdalena
[03:31.25]<03:31.25>(you''re <03:31.63>a <03:31.88>victim <03:32.19>of <03:32.51>the <03:32.82>fight)
[03:34.57]<03:34.57>(you <03:34.88>need <03:35.13>love)
[03:36.44]<03:36.44>Promised <03:36.69>me <03:37.13>delight
[03:39.13]<03:39.13>(You <03:39.44>need <03:39.69>love)
[03:41.63]<03:41.63>I''ll <03:41.88>never <03:42.07>be <03:43.79>Maria <03:44.16>Magdalena
[03:45.48]<03:45.48>(You''re <03:45.73>a <03:45.98>creature <03:46.29>of <03:46.54>the <03:46.79>night)
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (946, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.24>I''ve<00:00.49> <00:00.73>Had<00:00.98>)<00:01.22> <00:01.46>The<00:01.71> <00:01.95>Time<00:02.20> <00:02.44>of<00:02.68> <00:02.93>My<00:03.17> <00:03.42>Life<00:03.66> <00:03.90>-<00:04.15> <00:04.39>Bill<00:04.64> <00:04.88>Medley<00:05.12>/<00:05.37>Jennifer<00:05.61> <00:05.86>Warnes
[00:06.11]<00:06.11>Now <00:06.53>I''ve <00:07.57>had <00:07.77>the <00:08.25>time <00:08.87>of <00:09.28>my <00:09.64>life
[00:12.64]<00:12.64>No <00:12.82>I <00:13.13>never <00:13.83>felt <00:14.12>like <00:14.78>this <00:15.27>before
[00:16.79]<00:16.79>Yes <00:17.01>I <00:17.21>swear <00:18.43>it''s <00:18.70>the <00:18.98>truth
[00:21.33]<00:21.33>And <00:21.56>I <00:21.75>owe <00:22.10>it <00:22.34>all <00:22.63>to <00:22.99>you
[00:23.75]<00:23.75>''Cause <00:24.13>I''ve <00:25.45>had <00:25.91>the <00:26.32>time <00:26.64>of <00:27.11>my <00:27.52>life
[00:30.03]<00:30.03>And <00:30.32>I <00:30.76>owe <00:31.00>it <00:31.21>all <00:31.68>to <00:31.87>you
[00:41.40]<00:41.40>I''ve <00:41.67>been <00:42.00>waiting <00:42.45>for <00:42.73>so <00:42.94>long
[00:43.38]<00:43.38>Now <00:43.68>I''ve <00:43.95>finally <00:44.26>found
[00:44.69]<00:44.69>Someone <00:45.07>to <00:45.44>stand <00:45.95>by <00:46.18>me
[00:50.11]<00:50.11>We <00:50.27>saw <00:50.44>the <00:50.70>writing <00:51.01>on <00:51.35>the <00:51.63>wall
[00:52.47]<00:52.47>As <00:52.65>we <00:52.81>felt <00:53.13>this <00:53.44>magical <00:54.12>fantasy
[00:59.12]<00:59.12>Now <00:59.35>with <00:59.59>passion <00:59.85>in <01:00.05>our <01:00.21>eyes
[01:01.19]<01:01.19>There''s <01:01.40>no <01:01.55>way
[01:01.88]<01:01.88>We <01:02.08>could <01:02.24>disguise <01:02.47>it <01:02.93>secretly
[01:07.84]<01:07.84>So <01:08.01>we <01:08.16>take <01:08.32>each <01:08.64>other''s <01:08.97>hand
[01:09.81]<01:09.81>''Cause <01:10.07>we <01:10.27>seem <01:10.56>to
[01:10.85]<01:10.85>Understand <01:11.89>the <01:12.19>urgency
[01:16.00]<01:16.00>Just <01:16.26>remember
[01:18.47]<01:18.47>You''re <01:18.82>the <01:19.10>one <01:19.81>thing
[01:21.73]<01:21.73>I <01:21.92>can''t <01:22.11>get <01:22.48>enough <01:23.52>of
[01:26.58]<01:26.58>So <01:26.77>I''ll <01:26.98>tell <01:27.24>you <01:28.00>something
[01:31.62]<01:31.62>This <01:31.81>could <01:32.01>be <01:32.21>love <01:33.25>because
[01:35.84]<01:35.84>I''ve <01:36.81>had <01:37.65>the <01:37.84>time <01:38.02>of <01:38.21>my <01:38.40>life
[01:41.10]<01:41.10>No <01:41.30>I <01:41.51>never <01:42.09>felt <01:42.55>this <01:42.86>way <01:43.26>before
[01:45.30]<01:45.30>Yes <01:45.55>I <01:45.80>swear <01:46.92>it''s <01:47.13>the <01:47.32>truth
[01:49.38]<01:49.38>And <01:49.77>I <01:50.01>owe <01:50.59>it <01:50.93>all <01:51.25>to <01:51.48>you
[01:56.36]<01:56.36>Hey <01:56.67>baby
[02:00.99]<02:00.99>With <02:01.17>my <02:01.34>body <02:02.17>and <02:02.40>soul
[02:02.61]<02:02.61>I <02:02.87>want <02:03.17>you <02:03.44>more <02:03.74>than
[02:04.26]<02:04.26>You''ll <02:04.64>ever <02:05.67>know
[02:09.85]<02:09.85>So <02:10.07>we''ll <02:10.31>just <02:10.59>let <02:10.81>it <02:11.14>go
[02:11.50]<02:11.50>Don''t <02:12.02>be <02:12.34>afraid <02:13.14>to <02:13.38>lose <02:13.75>control
[02:18.70]<02:18.70>Yes <02:18.87>I <02:19.04>know <02:19.31>what''s <02:19.48>on <02:19.70>your <02:19.90>mind
[02:20.61]<02:20.61>When <02:20.88>you <02:21.13>say
[02:22.32]<02:22.32>Stay <02:22.58>with <02:22.98>me <02:23.32>tonight
[02:27.04]<02:27.04>Just <02:27.25>remember
[02:29.11]<02:29.11>You''re <02:29.88>the <02:30.10>one <02:30.86>thing
[02:32.20]<02:32.20>I <02:32.85>can''t <02:33.21>get <02:33.58>enough <02:34.57>of
[02:37.32]<02:37.32>So <02:37.55>I''ll <02:37.77>tell <02:38.26>you <02:38.75>something
[02:42.44]<02:42.44>This <02:42.63>could <02:42.84>be <02:43.05>love <02:43.86>because
[02:46.56]<02:46.56>I''ve <02:47.54>had <02:48.29>the <02:48.71>time <02:49.17>of <02:49.52>my <02:49.94>life
[02:52.18]<02:52.18>No <02:52.36>I <02:52.75>never <02:53.54>felt <02:53.76>this <02:54.07>way <02:54.34>before
[02:56.12]<02:56.12>Yes <02:56.32>I <02:56.47>swear <02:57.56>it''s <02:57.76>the <02:57.92>truth
[03:00.55]<03:00.55>And <03:00.75>I <03:01.03>owe <03:01.45>it <03:01.70>all <03:01.90>to <03:02.22>you
[03:02.69]<03:02.69>''Cause <03:03.05>I''ve <03:04.44>had <03:04.87>the <03:05.33>time <03:05.84>of <03:06.14>my <03:06.40>life
[03:08.72]<03:08.72>And <03:08.99>I''ve <03:09.88>searched
[03:10.48]<03:10.48>Through <03:10.92>every <03:11.46>open <03:11.92>door
[03:13.53]<03:13.53>''Til <03:14.01>I <03:14.29>found <03:15.13>the <03:15.53>truth
[03:18.23]<03:18.23>And <03:18.52>I <03:18.76>owe <03:19.00>it <03:19.23>all <03:19.52>to <03:19.76>you
[03:54.13]<03:54.13>Now <03:54.52>I''ve <03:56.12>had <03:57.00>the <03:57.24>time <03:57.65>of <03:57.93>my <03:58.21>life
[04:00.36]<04:00.36>No <04:00.54>I <04:00.77>never <04:01.56>felt <04:02.08>this <04:02.40>way <04:02.77>before
[04:04.64]<04:04.64>Yes <04:04.83>I <04:05.02>swear <04:05.98>it''s <04:06.36>the <04:06.68>truth
[04:09.10]<04:09.10>And <04:09.28>I <04:09.52>owe <04:09.68>it <04:09.96>all <04:10.26>to <04:10.62>you
[04:12.18]<04:12.18>I''ve <04:14.02>had <04:14.91>the <04:15.24>time <04:15.44>of <04:15.65>my <04:15.90>life
[04:17.92]<04:17.92>No <04:18.14>I <04:18.32>never <04:18.99>felt <04:19.34>this <04:19.62>way <04:20.20>before
[04:22.35]<04:22.35>Yes <04:22.61>I <04:22.82>swear <04:23.60>it''s <04:23.80>the <04:24.03>truth
[04:26.72]<04:26.72>And <04:26.99>I <04:27.18>owe <04:27.70>it <04:27.92>all <04:28.31>to <04:28.60>you
[04:29.03]<04:29.03>I''ve <04:30.81>had <04:31.40>the <04:31.69>time <04:31.93>of <04:32.12>my <04:32.31>life
[04:36.21]<04:36.21>No <04:36.60>I <04:36.89>never <04:37.23>felt <04:37.62>this <04:38.01>way <04:38.25>before
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (5853, 'lrc', 'line', 'local_lrc', '[00:23.03]<00:23.03>I
[00:25.66]<00:25.66>I <00:26.33>would <00:26.54>be <00:26.64>king
[00:30.80]<00:30.80>And <00:31.83>you
[00:33.68]<00:33.68>You <00:34.07>would <00:34.26>be <00:34.47>my <00:34.69>queen
[00:39.30]<00:39.30>For <00:39.69>nothing <00:39.92>could <00:40.11>keep <00:40.26>us <00:40.41>together
[00:42.52]<00:42.52>But <00:42.85>we <00:43.12>could <00:43.46>beat <00:44.37>them <00:47.77>forever <00:48.17>and <00:48.41>ever
[00:55.51]<00:55.51>We <00:55.85>could <00:56.01>be <00:56.14>heroes <00:59.43>just <00:59.98>for <01:00.34>one <01:01.05>day
[01:05.12]<01:05.12>You
[01:08.23]<01:08.23>You <01:08.51>can <01:08.75>be <01:08.88>mean
[01:12.06]<01:12.06>And <01:12.43>I
[01:15.95]<01:15.95>I''ll <01:16.38>drink <01:16.71>all <01:17.07>the <01:17.20>time
[01:21.16]<01:21.16>''Cause <01:21.50>we''re <01:21.94>lovers <01:24.85>and <01:25.15>that <01:25.45>is <01:25.69>the <01:25.94>fact
[01:28.41]<01:28.41>Oh <01:29.63>yes <01:30.05>we''re <01:30.56>lovers <01:33.18>and <01:33.52>that <01:33.83>is <01:34.07>that
[01:38.44]<01:38.44>For <01:39.01>nothing <01:41.61>will <01:41.92>keep <01:42.26>us <01:42.51>together
[01:47.54]<01:47.54>Or <01:47.96>we <01:51.55>maybe <01:54.46>then <01:58.70>just <01:59.04>for <01:59.25>one <01:59.73>day
[02:01.34]<02:01.34>Ah <02:02.11>I
[02:20.54]<02:20.54>Well <02:23.25>I <02:23.98>wish <02:24.51>you <02:24.65>could <02:24.91>swim
[02:28.41]<02:28.41>Like <02:28.92>the <02:29.13>dolphins <02:31.56>like <02:32.25>dolphins <02:33.20>can <02:33.93>swim
[02:37.21]<02:37.21>For <02:37.62>nothing <02:40.40>we''ll <02:40.68>driving <02:41.39>away
[02:43.89]<02:43.89>Oh <02:45.14>we <02:45.65>could <02:45.82>beat <02:46.10>them <02:46.60>for <02:49.13>just <02:49.44>for <02:49.66>one <02:50.20>day
[02:58.58]<02:58.58>We <02:58.98>can <03:00.29>be <03:01.02>heroes <03:01.63>forever <03:20.36>and <03:20.64>ever
[03:22.81]<03:22.81>What <03:23.21>you <03:24.27>say
[03:33.29]<03:33.29>I
[03:36.27]<03:36.27>I <03:36.73>will <03:37.01>be <03:37.60>king
[03:39.53]<03:39.53>And <03:39.89>you <03:39.99>you <03:40.16>will <03:40.38>be <03:40.51>my <03:40.66>queen
[03:44.29]<03:44.29>For <03:44.68>nothing <03:45.18>could <03:45.60>keep <03:45.75>us <03:45.89>together
[03:47.91]<03:47.91>We <03:48.11>could <03:48.59>beat <03:48.74>them <03:49.02>forever <03:49.47>and <03:50.09>ever
[03:52.33]<03:52.33>We <03:52.66>could <03:52.86>be <03:53.24>heroes <03:53.88>just <03:57.18>for <03:57.48>one <03:58.47>day
[04:01.63]<04:01.63>Oh <04:04.81>oh <04:04.99>I <04:08.15>can <04:10.38>remember <04:18.00>I <04:19.13>remember
[04:22.01]<04:22.01>Standing <04:22.87>by <04:24.40>the <04:24.84>wall <04:26.39>by <04:26.60>the <04:27.65>wall
[04:29.53]<04:29.53>And <04:29.81>the <04:30.44>guns <04:30.66>shot <04:32.97>above <04:35.13>our <04:35.37>heads <04:35.71>over <04:36.09>our <04:36.91>heads
[04:38.63]<04:38.63>And <04:40.84>we <04:41.03>kissed <04:43.04>as <04:43.55>though <04:43.65>nothing <04:43.97>could <04:44.16>fall <04:44.37>nothing <04:44.60>could <04:44.73>fall
[04:46.70]<04:46.70>And <04:46.93>the <04:47.20>shame <04:47.34>was <04:47.94>on <04:48.21>the <04:48.33>other <04:48.51>side
[04:50.85]<04:50.85>We <04:53.80>can <04:55.42>beat <04:55.68>them <04:55.95>for <04:56.56>ever <04:58.60>and <04:59.43>ever
[05:03.75]<05:03.75>Then <05:04.11>we <05:05.25>could <05:05.38>be <05:05.52>heroes <05:08.02>just <05:08.34>for <05:08.56>one <05:08.80>day
[05:12.01]<05:12.01>What <05:13.56>to <05:14.35>say
[05:17.03]<05:17.03>We <05:17.23>can <05:17.40>be <05:17.50>heroes <05:17.73>just <05:17.91>for <05:18.04>one <05:18.17>day
[05:20.17]<05:20.17>We <05:21.22>can <05:21.50>be <05:21.74>heroes
[05:24.17]<05:24.17>We''re <05:24.37>nothing <05:24.71>and <05:25.03>nothing <05:25.26>will <05:25.52>help <05:27.57>us
[05:28.28]<05:28.28>Maybe <05:28.75>we''re <05:28.89>lying <05:29.14>then <05:29.27>you <05:29.38>better <05:29.70>not <05:31.95>stay
[05:32.69]<05:32.69>Oh <05:32.87>we <05:33.24>can <05:33.53>be <05:34.35>heroes
[05:37.13]<05:37.13>Just <05:37.42>for <05:37.60>one <05:39.25>day
[05:47.37]<05:47.37>Oh <05:47.54>oh <05:47.69>oh <05:48.06>ohh <05:48.13>oh <05:48.46>oh <05:48.53>oh <05:48.78>ohh
[05:52.49]<05:52.49>Just <05:53.78>for <05:58.53>one <06:00.22>day
[06:01.94]<06:01.94>Oh <06:04.48>oh <06:04.65>oh <06:04.75>ohh <06:05.54>oh
[06:06.25]<06:06.25>Just <06:07.42>for <06:07.70>one <06:09.39>day
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (23, 'lrc', 'line', 'local_lrc', '[00:23.46]<00:23.46>I <00:23.65>would <00:23.90>die <00:24.46>for <00:24.71>you
[00:25.97]<00:25.97>I <00:26.28>would <00:26.46>die <00:26.96>for <00:27.21>you
[00:28.21]<00:28.21>I''ve <00:28.53>been <00:28.84>dying <00:30.28>just <00:30.78>to <00:31.46>feel <00:32.09>you <00:32.72>by <00:33.34>my <00:33.84>side
[00:37.96]<00:37.96>To <00:38.15>know <00:38.47>that <00:38.71>you''re <00:39.03>mine
[00:42.84]<00:42.84>I <00:43.65>will <00:44.21>cry <00:44.78>for <00:45.21>you
[00:46.34]<00:46.34>I <00:46.53>will <00:46.77>cry <00:47.28>for <00:47.59>you
[00:48.59]<00:48.59>I <00:48.90>will <00:49.28>wash <00:49.96>away <00:51.09>your <00:51.72>pain <00:52.28>with <00:52.84>all <00:53.46>my <00:54.09>tears
[00:58.15]<00:58.15>And <00:58.46>drown <00:58.84>your <00:59.15>fears
[01:24.57]<01:24.57>I <01:24.70>will <01:24.95>pray <01:25.58>for <01:25.95>you
[01:27.07]<01:27.07>I <01:27.26>will <01:27.51>pray <01:28.08>for <01:28.39>you
[01:29.39]<01:29.39>I <01:29.70>will <01:29.95>sell <01:30.57>my <01:31.20>soul <01:31.82>for <01:32.20>something <01:33.76>pure <01:34.32>and <01:34.76>true
[01:39.01]<01:39.01>Someone <01:39.07>like <01:40.14>you
[01:44.32]<01:44.32>See <01:46.14>your <01:46.51>face <01:47.20>every <01:47.89>place <01:48.45>that <01:48.76>I <01:49.08>walk <01:49.70>in
[01:51.20]<01:51.20>Hear <01:51.58>your <01:51.82>voice <01:52.26>every <01:52.95>time <01:53.57>that <01:53.89>I''m <01:54.20>talking
[01:56.76]<01:56.76>You <01:57.64>will <01:58.01>believe <01:58.95>in <01:59.26>me
[02:01.89]<02:01.89>And <02:02.82>I <02:03.14>will <02:03.45>never <02:04.08>be <02:04.95>ignored
[02:15.51]<02:15.51>I <02:15.70>will <02:15.95>burn <02:16.51>for <02:16.83>you
[02:17.83]<02:17.83>Feel <02:18.39>pain <02:19.07>for <02:19.32>you
[02:20.26]<02:20.26>I <02:20.58>will <02:20.95>twist <02:21.57>the <02:21.89>knife <02:22.82>and <02:23.39>bleed <02:24.07>my <02:24.70>aching <02:26.14>heart
[02:30.14]<02:30.14>And <02:30.32>tear <02:30.70>it <02:31.14>apart
[02:35.83]<02:35.83>I <02:36.07>will <02:36.32>lie <02:36.89>for <02:37.20>you
[02:38.20]<02:38.20>Beg <02:38.76>and <02:39.01>steal <02:39.45>for <02:39.82>you
[02:40.70]<02:40.70>I <02:41.01>will <02:41.32>crawl <02:42.01>on <02:42.58>hands <02:43.20>and <02:43.83>knees <02:44.45>until <02:45.70>you <02:46.39>see
[02:50.51]<02:50.51>You''re <02:50.95>just <02:51.26>like <02:51.64>me
[02:57.51]<02:57.51>Violate <02:58.45>all <02:58.95>the <02:59.20>love <02:59.83>that <03:00.32>I''m <03:00.70>missing
[03:02.45]<03:02.45>Throw <03:02.95>away <03:03.58>all <03:04.08>the <03:04.26>pain <03:04.89>that <03:05.20>I''m <03:05.51>living
[03:08.08]<03:08.08>You <03:08.95>will <03:09.39>believe <03:10.20>in <03:10.58>me
[03:13.26]<03:13.26>And <03:14.14>I <03:14.45>can <03:14.76>never <03:15.39>be <03:16.33>ignored
[03:27.08]<03:27.08>I <03:27.26>would <03:27.45>die <03:27.95>for <03:28.20>you
[03:31.01]<03:31.01>I <03:31.89>would <03:32.26>kill <03:32.89>for <03:33.14>you
[03:37.01]<03:37.01>I <03:37.20>will <03:37.45>steal <03:38.08>for <03:38.45>you
[03:41.45]<03:41.45>I''d <03:42.14>do <03:42.51>time <03:43.08>for <03:43.51>you
[03:47.08]<03:47.08>I <03:47.26>will <03:47.57>wait <03:48.51>for <03:48.70>you
[03:51.39]<03:51.39>I''d <03:52.39>make <03:52.70>room <03:53.64>for <03:54.01>you
[03:57.32]<03:57.32>I''d <03:57.51>sink <03:58.14>ships <03:58.70>for <03:59.08>you
[04:01.51]<04:01.51>To <04:02.51>be <04:02.83>close <04:03.58>to <04:03.89>you
[04:07.58]<04:07.58>To <04:07.83>be <04:08.02>part <04:08.70>of <04:09.01>you
[04:12.70]<04:12.70>Cause <04:12.95>I <04:13.14>believe <04:14.01>in <04:14.32>you
[04:17.83]<04:17.83>I <04:18.01>believe <04:18.70>in <04:19.20>you
[04:22.95]<04:22.95>I <04:23.14>would <04:23.51>die <04:23.95>for <04:24.26>you
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (5080, 'lrc', 'line', 'local_lrc', '[00:00.83]<00:00.83>#<00:01.09>1 <00:01.27>Crush - <00:01.47>Garbage
[00:01.67]<00:01.67>Lyrics <00:01.83>by：<00:02.04>Butch <00:02.23>Vig
[00:02.44]<00:02.44>Composed <00:02.63>by：<00:02.83>Butch <00:03.02>Vig
[00:19.73]<00:19.73>I <00:20.03>would <00:20.35>die <00:21.05>for <00:21.46>you
[00:22.44]<00:22.44>I <00:22.70>would <00:22.98>die <00:23.55>for <00:23.77>you
[00:24.90]<00:24.90>I''ve <00:25.10>been <00:25.40>dying <00:26.75>just <00:27.41>to <00:28.07>feel <00:28.76>you <00:29.49>by <00:29.91>my <00:30.48>side
[00:34.45]<00:34.45>To <00:34.67>know <00:34.98>that <00:35.33>you''re <00:35.60>mine
[00:40.14]<00:40.14>I <00:40.36>will <00:40.65>cry <00:41.36>for <00:41.61>you
[00:42.69]<00:42.69>I <00:42.93>will <00:43.25>cry <00:43.96>for <00:44.18>you
[00:45.19]<00:45.19>I <00:45.48>will <00:45.80>wash <00:46.45>away <00:47.87>your <00:48.50>pain <00:49.14>with <00:49.73>all <00:50.33>my <00:50.93>tears
[00:55.06]<00:55.06>And <00:55.26>drown <00:55.60>your <00:56.02>fears
[01:21.21]<01:21.21>I <01:21.42>will <01:21.66>pray <01:22.14>for <01:22.45>you
[01:23.61]<01:23.61>I <01:23.84>will <01:24.06>pray <01:24.74>for <01:24.95>you
[01:26.09]<01:26.09>I <01:26.34>will <01:26.59>sell <01:27.23>my <01:27.91>soul <01:28.53>for <01:29.16>something <01:30.42>pure <01:31.03>and <01:31.78>true
[01:35.80]<01:35.80>Someone <01:36.87>like <01:37.22>you
[01:42.62]<01:42.62>See <01:42.87>your <01:43.17>face <01:43.89>every <01:44.42>place <01:45.12>that <01:45.43>I <01:45.75>walk <01:46.32>in
[01:47.75]<01:47.75>Hear <01:48.03>your <01:48.33>voice <01:48.92>every <01:49.52>time <01:50.27>that <01:50.54>I''m <01:50.91>talking
[01:54.20]<01:54.20>You <01:54.44>will <01:54.76>believe <01:55.63>in <01:55.88>me
[01:59.33]<01:59.33>And <01:59.54>I <01:59.80>will <02:00.20>never <02:01.12>be <02:01.64>ignored
[02:12.06]<02:12.06>I <02:12.32>will <02:12.57>burn <02:13.22>for <02:13.46>you
[02:14.61]<02:14.61>Feel <02:14.86>pain <02:15.69>for <02:15.99>you
[02:17.13]<02:17.13>I <02:17.34>will <02:17.61>twist <02:18.32>the <02:18.89>knife <02:19.47>and <02:20.18>bleed <02:20.77>my <02:21.49>aching <02:22.73>heart
[02:26.51]<02:26.51>And <02:26.77>tear <02:27.39>it <02:27.70>apart
[02:32.36]<02:32.36>I <02:32.58>will <02:32.93>lie <02:33.53>for <02:33.83>you
[02:34.97]<02:34.97>Beg <02:35.22>and <02:35.49>steal <02:36.11>for <02:36.38>you
[02:37.49]<02:37.49>I <02:37.67>will <02:37.93>crawl <02:38.73>on <02:39.26>hands <02:39.89>and <02:40.50>knees <02:41.19>until <02:42.45>you <02:43.07>see
[02:47.14]<02:47.14>You''re <02:47.59>just <02:47.89>like <02:48.27>me
[02:54.07]<02:54.07>Violate <02:55.13>all <02:55.47>the <02:55.81>love <02:56.42>that <02:56.72>I''m <02:57.05>missing
[02:59.05]<02:59.05>Throw <02:59.36>away <03:00.17>all <03:00.52>the <03:00.82>pain <03:01.58>that <03:01.82>I''m <03:02.11>living
[03:05.51]<03:05.51>You <03:05.79>will <03:06.07>believe <03:06.84>in <03:07.15>me
[03:10.73]<03:10.73>And <03:10.93>I <03:11.16>can <03:11.46>never <03:12.41>be <03:13.03>ignored
[03:23.45]<03:23.45>I <03:23.63>would <03:23.86>die <03:24.45>for <03:24.76>you
[03:28.29]<03:28.29>I <03:28.54>would <03:28.87>kill <03:29.61>for <03:29.95>you
[03:33.35]<03:33.35>I <03:33.68>will <03:34.00>steal <03:34.96>for <03:35.30>you
[03:38.74]<03:38.74>I''d <03:38.94>do <03:39.21>time <03:40.02>for <03:40.46>you
[03:43.73]<03:43.73>I <03:43.98>will <03:44.26>wait <03:45.20>for <03:45.52>you
[03:48.71]<03:48.71>I''d <03:49.05>make <03:49.42>room <03:50.17>for <03:50.65>you
[03:53.83]<03:53.83>I''d <03:54.14>sink <03:54.70>ships <03:55.28>for <03:55.71>you
[03:58.98]<03:58.98>To <03:59.28>be <03:59.62>close <04:00.53>to <04:00.89>you
[04:04.14]<04:04.14>To <04:04.38>be <04:04.81>part <04:05.50>of <04:05.91>you
[04:09.21]<04:09.21>''Cause <04:09.47>I <04:09.86>believe <04:10.68>in <04:10.97>you
[04:14.21]<04:14.21>I <04:14.54>believe <04:15.54>in <04:16.05>you
[04:19.30]<04:19.30>I <04:19.65>would <04:20.02>die <04:20.63>for <04:20.92>you
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (14595, 'lrc', 'line', 'local_lrc', '[00:00.83]<00:00.83>#<00:01.09>1 <00:01.27>Crush - <00:01.47>Garbage
[00:01.67]<00:01.67>Lyrics <00:01.83>by：<00:02.04>Butch <00:02.23>Vig
[00:02.44]<00:02.44>Composed <00:02.63>by：<00:02.83>Butch <00:03.02>Vig
[00:19.73]<00:19.73>I <00:20.03>would <00:20.35>die <00:21.05>for <00:21.46>you
[00:22.44]<00:22.44>I <00:22.70>would <00:22.98>die <00:23.55>for <00:23.77>you
[00:24.90]<00:24.90>I''ve <00:25.10>been <00:25.40>dying <00:26.75>just <00:27.41>to <00:28.07>feel <00:28.76>you <00:29.49>by <00:29.91>my <00:30.48>side
[00:34.45]<00:34.45>To <00:34.67>know <00:34.98>that <00:35.33>you''re <00:35.60>mine
[00:40.14]<00:40.14>I <00:40.36>will <00:40.65>cry <00:41.36>for <00:41.61>you
[00:42.69]<00:42.69>I <00:42.93>will <00:43.25>cry <00:43.96>for <00:44.18>you
[00:45.19]<00:45.19>I <00:45.48>will <00:45.80>wash <00:46.45>away <00:47.87>your <00:48.50>pain <00:49.14>with <00:49.73>all <00:50.33>my <00:50.93>tears
[00:55.06]<00:55.06>And <00:55.26>drown <00:55.60>your <00:56.02>fears
[01:21.21]<01:21.21>I <01:21.42>will <01:21.66>pray <01:22.14>for <01:22.45>you
[01:23.61]<01:23.61>I <01:23.84>will <01:24.06>pray <01:24.74>for <01:24.95>you
[01:26.09]<01:26.09>I <01:26.34>will <01:26.59>sell <01:27.23>my <01:27.91>soul <01:28.53>for <01:29.16>something <01:30.42>pure <01:31.03>and <01:31.78>true
[01:35.80]<01:35.80>Someone <01:36.87>like <01:37.22>you
[01:42.62]<01:42.62>See <01:42.87>your <01:43.17>face <01:43.89>every <01:44.42>place <01:45.12>that <01:45.43>I <01:45.75>walk <01:46.32>in
[01:47.75]<01:47.75>Hear <01:48.03>your <01:48.33>voice <01:48.92>every <01:49.52>time <01:50.27>that <01:50.54>I''m <01:50.91>talking
[01:54.20]<01:54.20>You <01:54.44>will <01:54.76>believe <01:55.63>in <01:55.88>me
[01:59.33]<01:59.33>And <01:59.54>I <01:59.80>will <02:00.20>never <02:01.12>be <02:01.64>ignored
[02:12.06]<02:12.06>I <02:12.32>will <02:12.57>burn <02:13.22>for <02:13.46>you
[02:14.61]<02:14.61>Feel <02:14.86>pain <02:15.69>for <02:15.99>you
[02:17.13]<02:17.13>I <02:17.34>will <02:17.61>twist <02:18.32>the <02:18.89>knife <02:19.47>and <02:20.18>bleed <02:20.77>my <02:21.49>aching <02:22.73>heart
[02:26.51]<02:26.51>And <02:26.77>tear <02:27.39>it <02:27.70>apart
[02:32.36]<02:32.36>I <02:32.58>will <02:32.93>lie <02:33.53>for <02:33.83>you
[02:34.97]<02:34.97>Beg <02:35.22>and <02:35.49>steal <02:36.11>for <02:36.38>you
[02:37.49]<02:37.49>I <02:37.67>will <02:37.93>crawl <02:38.73>on <02:39.26>hands <02:39.89>and <02:40.50>knees <02:41.19>until <02:42.45>you <02:43.07>see
[02:47.14]<02:47.14>You''re <02:47.59>just <02:47.89>like <02:48.27>me
[02:54.07]<02:54.07>Violate <02:55.13>all <02:55.47>the <02:55.81>love <02:56.42>that <02:56.72>I''m <02:57.05>missing
[02:59.05]<02:59.05>Throw <02:59.36>away <03:00.17>all <03:00.52>the <03:00.82>pain <03:01.58>that <03:01.82>I''m <03:02.11>living
[03:05.51]<03:05.51>You <03:05.79>will <03:06.07>believe <03:06.84>in <03:07.15>me
[03:10.73]<03:10.73>And <03:10.93>I <03:11.16>can <03:11.46>never <03:12.41>be <03:13.03>ignored
[03:23.45]<03:23.45>I <03:23.63>would <03:23.86>die <03:24.45>for <03:24.76>you
[03:28.29]<03:28.29>I <03:28.54>would <03:28.87>kill <03:29.61>for <03:29.95>you
[03:33.35]<03:33.35>I <03:33.68>will <03:34.00>steal <03:34.96>for <03:35.30>you
[03:38.74]<03:38.74>I''d <03:38.94>do <03:39.21>time <03:40.02>for <03:40.46>you
[03:43.73]<03:43.73>I <03:43.98>will <03:44.26>wait <03:45.20>for <03:45.52>you
[03:48.71]<03:48.71>I''d <03:49.05>make <03:49.42>room <03:50.17>for <03:50.65>you
[03:53.83]<03:53.83>I''d <03:54.14>sink <03:54.70>ships <03:55.28>for <03:55.71>you
[03:58.98]<03:58.98>To <03:59.28>be <03:59.62>close <04:00.53>to <04:00.89>you
[04:04.14]<04:04.14>To <04:04.38>be <04:04.81>part <04:05.50>of <04:05.91>you
[04:09.21]<04:09.21>''Cause <04:09.47>I <04:09.86>believe <04:10.68>in <04:10.97>you
[04:14.21]<04:14.21>I <04:14.54>believe <04:15.54>in <04:16.05>you
[04:19.30]<04:19.30>I <04:19.65>would <04:20.02>die <04:20.63>for <04:20.92>you
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (124, 'lrc', 'line', 'local_lrc', '[00:02.08]I feel I have to share
[00:25.01]What I do or where I am
[00:28.04]There''s an urge within I can''t ignore
[00:34.02]Hashtag me and go
[00:37.01]I''m addicted to your love
[00:39.06]I''m addicted to my aimless drive
[00:45.06]There''s a mask upon my face I can''t live without
[00:50.05]So you won''t recognize me when I am in the crowd
[00:56.01]I lost my calmness in the world
[00:59.07]Where everything is searchable
[01:03.02]I''m dreaming inside out
[01:08.09]Feeling inside out
[01:14.09]
[01:26.03]I don''t have my place
[01:40.02]I''m in here and everywhere
[01:42.07]Just another day in HD frame
[01:48.09]So hashtag me and go
[01:51.00]Cause I''m addicted to your love
[01:54.01]I''m afraid you''re the only friend I''ve got
[01:59.02]There''s a mask upon my face I can''t live without
[02:05.02]So you won''t recognize me when I am in the crowd
[02:11.00]I lost my calmness in the world
[02:14.09]Where everything is searchable
[02:17.06]I''m dreaming inside out (where everything is searchable)
[02:23.06]Feeling inside out (where everything is searchable)
[02:30.04]Dreaming inside out (where everything is searchable)
[02:35.01]Feeling inside out
[02:42.01]
[03:04.07](Hashtag me and go)
[03:06.01](''Cause I''m addicted to your love)
[03:11.01]
[03:20.02]I lost my calmness in the world
[03:23.04]Where everything is searchable
[03:26.04]I''m dreaming inside out (where everything is searchable)
[03:32.01]Feeling inside out (where everything is searchable)
[03:38.05]Dreaming inside out (where everything is searchable)
[03:43.08]Feeling inside out
[03:49.03]
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (11792, 'lrc', 'line', 'local_lrc', '[00:00.17]<00:00.17>&Run <00:00.67>- <00:00.70>Sir <00:00.73>Sly
[00:00.76]<00:00.76>Written <00:00.80>by<00:01.13>：<00:01.35>Landon Jacobs/Jason Suwito/Hayden Coplen
[00:21.74]<00:21.74>You <00:21.91>could <00:22.11>be <00:22.32>another <00:22.92>face
[00:23.57]<00:23.57>That <00:23.77>I <00:24.01>forget <00:25.00>soon <00:25.23>as <00:25.42>I <00:25.69>move <00:26.06>along
[00:26.95]<00:26.95>Everybody <00:27.65>makes <00:28.01>mistakes
[00:28.79]<00:28.79>Am <00:28.94>I <00:29.17>mistaken <00:29.88>for <00:30.06>the <00:30.26>way <00:30.57>I <00:30.83>carry <00:31.45>on
[00:32.12]<00:32.12>You <00:32.27>could <00:32.45>show <00:32.66>a <00:32.85>little <00:33.24>grace
[00:34.00]<00:34.00>But <00:34.19>maybe <00:34.63>things <00:35.03>just <00:35.45>went <00:35.77>a <00:35.94>bit <00:36.26>too <00:36.69>far
[00:37.80]<00:37.80>We <00:38.04>are <00:38.39>just <00:38.67>who <00:38.94>we <00:39.36>are <00:40.33>no <00:40.55>time <00:41.14>for <00:41.42>what <00:41.68>ifs
[00:42.55]<00:42.55>And <00:42.73>what <00:42.90>if <00:43.14>nots
[00:44.62]<00:44.62>Heavy <00:45.26>as <00:45.56>the <00:45.86>setting <00:46.76>sun
[00:49.07]<00:49.07>Oh <00:49.36>I''m <00:49.73>counting <00:50.33>all <00:50.73>the <00:51.04>numbers <00:51.66>between <00:52.38>zero <00:53.05>and <00:53.32>one
[00:54.91]<00:54.91>Happy <00:55.61>but <00:55.98>a <00:56.29>little <00:57.25>lost
[00:59.62]<00:59.62>Well <00:59.86>I <01:00.12>don''t <01:00.56>know <01:00.88>what <01:01.15>I <01:01.43>don''t <01:01.70>know
[01:02.15]<01:02.15>So <01:02.47>I''ll <01:02.79>kick <01:03.15>my <01:03.46>shoes <01:04.18>off <01:04.81>and <01:05.18>run <01:06.04>yeah
[01:08.34]<01:08.34>Kick <01:08.88>my <01:09.32>shoes <01:10.01>off <01:10.41>and <01:10.73>run
[01:13.64]<01:13.64>Kick <01:14.21>my <01:14.65>shoes <01:15.22>off <01:15.63>and <01:15.97>run
[01:16.66]<01:16.66>Run <01:17.81>we''ll <01:17.97>be <01:18.16>running <01:18.49>barefoot
[01:18.90]<01:18.90>Kick <01:19.33>my <01:19.78>shoes <01:20.44>off <01:20.84>and <01:21.13>run
[01:24.05]<01:24.05>Kick <01:24.56>my <01:25.05>shoes <01:25.66>off <01:26.08>and
[01:26.95]<01:26.95>You <01:27.13>could <01:27.32>be <01:27.52>a <01:27.73>happy <01:28.18>bride
[01:28.82]<01:28.82>And <01:29.00>we <01:29.18>could <01:29.46>still <01:29.82>be <01:30.20>blissfully <01:31.05>in <01:31.34>love
[01:32.12]<01:32.12>Instead <01:32.33>of <01:32.53>being <01:32.83>25
[01:34.03]<01:34.03>And <01:34.23>already <01:34.81>feeling <01:35.32>like <01:35.59>you <01:35.78>have <01:36.08>had <01:36.45>enough
[01:37.41]<01:37.41>You <01:37.58>could <01:37.77>be <01:37.96>my <01:38.14>one <01:38.47>regret
[01:39.31]<01:39.31>Infinitely <01:40.54>spiraling <01:41.49>me <01:41.79>down
[01:42.92]<01:42.92>Sometimes <01:43.61>the <01:43.83>world <01:44.22>feels <01:44.86>loud
[01:47.17]<01:47.17>Heavy <01:47.89>as <01:48.20>the <01:48.52>setting <01:49.47>sun
[01:51.68]<01:51.68>Oh <01:51.99>I''m <01:52.35>counting <01:52.91>all <01:53.27>the <01:53.61>numbers <01:54.33>between <01:54.95>zero <01:55.66>and <01:56.00>one
[01:57.55]<01:57.55>Happy <01:58.24>but <01:58.58>a <01:58.90>little <01:59.94>lost
[02:02.16]<02:02.16>Well <02:02.47>I <02:02.73>don''t <02:03.15>know <02:03.52>what <02:03.80>I <02:04.07>don''t <02:04.35>know
[02:04.76]<02:04.76>So <02:05.11>I''ll <02:05.45>kick <02:05.84>my <02:06.13>shoes <02:06.85>off <02:07.45>and <02:07.74>run <02:08.71>yeah
[02:10.94]<02:10.94>Kick <02:11.49>my <02:11.99>shoes <02:12.61>off <02:13.01>and <02:13.34>run
[02:16.14]<02:16.14>Kick <02:16.73>my <02:17.19>shoes <02:17.88>off <02:18.30>and <02:18.58>run
[02:19.29]<02:19.29>Run <02:20.40>we''ll <02:20.60>be <02:20.82>running <02:21.15>barefoot
[02:21.54]<02:21.54>Kick <02:21.93>my <02:22.35>shoes <02:23.03>off <02:23.48>and <02:23.81>run
[02:26.65]<02:26.65>Kick <02:27.16>my <02:27.65>shoes <02:28.26>off <02:28.70>and
[02:30.88]<02:30.88>Run
[02:32.88]<02:32.88>Run <02:33.44>run <02:34.07>run
[02:41.31]<02:41.31>Run
[02:43.29]<02:43.29>Run <02:43.85>run <02:44.49>run
[02:49.78]<02:49.78>Heavy <02:50.50>as <02:50.79>the <02:51.10>setting <02:51.94>sun
[02:54.37]<02:54.37>Oh <02:54.72>I''m <02:54.96>counting <02:55.66>all <02:55.98>the <02:56.30>numbers <02:56.99>between <02:57.64>zero <02:58.29>and <02:58.57>one
[02:58.88]<02:58.88>Run <02:59.60>run <03:00.12>run
[03:00.31]<03:00.31>Happy <03:00.90>but <03:01.27>a <03:01.56>little <03:02.49>lost
[03:04.82]<03:04.82>Well <03:05.06>I <03:05.36>don''t <03:05.77>know <03:06.12>what <03:06.37>I <03:06.67>don''t <03:06.94>know
[03:07.36]<03:07.36>So <03:07.73>I''ll <03:08.06>kick <03:08.39>my <03:08.71>shoes <03:09.36>off <03:09.99>and
[03:10.62]<03:10.62>Run <03:10.99>into <03:11.65>the <03:11.95>setting <03:12.88>sun
[03:19.79]<03:19.79>Run <03:20.39>run <03:20.91>run
[03:21.11]<03:21.11>Run <03:21.53>into <03:22.13>the <03:22.44>setting <03:23.27>sun
[03:30.33]<03:30.33>Run <03:30.82>run
[03:31.13]<03:31.13>I''ll <03:31.52>run <03:31.89>into <03:32.57>the <03:32.87>setting <03:33.66>sun
[03:41.68]<03:41.68>I''ll <03:42.02>run <03:42.37>into <03:42.99>the <03:43.27>setting <03:44.19>sun
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (11509, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>#<00:00.32>icanteven<00:00.64>(<00:00.96>feat.<00:01.28> <00:01.59>French<00:01.91> <00:02.23>Montana<00:02.55>)<00:02.87> <00:03.19>(<00:03.51>Explicit<00:03.83>)<00:04.15> <00:04.47>-<00:04.79> <00:05.10>The<00:05.42> <00:05.74>Neighbourhood
[00:06.07]<00:06.07>Lyrics<00:06.94> <00:07.80>by<00:08.67>：<00:09.54>Derrick<00:10.40> <00:11.27>Johnson
[00:12.14]<00:12.14>Composed<00:13.01> <00:13.87>by<00:14.74>：<00:15.61>The<00:16.48> <00:17.34>Neighborhood
[00:18.21]<00:18.21>Just <00:18.44>got <00:18.65>cheated <00:18.88>on <00:20.04>no <00:20.25>it''s <00:20.52>not <00:20.77>my <00:21.03>day
[00:22.06]<00:22.06>That''s <00:22.29>not <00:22.54>my <00:22.83>b***h <00:23.61>she''s <00:23.91>not <00:24.25>my <00:24.50>girl <00:24.73>she''s <00:24.96>not <00:25.25>my <00:25.47>babe
[00:26.03]<00:26.03>My <00:26.36>stomach''s <00:26.96>in <00:27.25>pain
[00:28.64]<00:28.64>I <00:28.86>hope <00:29.09>I <00:29.30>don''t <00:29.52>throw <00:29.77>up <00:29.99>all <00:30.22>over <00:30.45>what <00:30.66>you <00:30.92>told
[00:31.29]<00:31.29>But <00:31.55>it <00:31.80>hurts <00:32.05>oh <00:32.31>no <00:32.69>no
[00:34.41]<00:34.41>I <00:34.71>can''t <00:34.82>even <00:35.41>I <00:35.63>can''t <00:35.84>even <00:36.16>believe <00:36.37>what <00:36.60>you <00:36.81>did <00:37.03>to <00:37.28>me
[00:38.66]<00:38.66>You <00:38.81>can''t <00:39.05>even <00:39.58>you <00:39.81>can''t <00:40.09>even <00:40.37>say <00:40.64>I''m <00:40.96>overreacting
[00:42.94]<00:42.94>I <00:43.18>can''t <00:43.46>even <00:43.85>can''t <00:44.11>even <00:44.34>hear <00:44.60>your <00:44.97>side
[00:46.41]<00:46.41>Shame <00:47.45>on <00:47.67>me <00:48.26>you <00:48.53>fooled <00:49.14>me <00:49.36>twice
[00:55.85]<00:55.85>And <00:56.05>you <00:56.25>said <00:56.48>I <00:56.69>wasn''t <00:56.88>just <00:57.05>like <00:57.50>anyone <00:58.62>like
[01:02.10]<01:02.10>But <01:02.34>you <01:02.58>treated <01:02.85>me <01:03.09>just <01:03.40>like <01:03.69>everyone <01:04.18>like <01:04.41>everyone <01:05.24>else
[01:05.92]<01:05.92>You <01:06.18>like <01:06.42>to <01:06.65>say <01:07.12>that <01:07.39>you''re <01:07.66>right
[01:08.46]<01:08.46>Did <01:08.67>it <01:08.90>make <01:09.12>you <01:09.34>feel <01:09.57>bad
[01:09.79]<01:09.79>When <01:09.99>you <01:10.20>cheated <01:10.40>on <01:10.63>your <01:10.84>man <01:11.05>last <01:11.25>night
[01:11.84]<01:11.84>Did <01:12.05>I <01:12.24>even <01:12.45>ever <01:12.67>''cross <01:12.90>your <01:13.18>mind
[01:14.22]<01:14.22>You <01:14.47>like <01:14.70>to <01:14.95>say <01:15.29>that <01:15.69>you''re <01:15.97>right
[01:16.95]<01:16.95>Did <01:17.16>it <01:17.36>make <01:17.62>you <01:17.83>feel <01:18.05>bad
[01:18.26]<01:18.26>When <01:18.47>you <01:18.68>cheated <01:18.90>on <01:19.11>your <01:19.34>man <01:19.56>last <01:19.77>night
[01:20.24]<01:20.24>Did <01:20.46>I <01:20.68>even <01:20.91>ever <01:21.13>''cross <01:21.82>your <01:22.07>mind
[01:22.78]<01:22.78>You <01:23.00>like <01:23.25>to <01:23.45>say <01:23.93>that <01:24.20>you''re <01:24.43>ri-
[01:24.92]<01:24.92>You <01:25.17>like <01:25.43>to <01:25.70>say <01:25.91>that <01:26.16>you''re <01:26.42>right
[01:27.58]<01:27.58>More <01:27.84>ten
[01:28.56]<01:28.56>Drop <01:28.89>head <01:29.13>with <01:29.33>a <01:29.55>time <01:29.79>hoodie <01:30.01>on <01:30.23>me
[01:30.74]<01:30.74>Bought <01:30.97>a <01:31.23>hundred <01:31.45>chains <01:31.66>now <01:31.87>the <01:32.07>b***hes <01:32.27>all <01:32.48>linger
[01:32.81]<01:32.81>Walk <01:33.05>by <01:33.24>the <01:33.47>crib <01:33.68>smell <01:33.88>the <01:34.08>kush <01:34.29>all <01:34.51>stanky
[01:34.83]<01:34.83>Floor <01:35.05>seats <01:35.26>4 <01:35.54>quarter <01:35.76>spent <01:35.96>a <01:36.16>quarter <01:36.37>on <01:36.57>the <01:36.76>link
[01:37.01]<01:37.01>Ridin'' <01:37.24>through <01:37.45>Philly <01:37.74>Meek <01:38.06>Milly <01:38.30>never <01:38.54>lost
[01:38.91]<01:38.91>Still <01:39.14>ridin'' <01:39.34>through <01:39.57>the <01:39.80>strip <01:40.01>catchin'' <01:40.22>licks <01:40.44>with <01:40.66>my <01:40.90>dog
[01:41.12]<01:41.12>30 <01:41.35>for <01:41.60>the <01:41.82>whole <01:42.24>15 <01:42.49>for <01:42.73>the <01:42.95>half
[01:43.16]<01:43.16>You <01:43.39>could <01:43.63>break <01:43.85>it <01:44.08>down <01:44.28>come <01:44.47>and <01:44.67>see <01:44.86>me <01:45.07>with <01:45.28>the <01:45.48>bag
[01:45.70]<01:45.70>Shorty <01:45.91>fell <01:46.16>in <01:46.37>love <01:46.58>with <01:46.77>a <01:46.97>young <01:47.15>rich <01:47.36>n***a
[01:47.55]<01:47.55>Blunt <01:47.75>full <01:47.98>of <01:48.19>smoke <01:48.41>and <01:48.60>a <01:48.96>cup <01:49.16>full <01:49.35>of <01:49.56>liquor
[01:49.75]<01:49.75>Say <01:49.92>what <01:50.13>I <01:50.33>mean <01:50.53>and <01:50.73>I <01:50.94>mean <01:51.12>what <01:51.33>I <01:51.52>said <01:51.73>baby
[01:51.92]<01:51.92>Silk <01:52.13>sheets <01:52.35>I <01:52.56>be <01:52.80>slippin'' <01:53.01>off <01:53.21>the <01:53.41>bed <01:53.61>baby
[01:54.28]<01:54.28>I <01:54.45>can''t <01:54.67>even <01:55.42>I <01:55.60>can''t <01:55.78>even <01:56.00>believe <01:56.23>what <01:56.43>you <01:56.65>did <01:56.86>to <01:57.11>me
[01:58.51]<01:58.51>You <01:58.71>can''t <01:59.27>even <01:59.79>you <01:59.99>can''t <02:00.17>even <02:00.38>say <02:00.61>I''m <02:00.83>overreacting
[02:02.69]<02:02.69>I <02:02.92>can''t <02:03.13>even <02:03.69>can''t <02:03.96>even <02:04.19>hear <02:04.46>your <02:05.08>side
[02:06.36]<02:06.36>Shame <02:06.67>on <02:07.19>me <02:08.39>you <02:08.72>fooled <02:09.10>me <02:09.40>twice
[02:15.78]<02:15.78>And <02:16.02>you <02:16.25>said <02:16.49>I <02:16.69>wasn''t <02:16.89>just <02:17.11>like <02:17.39>anyone <02:18.68>like
[02:22.10]<02:22.10>But <02:22.32>you <02:22.52>treated <02:22.84>me <02:22.97>just <02:23.17>like <02:23.47>everyone <02:24.28>like <02:24.56>everyone <02:25.35>else
[02:26.00]<02:26.00>You <02:26.24>like <02:26.53>to <02:26.82>say <02:27.12>that <02:27.44>you''re <02:27.76>right
[02:28.51]<02:28.51>Did <02:28.76>it <02:29.00>make <02:29.21>you <02:29.43>feel <02:29.67>bad
[02:29.89]<02:29.89>When <02:30.12>you <02:30.33>cheated <02:30.54>on <02:30.76>your <02:30.96>man <02:31.18>last <02:31.40>night
[02:32.11]<02:32.11>Did <02:32.36>I <02:32.59>even <02:32.80>ever <02:33.05>''cross <02:33.39>your <02:33.67>mind
[02:34.28]<02:34.28>You <02:34.51>like <02:34.72>to <02:34.96>say <02:35.35>that <02:35.61>you''re <02:35.99>right
[02:37.01]<02:37.01>Did <02:37.25>it <02:37.48>make <02:37.71>you <02:37.92>feel <02:38.16>bad
[02:38.36]<02:38.36>When <02:38.57>you <02:38.80>cheated <02:39.03>on <02:39.25>your <02:39.48>man <02:39.69>last <02:39.92>night
[02:40.41]<02:40.41>Did <02:40.60>I <02:40.84>even <02:41.08>ever <02:41.36>''cross <02:41.68>your <02:41.95>mind
[02:42.60]<02:42.60>You <02:42.87>like <02:43.17>to <02:43.39>say <02:43.62>that <02:43.89>you''re <02:44.12>right
[02:44.74]<02:44.74>You <02:45.00>like <02:45.23>to <02:45.44>say <02:45.67>that <02:45.90>you''re <02:46.13>right
[03:12.43]<03:12.43>You <03:12.71>like <03:12.94>to <03:13.15>say <03:13.35>that <03:13.54>you''re <03:13.74>right
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (8296, 'lrc', 'line', 'local_lrc', '[00:21.30]Yeah
[00:22.38]I can''t be ridin'' with a sucker, nigga,
[00:24.05]Nah uh-uh, no sir, can''t do it (no sir)
[00:27.95]Can''t see yourself going broke no
[00:30.62]Time soon, no sir, can''t view it (no sir)
[00:32.75]My niggas in the game, all you niggas on the sidelines
[00:36.10]Lookin'' mad as hell, bitch, Jon Gruden (god-damn)
[00:38.78]I take my Nikes off and put them Saint Laurent''s on my feet
[00:41.67]Still wanna come to rap? Then just do it
[00:45.92]
[00:50.18]Young nigga, what your life like?
[00:52.74]All my niggas ballin'' round here
[00:54.69]Reppin'' players like a highlight
[00:56.42]And all my bitches out here lookin'' like fine wine
[00:59.35]All your bitches out here lookin'' like fright night, yikes
[01:02.76]20 thousand dollars on a Rollie, no Ice
[01:05.31]Know you can''t afford it if you ask ''bout the price
[01:08.35]See I been gettin'' played like all my damn life
[01:11.45]If it ain''t about no money, you just don''t live right, agh!
[01:30.94]
[01:37.13]Change all the time, can change all the time
[01:39.79]If I really want to I could change all your minds
[01:42.64]I change in the day, I change in the night
[01:45.60]I paint it all black and I paint it all white
[01:48.43]Change all the time, can change all the time
[01:51.04]If I really want to I could change all your minds
[01:54.08]I change in the day, I change in the night
[01:56.75]I paint it all black and I paint it all white
[01:59.64]Margiela to offset her
[02:01.14]My money under this jacket
[02:02.58]Versace boots for my stature
[02:03.86]Can''t dap me, I''m too dapper, damn
[02:06.25]I mean I''m fleeker than the scammers on the damn ground
[02:08.87]Vintage, trans, strange, X-Men: Last Stand
[02:11.82]And your last man shoppin'' out my trash can
[02:14.61]Recycle bin has been, damn, how ya life been?
[02:17.31]Tell us how the lights been
[02:18.85]Tell us how your night ends
[02:20.26]Shit, I don''t fuck the groupie hoes or they hype friends
[02:23.12]Benjamins take ''em in, call it a night''s end
[02:25.72]Smilin'' the whole damn time thinkin'' "nice win"
[02:29.14]I remember nights when ends was the end, friends was absent
[02:32.22]Fueled by nothing but passion, now
[02:34.68]It''s packs of hundreds, these niggas they done done it
[02:37.13]You guessed just how they did it
[02:38.44]They dumb it just so they near it
[02:39.64]They payin'' ''cause we amazing
[02:41.28]Put a penis in the Caymans
[02:42.55]Now you don''t know if I''m playing, that''s a win right there
[02:45.69]You tryna celebrate, oh, that''s some head right there
[02:48.65]I''m tryna medicate, just put the gas in the air
[02:51.19]And we don''t give a fuck, throwin'' money everywhere
[02:54.07]Welcome the lader''s blessing as we add on the X''s damn
[02:59.13]
[03:09.18]I met a stranger yesterday
[03:13.38]She said it would all just go away (all go)
[03:16.20]And when she put her hands on me
[03:20.94]I met a stranger yesterday
[03:24.98]She said it would all just go away
[03:27.86]And when she put her hands on me
[03:30.66]It was gone, it was gone
[03:36.21]It was gone and gone and gone, gone and gone and gone
[03:41.84]You''re never comin'' back
[03:43.13]Once you go away, you''re never comin'' back
[03:46.07]Once you go away, you''re never comin'' back
[03:49.41]''Ted it, stranger wanted it
[03:54.15](Change all the time, can change all the time
[03:56.99]If I really want to I could change all your minds)
[03:59.63]Oh god, I can''t believe I made this shit
[04:03.90](I change in the day, I change in the night
[04:04.15]I paint it all black and I paint it all white)
[04:04.36]This is for friends and family
[04:07.01](Change all the time, can change all the time
[04:07.29]If I really want to I could change all your minds)
[04:07.58]This is for those who stand right by your side
[04:10.70](I change in the day, I change in the night
[04:10.91]I paint it all black and I paint it all white)
[04:11.20]Friends and family
[04:13.63]This is for ones I won''t forget
[04:15.88]
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (11512, 'lrc', 'line', 'local_lrc', '[00:01.76]<00:01.76>Aw<00:01.99> man<00:02.29> the<00:02.39> whip<00:02.53> black<00:02.69> and<00:03.20> white
[00:05.20]<00:05.20>My<00:05.36> B***h<00:05.58> black<00:05.83> and<00:05.96> white
[00:07.47]<00:07.47>My<00:07.63> fear<00:07.89> black<00:08.45> and<00:08.74> white
[00:11.32]<00:11.32>I''m<00:11.72> never<00:12.21> the<00:12.61> same<00:13.41> I<00:13.51> change<00:13.90> every<00:14.18> week
[00:14.77]<00:14.77>I<00:14.85> won''t<00:15.01> stay<00:15.31> in<00:15.43> the<00:15.64> middle<00:16.02> I''ll<00:16.62> k**l<00:16.76> everything
[00:17.65]<00:17.65>Yeah<00:17.82> I''m<00:17.92> stuck<00:18.27> in<00:18.39> between<00:19.13> if<00:19.32> I''m<00:19.53> wrong<00:19.71> or<00:20.03> I''m<00:20.22> right
[00:20.59]<00:20.59>I<00:20.77> would<00:20.90> ask<00:21.19> for<00:21.35> advice<00:21.86> but<00:22.00> I<00:22.09> just<00:22.46> do<00:22.63> what<00:22.81> I<00:22.93> like
[00:23.46]<00:23.46>Can''t<00:23.69> get<00:23.87> over<00:24.19> the<00:24.29> fact<00:24.88> people<00:25.28> living<00:25.66> a<00:25.77> lie
[00:26.31]<00:26.31>Just<00:26.77> to<00:26.87> stay<00:27.00> entertained<00:27.84> what<00:28.07> a<00:28.17> waste<00:28.49> of<00:28.59> a<00:28.67> life
[00:29.31]<00:29.31>What<00:29.58> a<00:29.65> waste<00:29.94> of<00:30.04> a<00:30.11> space<00:30.74> what<00:30.93> the<00:31.18> f**k<00:31.32> is<00:31.47> your<00:31.76> point
[00:32.24]<00:32.24>You''re<00:32.43> a<00:32.54> waist<00:32.83> with<00:32.96> no<00:33.06> spine<00:33.69> you''re<00:33.88> a<00:34.04> waste<00:34.29> of<00:34.40> my<00:34.59> time
[00:35.25]<00:35.25>I<00:35.33> smoke<00:35.67> cause<00:35.82> I''m<00:35.94> stressed<00:36.70> I<00:36.83> try<00:37.18> to<00:37.31> get<00:37.47> high
[00:38.01]<00:38.01>But<00:38.16> it<00:38.30> gets<00:38.60> me<00:38.72> depressed<00:39.29> I''m<00:39.62> just<00:39.85> tryna<00:40.19> get<00:40.36> by
[00:40.93]<00:40.93>I''m<00:41.22> just<00:41.51> drivin''<00:41.91> at<00:42.24> night<00:42.37> I<00:42.55> got<00:42.70> no<00:42.90> music<00:43.33> on
[00:43.78]<00:43.78>I<00:43.99> got<00:44.17> no<00:44.33> favorite<00:44.68> song<00:45.31> it''s<00:45.65> just<00:45.81> me<00:45.91> and<00:46.04> my<00:46.15> thoughts
[00:46.95]<00:46.95>I''ve<00:47.08> fallen<00:47.54> in<00:47.71> love<00:48.32> I''ve<00:48.54> fallen<00:48.95> behind
[00:49.81]<00:49.81>I''ve<00:50.00> fallen<00:50.36> for<00:50.59> her<00:51.13> more<00:51.31> than<00:51.45> once<00:51.74> only<00:52.01> twice
[00:52.72]<00:52.72>I<00:52.88> fell<00:53.12> in<00:53.25> the<00:53.39> pool<00:54.00> got<00:54.14> chlorine<00:54.58> in<00:54.70> my<00:55.32> eyes
[00:55.68]<00:55.68>And<00:55.82> it<00:55.92> burned<00:56.08> for<00:56.32> a<00:56.39> minute<00:56.95> but<00:57.16> I<00:57.29> didn''t<00:57.61> go<00:57.82> blind
[00:58.29]<00:58.29>This<00:58.42> is<00:58.53> for<00:58.63> my<00:58.87> friends<00:59.06> who<00:59.17> play
[00:59.42]<00:59.42>The<00:59.58> old<00:59.92> cafes<01:00.58> and<01:00.85> they<01:01.01> kick<01:01.30> it<01:01.41> in<01:01.59> the<01:01.69> parking<01:02.24> lot
[01:02.95]<01:02.95>They<01:03.58> call<01:03.92> me<01:04.02> one<01:04.24> take<01:04.40> Jake<01:05.06> baby
[01:05.68]<01:05.68>Well<01:05.98> I<01:06.29> mean<01:06.50> what<01:06.70> I<01:06.82> speak<01:06.99> what<01:07.25> I<01:07.49> feel<01:07.92> with<01:08.19> a<01:08.42> broken<01:08.77> heart
[01:09.07]<01:09.07>I''ve<01:09.42> been<01:09.66> getting<01:10.16> money<01:10.47> all<01:10.60> day<01:11.61> so<01:11.84> I<01:12.14> can<01:12.32> spend<01:12.69> it<01:12.85> all<01:13.13> on<01:13.43> us
[01:13.65]<01:13.65>They<01:13.82> call<01:14.03> me<01:14.17> one<01:14.42> take<01:14.90> Jake<01:15.14> baby
[01:16.00]<01:16.00>I''ve<01:16.14> been<01:16.44> getting<01:16.88> money<01:17.50> all<01:17.74> night<01:18.41> so<01:19.05> I<01:19.92> can<01:20.56> spend<01:21.29> it<01:21.42> all<01:22.67> on<01:23.61> us
[01:24.27]<01:24.27>I<01:24.66> got<01:24.93> that<01:25.64> big<01:25.82> fat<01:26.22> snake<01:26.88> baby
[01:27.42]<01:27.42>You<01:27.55> can''t<01:27.97> get<01:28.11> me<01:28.29> to<01:28.67> spit<01:28.84> but<01:29.08> I''m<01:29.18> so<01:29.44> hard<01:29.60> to<01:29.70> swallow
[01:30.54]<01:30.54>My<01:30.76> daddy<01:31.10> is<01:31.28> dead<01:31.70> I''ve<01:31.81> got<01:32.08> no<01:32.35> man<01:32.56> to<01:32.66> follow
[01:33.29]<01:33.29>And<01:33.46> I<01:33.56> know<01:33.81> that<01:33.98> I''m<01:34.10> shallow<01:34.55> but<01:34.79> why<01:35.05> shouldn''t<01:35.53> I<01:35.60> be
[01:36.12]<01:36.12>I<01:36.32> don''t<01:36.48> mean<01:36.72> to<01:36.82> get<01:37.22> deep<01:37.46> it''s<01:37.59> just<01:38.01> 1<01:38.04> of<01:38.33> those<01:38.73> weaks
[01:39.08]<01:39.08>Couldn''t<01:39.41> tell<01:39.65> you<01:39.82> the<01:39.97> day<01:40.52> couldn''t<01:40.87> tell<01:41.08> you<01:41.25> the<01:41.35> time
[01:42.03]<01:42.03>Trouble<01:42.33> falling<01:42.77> asleep<01:43.27> for<01:43.63> the<01:43.73> past<01:44.04> couple<01:44.40> nights
[01:44.93]<01:44.93>Trouble<01:45.32> being<01:45.63> alone<01:46.37> I''ve<01:46.54> been<01:46.67> losing<01:47.08> my<01:47.22> mind
[01:47.80]<01:47.80>But<01:48.01> I<01:48.08> don''t<01:48.25> want<01:48.43> any<01:48.60> trouble<01:49.16> it<01:49.27> just<01:49.48> chooses<01:49.87> to<01:49.97> find<01:50.55> me
[01:55.08]<01:55.08>This<01:55.48> is<01:55.96> for<01:56.13> my<01:56.24> friends<01:56.43> who<01:56.53> play
[01:58.34]<01:58.34>The<01:58.63> old<02:00.45> cafes<02:00.91> and<02:02.79> they<02:03.39> kick<02:04.31> it<02:04.75> in<02:05.23> the<02:05.73> parking<02:06.29> lot
[02:06.91]<02:06.91>Well<02:08.52> I<02:09.09> mean<02:09.78> what<02:10.59> I<02:10.73> speak<02:11.24> what<02:12.63> I<02:12.77> feel<02:13.82> with<02:14.44> a<02:14.63> broken<02:15.82> heart
[02:16.46]<02:16.46>I''ve<02:17.89> been<02:18.40> getting<02:18.78> money<02:19.45> all<02:19.61> day<02:19.83> so<02:20.30> I<02:20.37> can<02:20.66> spend<02:20.90> it<02:21.27> all<02:21.75> on<02:22.20> us
[02:23.02]<02:23.02>I''ve<02:24.63> been<02:25.08> getting<02:26.43> money<02:27.58> all<02:28.58> night<02:29.47> so<02:29.84> I<02:29.93> can<02:30.16> spend<02:30.62> it<02:30.93> all<02:32.42> on<02:33.53> us
[02:34.05]<02:34.05>Said<02:34.27> I''ve<02:34.72> been<02:34.99> thinkin''<02:35.85> bout<02:36.21> you
[02:36.49]<02:36.49>And<02:36.72> what<02:37.08> we<02:37.33> gon''<02:38.13> do
[02:38.84]<02:38.84>But<02:39.27> I''ve<02:39.40> been<02:39.67> thinkin''<02:41.06> bout<02:42.64> fallin''<02:44.53> in<02:45.02> love
[02:46.63]<02:46.63>I''ve<02:47.00> been<02:47.55> thinkin''<02:47.83> bout<02:49.80> you
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (1917, 'lrc', 'line', 'local_lrc', '[00:12.11]Lips meet teeth and tongue
[00:16.90]My heart skips eight beats at once
[00:22.81]If we were meant to be, we would have been by now
[00:29.03]See what you wanna see, all I see is him right now
[00:34.37]H-h-him right now
[00:36.75]I''ll sit and watch your car burn
[00:41.12]With the fire that you started in me
[00:44.32]But you never came back to ask it out
[00:47.68]Go ahead and watch my heart burn
[00:51.59]With the fire that you started in me
[00:54.50]But I''ll never let you back to put it out
[00:58.60]
[01:01.53]Your love feels so fake
[01:06.56]My demands aren''t high to make
[01:12.57]If I could get to sleep, I would have slept by now
[01:18.49]Your lies will never keep, I think you need to blow them out
[01:24.42](B-b-blow them out)
[01:26.88]I''ll sit and watch your car burn
[01:31.28]With the fire that you started in me
[01:34.06]But you never came back to ask it out
[01:37.13]Go ahead and watch my heart burn
[01:41.13]With the fire that you started in me
[01:44.24]But I''ll never let you back to put it out
[01:48.97]
[01:51.17]7-4-2008, I still remember that, heaven sent a present my way
[01:55.55]I won''t forget your laugh, packing everything when you leave
[01:58.49]You know you coming back, wanna see me down on my knees but that was made for a ring
[02:03.08]I try to wait for the storm to calm down but that''s stubborn, baby, leading to war
[02:07.13]We droned down on each other, tryin'' to even the score
[02:10.17]We all been found guilty in the court of aorta
[02:13.90]And I''ll watch your car burn
[02:17.67]With the fire that you started in me
[02:20.69]But you never came back to ask it out
[02:23.91]Go ahead and watch my heart burn
[02:27.98]With the fire that you started in me
[02:30.80]But I''ll never let you back to put it out
[02:34.75]Watch your car, watch your car burn
[02:38.24]I won''t forget your laugh
[02:38.95]Go ahead and watch my heart, watch my heart burn
[02:42.27]You know you coming back, you know you coming back
[02:43.57]Watch your car, watch your car burn
[02:44.24](Go ahead and watch my heart, watch my heart burn)
[02:48.67]Tryna even the score
[02:49.37]Go ahead and watch my heart, watch my heart burn
[02:52.38]Found guilty in the court of aorta
[02:53.67]
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (20851, 'lrc', 'line', 'local_lrc', '[00:13.16]<00:13.16>Got<00:13.99> <00:14.13>no<00:14.31> <00:14.39>reason<00:14.76> <00:15.28>for<00:16.04> <00:16.13>coming<00:16.30> <00:16.31>to<00:16.43> <00:16.50>me
[00:18.84]<00:18.84>And<00:18.98> <00:19.02>the<00:19.16> <00:19.21>rain<00:19.36> <00:19.41>runnin''<00:20.11> <00:20.24>down
[00:21.79]<00:21.79>There''s<00:22.19> <00:22.22>no<00:22.27> <00:22.31>reason
[00:25.59]<00:25.59>And<00:27.99> <00:28.27>the<00:28.49> <00:28.54>same<00:28.72> <00:28.79>voice
[00:30.53]<00:30.53>Comin''<00:30.61> <00:30.63>to<00:30.66> <00:30.70>me<00:30.76> <00:30.86>like<00:31.43> <00:31.50>it''s<00:31.76> <00:31.79>all<00:31.98> <00:32.03>slowin''<00:32.31> <00:32.65>down
[00:34.70]<00:34.70>And<00:34.90> <00:34.95>believe<00:35.25> <00:35.28>me
[00:39.56]<00:39.56>I<00:39.59> <00:39.63>was<00:39.74> <00:39.78>the<00:39.87> <00:39.91>one<00:40.02> <00:40.06>who<00:40.33> <00:40.41>let<00:40.69> <00:40.81>you<00:41.17> <00:41.24>know
[00:42.79]<00:42.79>I<00:42.86> <00:42.92>was<00:43.02> <00:43.06>your<00:43.16> <00:43.17>"sorry<00:43.61> <00:43.66>ever<00:43.94> <00:44.04>after"
[00:46.27]<00:46.27>''74-<00:47.69>''75
[00:51.69]<00:51.69>It''s<00:54.92> <00:54.99>not<00:55.19> <00:55.26>easy
[00:56.84]<00:56.84>Nothin''<00:56.97> <00:57.01>to<00:57.08> <00:57.11>say<00:57.32> <00:57.44>''cause<00:57.79> <00:57.87>it''s<00:58.32> <00:58.39>already<00:58.88> <00:58.96>said
[01:01.40]<01:01.40>It''s<01:01.67> <01:01.72>never<01:01.97> <01:02.03>easy
[01:06.31]<01:06.31>When<01:06.44> <01:06.48>I<01:06.53> <01:06.58>look<01:06.75> <01:06.83>on<01:07.08> <01:07.13>your<01:07.53> <01:07.76>eyes
[01:09.99]<01:09.99>Then<01:10.11> <01:10.14>I<01:10.17> <01:10.22>find<01:10.58> <01:10.62>that<01:10.91> <01:11.06>I''ll<01:11.54> <01:11.57>do<01:11.66> <01:11.71>fine
[01:12.82]<01:12.82>When<01:13.02> <01:13.05>I<01:13.12> <01:13.17>look<01:13.54> <01:13.72>on<01:14.05> <01:14.11>your<01:14.39> <01:14.45>eyes
[01:16.32]<01:16.32>Then<01:16.54> <01:16.62>I''ll<01:16.80> <01:16.84>do<01:16.90> <01:16.97>better
[01:19.90]<01:19.90>I<01:19.91> <01:19.92>was<01:19.98> <01:20.02>the<01:20.14> <01:20.20>one<01:20.47> <01:20.55>who<01:20.73> <01:20.95>let<01:21.35> <01:21.45>you<01:21.60> <01:21.65>know
[01:22.93]<01:22.93>I<01:22.95> <01:22.98>was<01:23.08> <01:23.13>your<01:23.38> <01:23.41>"sorry<01:23.60> <01:23.63>ever<01:23.88> <01:24.01>after"
[01:26.45]<01:26.45>''74-<01:28.23>''75
[01:33.00]<01:33.00>Giving<01:33.27> <01:33.32>me<01:33.45> <01:33.52>more<01:33.77> <01:33.81>and<01:34.05> <01:34.12>I''ll<01:34.55> <01:34.67>defy
[01:36.62]<01:36.62>''Cause<01:36.82> <01:36.85>you''re<01:37.27> <01:37.30>really<01:37.64> <01:37.75>only<01:38.14> <01:38.30>after
[01:40.22]<01:40.22>''74-<01:41.70>''75
[02:25.15]<02:25.15>Got<02:25.27> <02:25.32>no<02:25.43> <02:27.13>reason<02:27.63> <02:27.66>for<02:27.77> <02:27.82>comin''<02:28.12> <02:28.17>to<02:28.27> <02:28.63>me
[02:29.62]<02:29.62>And<02:29.70> <02:29.72>the<02:29.80> <02:29.85>rain<02:30.00> <02:30.05>runnin''<02:30.42> <02:30.45>down
[02:32.16]<02:32.16>There''s<02:32.62> <02:32.67>no<02:32.77> <02:32.81>reason
[02:36.71]<02:36.71>When<02:36.81> <02:36.84>I<02:36.91> <02:37.09>look<02:37.27> <02:37.31>on<02:37.42> <02:37.62>your<02:37.98> <02:38.03>eyes
[02:40.29]<02:40.29>Then<02:40.54> <02:40.57>I<02:40.64> <02:40.67>find<02:41.07> <02:41.12>that<02:41.31> <02:41.37>I''ll<02:41.90> <02:41.97>do<02:42.11> <02:42.17>fine
[02:43.25]<02:43.25>When<02:43.44> <02:43.52>I<02:43.74> <02:43.81>look<02:44.11> <02:44.17>on<02:44.29> <02:44.34>your<02:44.49> <02:44.54>eyes
[02:46.38]<02:46.38>Then<02:46.63> <02:46.81>I''ll<02:47.01> <02:47.14>do<02:47.26> <02:47.29>better
[02:49.89]<02:49.89>I<02:49.96> <02:50.04>was<02:50.16> <02:50.21>the<02:50.41> <02:50.47>one<02:50.60> <02:50.66>who<02:51.36> <02:51.57>let<02:51.89> <02:51.97>you<02:52.29> <02:52.30>know
[02:53.25]<02:53.25>I<02:53.30> <02:53.35>was<02:53.43> <02:53.46>your<02:53.72> <02:53.83>"sorry<02:54.15> <02:54.16>ever<02:54.30> <02:54.33>after"
[02:56.34]<02:56.34>''74-<02:58.43>''75
[03:03.24]<03:03.24>Giving<03:03.56> <03:03.59>me<03:03.67> <03:03.71>more<03:03.84> <03:03.87>and<03:04.32> <03:04.44>I''ll<03:04.81> <03:04.91>defy
[03:06.84]<03:06.84>''Cause<03:07.04> <03:07.06>you''re<03:07.46> <03:07.53>really<03:07.97> <03:08.06>only<03:08.42> <03:08.54>after
[03:10.44]<03:10.44>''74-<03:11.22>''75
[03:16.68]<03:16.68>I<03:16.71> <03:16.74>was<03:16.85> <03:16.88>the<03:17.08> <03:17.23>one<03:17.50> <03:17.63>who<03:18.16> <03:18.30>let<03:18.51> <03:18.55>you<03:18.65> <03:18.70>know
[03:19.84]<03:19.84>I<03:19.86> <03:19.89>was<03:19.97> <03:19.99>your<03:20.14> <03:20.27>"sorry<03:20.74> <03:20.81>ever<03:21.14> <03:21.29>after"
[03:23.01]<03:23.01>''74-<03:24.84>''75
[03:29.97]<03:29.97>Giving<03:30.14> <03:30.20>me<03:30.30> <03:30.34>more<03:30.47> <03:30.50>and<03:30.66> <03:30.89>I''ll<03:31.64> <03:31.74>defy
[03:33.02]<03:33.02>''Cause<03:33.40> <03:33.57>you''re<03:33.76> <03:33.82>really<03:34.19> <03:34.24>only<03:34.59> <03:34.70>after
[03:36.62]<03:36.62>''74-<03:38.45>''75
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (15824, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>''Round<00:00.38> <00:00.76>Midnight<00:01.15> <00:01.53>-<00:01.91> <00:02.29>Amy<00:02.67> <00:03.06>Winehouse<00:03.44> <00:03.82>(<00:04.20>艾<00:04.58>米<00:04.97>·<00:05.35>怀<00:05.73>恩<00:06.11>豪<00:06.49>斯<00:06.88>)
[00:07.28]<00:07.28>It <00:07.65>begins <00:08.28>to <00:08.59>tell
[00:11.09]<00:11.09>''Round <00:11.28>midnight  <00:13.28>''round <00:13.65>midnight
[00:17.65]<00:17.65>I <00:17.96>do <00:18.40>pretty <00:19.59>well
[00:21.40]<00:21.40>Till <00:21.65>after <00:24.11>sundown <00:28.25>and <00:28.44>suppertime <00:30.19>I''m <00:30.44>feelin'' <00:32.56>sad
[00:37.13]<00:37.13>But <00:37.38>it <00:37.63>really <00:38.06>gets <00:38.50>bad  <00:39.94>''round <00:40.25>midnight
[00:49.49]<00:49.49>Memories <00:50.00>always <00:50.87>start
[00:53.19]<00:53.19>''Round <00:53.44>midnight  <00:55.75>''round <00:56.06>midnight
[00:59.75]<00:59.75>Haven''t <01:00.25>got <01:00.50>the <01:00.81>heart <01:03.56>to <01:03.81>stand <01:05.13>those <01:06.43>memories
[01:09.06]<01:09.06>So <01:10.95>when <01:11.20>my <01:11.45>heart <01:12.20>is <01:12.64>still <01:14.54>with <01:14.73>you
[01:18.79]<01:18.79>Yes <01:19.18>ol'' <01:20.87>midnight <01:22.43>knows <01:23.55>it  <01:24.81>too
[02:02.57]<02:02.57>For <02:02.89>''round <02:03.33>midnight  <02:06.01>when <02:06.20>it <02:06.51>comes <02:07.33>around
[02:13.57]<02:13.57>So <02:13.76>let <02:14.01>our <02:14.57>hearts <02:14.76>take <02:15.07>wings
[02:17.64]<02:17.64>''Round <02:17.82>midnight  <02:20.20>''round <02:20.39>midnight
[02:24.34]<02:24.34>Let <02:24.59>the <02:24.83>angels <02:25.98>sing <02:28.16>for <02:28.41>your <02:29.60>returning
[02:34.86]<02:34.86>Till <02:35.10>our <02:35.35>love <02:36.42>is <02:36.66>safe <02:38.91>and <02:39.16>sound
[02:43.16]<02:43.16>And <02:43.41>old <02:44.10>midnight <02:48.91>comes <02:49.54>around
[02:54.60]<02:54.60>Cause <02:54.79>I''m <02:55.04>feelin'' <02:55.29>sad
[02:57.04]<02:57.04>And <02:57.23>it <02:57.47>really <02:57.73>gets <02:57.97>bad
[02:59.79]<02:59.79>''Round <03:00.04>midnight  <03:02.35>''round <03:02.54>midnight
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (21412, 'lrc', 'line', 'local_lrc', '[00:00.55]<00:00.55>''Cause <00:00.80>I <00:01.11>Sez <00:01.36>So <00:01.67>- <00:01.92>New <00:02.17>York <00:02.42>Dolls
[00:29.69]<00:29.69>Go <00:29.94>point <00:30.13>your <00:30.31>camera <00:30.62>some <00:30.94>other <00:31.19>way
[00:33.06]<00:33.06>Ain''t <00:33.31>gonna <00:33.56>be <00:33.94>in <00:34.25>your <00:34.50>movie <00:34.75>today
[00:36.69]<00:36.69>You <00:36.94>got <00:37.19>surveillance <00:37.50>for <00:37.81>reality
[00:39.69]<00:39.69>Then <00:39.94>you <00:40.19>best <00:40.50>not <00:40.81>even <00:41.12>look <00:41.69>at <00:42.00>me
[00:43.06]<00:43.06>I <00:43.31>sez Why  <00:43.56>why  <00:44.00>why  <00:45.37>Cause <00:45.69>I <00:45.94>sez <00:46.19>so
[00:50.70]<00:50.70>Why  <00:51.02>why  <00:51.27>why  <00:51.58>Cause <00:51.83>I <00:52.08>sez <00:52.27>so
[00:57.64]<00:57.64>Takin'' <00:58.08>pretty <00:58.39>pictures <00:58.70>everywhere <00:58.95>I <01:00.08>go
[01:01.39]<01:01.39>Orwell <01:01.70>in <01:02.01>the <01:02.33>bathroom <01:02.70>watching <01:03.01>me <01:03.20>go
[01:05.14]<01:05.14>I <01:05.45>give <01:05.70>the <01:05.95>finger <01:06.51>to <01:06.76>the <01:06.95>eye <01:07.20>in <01:07.39>the <01:07.58>sky
[01:08.64]<01:08.64>I <01:08.89>ain''t <01:09.14>no <01:09.39>model  <01:09.76>I''m <01:10.01>a <01:10.32>regular <01:10.89>guy
[01:11.89]<01:11.89>Why  <01:12.20>why  <01:12.82>why  <01:13.07>Cause <01:13.39>I <01:13.58>sez <01:13.83>so
[01:19.01]<01:19.01>Why  <01:19.32>why  <01:19.76>why  <01:20.14>Cause <01:20.51>I <01:20.76>sez <01:21.01>so
[01:54.64]<01:54.64>Even <01:54.95>the <01:55.20>bodega <01:55.51>was <01:55.83>makin'' <01:56.07>movies  <01:56.32>yo
[01:58.07]<01:58.07>Everything <01:58.39>I <01:58.64>do <01:58.95>is <01:59.26>on <01:59.51>the <01:59.70>video
[02:01.89]<02:01.89>Flew <02:02.14>into <02:02.32>JFK  <02:02.70>was <02:02.95>lousy <02:03.14>with <02:03.39>oink
[02:05.45]<02:05.45>I <02:05.75>hear <02:05.93>they <02:06.18>lock <02:06.37>you <02:06.62>up <02:06.87>for <02:07.06>smokin'' <02:07.25>a <02:07.43>joint
[02:08.68]<02:08.68>Why  <02:08.99>why  <02:09.43>why  <02:09.81>Cause <02:10.13>I <02:10.32>sez <02:10.57>so
[02:15.20]<02:15.20>Why  <02:15.88>why  <02:16.20>why  <02:16.51>Cause <02:16.82>I <02:17.01>sez <02:17.26>so
[02:21.83]<02:21.83>I <02:26.58>sez <02:26.89>so
[02:29.76]<02:29.76>I <02:30.08>sez <02:30.33>so
[02:32.64]<02:32.64>I <02:33.26>sez <02:33.52>so
[02:36.95]<02:36.95>Yeah
[02:40.01]<02:40.01>Yeah
[02:43.39]<02:43.39>I <02:43.64>say <02:43.83>yeah
[02:49.83]<02:49.83>Cause <02:50.08>I <02:50.26>sez <02:50.51>so
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (21419, 'lrc', 'line', 'local_lrc', '[00:01.46]<00:01.46>Say <00:01.71>man  <00:02.51>they <00:02.80>tell <00:02.97>me <00:03.12>you <00:03.27>think <00:03.41>you''re <00:03.59>pretty <00:03.76>good
[00:05.26]<00:05.26>Don''t <00:05.44>you <00:05.63>know <00:06.07>you''re <00:06.31>in <00:06.47>my <00:06.83>neighborhood
[00:08.95]<00:08.95>And <00:09.16>they <00:09.36>tell <00:09.57>me <00:09.76>you''re <00:09.93>pretty <00:10.12>fast <00:10.43>on <00:10.71>your <00:11.02>feet
[00:12.89]<00:12.89>You <00:13.06>better <00:13.23>be <00:13.43>at <00:13.61>the <00:13.91>dance <00:14.21>down <00:14.38>on <00:14.56>14th <00:14.72>street  <00:15.58>you <00:15.72>hear
[00:18.11]<00:18.11>Yeah <00:20.78>there''s <00:21.01>gonna <00:21.32>be <00:21.57>a <00:21.84>showdown
[00:25.81]<00:25.81>There''s <00:28.41>gonna <00:28.82>be <00:29.06>a <00:29.33>showdown
[00:33.30]<00:33.30>There''s <00:35.95>gonna <00:36.36>be <00:36.63>a <00:36.87>showdown
[00:40.68]<00:40.68>Yeah  <00:41.49>yeah  <00:43.39>there''s <00:43.62>gonna <00:43.81>be <00:44.05>a <00:44.26>showdown
[00:48.95]<00:48.95>I''ve <00:49.17>got <00:49.50>the <00:49.76>ten <00:50.33>notches <00:52.39>right <00:52.59>on <00:52.81>my <00:53.14>shoes
[00:55.75]<00:55.75>''Cause <00:55.94>when <00:56.15>it <00:56.42>comes <00:56.85>to <00:57.17>dancin''  <00:59.53>I <00:59.85>just <01:00.20>can''t <01:00.68>lose
[01:03.64]<01:03.64>They <01:03.83>call <01:04.03>me <01:04.24>the <01:04.49>top <01:04.84>cat  <01:06.96>right <01:07.19>in <01:07.43>this <01:07.71>man''s <01:08.16>town
[01:10.70]<01:10.70>I <01:10.91>just <01:11.12>want <01:11.31>you <01:11.58>to <01:11.79>meet <01:12.07>me  <01:12.99>baby <01:13.19>when <01:13.42>the <01:13.70>sun <01:14.81>goes <01:15.38>down <01:16.59>that''s <01:16.91>when
[01:17.35]<01:17.35>There''s <01:20.24>gonna <01:20.53>be <01:20.86>a <01:21.10>showdown
[01:24.96]<01:24.96>There''s <01:27.73>gonna <01:28.03>be <01:28.30>a <01:28.56>showdown
[01:47.60]<01:47.60>All <01:47.75>the <01:48.07>girls <01:48.42>been <01:48.83>losin'' <01:51.77>faith <01:52.07>in <01:52.37>me
[01:54.85]<01:54.85>If <01:54.94>don''t <01:55.29>seem <01:55.75>like <01:56.17>top <01:56.60>cat''s <01:57.01>great
[01:58.66]<01:58.66>As <01:58.89>he <01:59.07>once <01:59.31>used <01:59.52>to <01:59.86>be
[02:02.46]<02:02.46>I <02:02.65>know <02:02.84>I''m <02:03.15>good
[02:04.36]<02:04.36>So <02:04.57>you <02:05.70>you <02:05.93>to <02:06.15>better <02:06.57>be <02:06.82>better
[02:09.38]<02:09.38>When <02:09.59>you <02:09.89>get <02:10.17>yourself <02:10.63>out <02:10.84>on <02:11.10>that <02:11.44>floor
[02:12.10]<02:12.10>You <02:12.31>better <02:12.52>have <02:12.93>your <02:13.33>steps <02:13.91>together
[02:16.16]<02:16.16>There''s <02:18.65>gonna <02:19.09>be <02:19.40>a <02:19.64>showdown
[02:23.45]<02:23.45>There''s <02:26.08>gonna <02:26.36>be <02:26.60>a <02:26.85>showdown
[02:30.85]<02:30.85>I <02:30.97>get <02:31.26>reputation <02:32.09>has <02:32.31>been <02:34.45>one <02:34.64>of <02:34.81>the <02:35.16>fastest <02:35.48>men <02:35.97>alive
[02:38.22]<02:38.22>So <02:38.41>I''m <02:38.57>gonna <02:38.78>see <02:39.00>how <02:39.28>good <02:39.51>you <02:39.84>are <02:41.67>when <02:41.88>I <02:42.31>count <02:42.67>to <02:42.98>five
[02:45.94]<02:45.94>Oh <02:46.20>man  <02:46.43>you <02:46.63>better <02:47.03>step
[02:49.73]<02:49.73>You <02:50.02>can <02:50.21>do <02:50.38>better <02:50.63>than <02:50.78>that
[02:53.05]<02:53.05>Come <02:53.24>on <02:53.71>baby  <02:54.03>you <02:54.23>better <02:54.47>step <02:54.70>aside
[02:55.97]<02:55.97>And <02:56.17>let <02:56.35>me <02:56.51>get <02:56.76>out <02:56.97>here <02:57.15>and <02:57.36>do <02:57.57>my <02:57.91>jive
[02:59.21]<02:59.21>There''s <03:01.88>gonna <03:02.30>be <03:02.51>a <03:02.77>showdown
[03:06.63]<03:06.63>There''s <03:09.22>gonna <03:09.50>be <03:09.73>a <03:10.01>showdown
[03:13.81]<03:13.81>There''s <03:16.50>gonna <03:16.71>be <03:16.96>a <03:17.31>showdown
[03:23.79]<03:23.79>Gonna <03:24.14>be <03:24.35>a <03:24.57>showdown
[03:26.84]<03:26.84>Showdown <03:27.75>now
[03:28.49]<03:28.49>(<03:29.03>There''s <03:29.07>Gonna <03:29.11>Be <03:29.15>A) <03:29.18>Showdown <03:29.22>- <03:29.26>The <03:29.29>New <03:29.32>York <03:29.37>Dolls
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (22882, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>''Til<00:00.04> <00:00.08>It''s<00:00.12> <00:00.16>Over<00:00.20> <00:00.23>(<00:00.27>直<00:00.31>到<00:00.35>一<00:00.39>切<00:00.43>结<00:00.47>束<00:00.51>)<00:00.55> <00:00.58>(<00:00.62>Explicit<00:00.66>)<00:00.70> <00:00.74>(<00:00.78>HomePod<00:00.82>宣<00:00.86>传<00:00.90>片<00:00.94>背<00:00.97>景<00:01.01>音<00:01.05>乐<00:01.09>)<00:01.13> <00:01.17>-<00:01.21> <00:01.25>Anderson<00:01.29> <00:01.33>.Paak
[00:01.40]<00:01.40>We <00:01.54>stayed <00:01.88>up <00:02.15>all <00:02.40>night <00:02.67>watching <00:03.10>the <00:03.26>comedy <00:03.83>show
[00:05.60]<00:05.60>That <00:05.83>aged <00:06.11>whiskey <00:06.77>and <00:06.97>hydro
[00:08.09]<00:08.09>Good <00:08.27>lord <00:09.18>what <00:09.34>a <00:09.49>nice <00:09.77>conversation
[00:11.30]<00:11.30>I''m <00:11.52>too <00:11.83>floored <00:12.44>to <00:12.61>get <00:12.77>anywhere <00:13.18>safely
[00:15.04]<00:15.04>You <00:15.20>know <00:15.36>I <00:15.50>talk <00:15.67>about <00:15.94>you <00:16.14>highly
[00:18.95]<00:18.95>I''m <00:19.14>fascinated <00:20.04>for <00:20.22>the <00:20.39>time <00:20.84>being
[00:21.61]<00:21.61>We <00:21.76>can <00:21.90>laugh <00:22.33>until <00:22.56>the <00:22.74>morning
[00:25.68]<00:25.68>Or <00:25.86>we <00:26.02>can <00:26.18>dance <00:26.62>in <00:26.77>the <00:26.94>hallway
[00:28.31]<00:28.31>Only <00:28.57>one <00:28.76>more <00:29.09>night <00:29.38>in <00:29.56>Los <00:30.00>Angeles
[00:32.56]<00:32.56>I <00:32.72>really <00:32.87>thought <00:33.06>I <00:33.21>could <00:33.37>handle <00:33.90>it
[00:34.84]<00:34.84>But <00:35.03>the <00:35.20>funny <00:35.58>thing <00:36.18>is <00:36.55>I <00:36.74>was <00:36.91>holding <00:37.30>back <00:37.71>tears
[00:38.30]<00:38.30>I <00:38.49>didn''t <00:38.63>think <00:38.80>this <00:39.03>day <00:39.27>would <00:39.47>happen
[00:41.55]<00:41.55>I <00:41.75>give <00:41.95>all <00:42.19>this <00:42.38>up <00:42.72>for <00:42.98>a <00:43.16>chance <00:43.54>at <00:43.72>it
[00:45.85]<00:45.85>You <00:46.02>would <00:46.18>have <00:46.35>thought <00:46.51>I''d <00:46.67>be <00:46.83>the <00:47.00>man <00:47.26>for <00:47.42>this
[00:48.36]<00:48.36>But <00:48.51>the <00:48.68>funny <00:48.91>thing <00:49.30>is <00:49.75>we <00:49.94>can <00:50.18>never <00:50.57>stay <00:51.00>here
[00:51.72]<00:51.72>I <00:51.89>didn''t <00:52.08>think <00:52.25>this <00:52.44>day <00:52.75>could <00:53.08>happen
[00:54.24]<00:54.24>I''ma <00:54.39>ride <00:54.55>it <00:54.70>''til <00:54.85>it''s <00:55.04>over
[01:01.04]<01:01.04>I''ma <01:01.20>ride <01:01.35>it <01:01.50>''til <01:01.65>it''s <01:01.82>over
[01:07.71]<01:07.71>I''ma <01:07.88>ride <01:08.04>it <01:08.16>''til <01:08.29>it''s <01:08.47>over
[01:14.45]<01:14.45>I''ma <01:14.62>ride <01:14.77>it <01:14.89>''til <01:15.02>it''s <01:15.18>over
[01:21.10]<01:21.10>I''ma <01:21.25>ride <01:23.27>I''ma <01:23.42>ride
[01:26.60]<01:26.60>Ride
[01:27.92]<01:27.92>I''ma <01:28.08>ride <01:28.25>it <01:28.40>''til <01:28.53>it''s <01:28.66>over
[01:29.91]<01:29.91>I''ma <01:30.06>ride <01:31.36>I''ma <01:31.49>ride
[01:33.28]<01:33.28>I''ma <01:33.42>ride <01:34.96>I''ma <01:35.11>ride
[01:41.80]<01:41.80>And <01:41.97>don''t <01:42.11>all <01:42.32>this <01:42.50>new <01:42.76>music <01:43.18>sound <01:43.66>the <01:43.85>same
[01:45.96]<01:45.96>Yeah <01:46.14>we <01:46.29>must <01:46.45>be <01:46.60>getting <01:46.85>old <01:47.23>and <01:47.41>grey
[01:48.35]<01:48.35>We <01:48.57>left <01:48.74>early <01:48.98>girl <01:49.24>that <01:49.43>bed <01:49.62>was <01:49.87>sh*tty <01:50.30>anyway
[01:51.48]<01:51.48>We <01:51.65>went <01:51.83>home <01:52.10>and <01:52.28>left <01:52.49>our <01:52.70>clothes <01:53.12>up <01:53.30>in <01:53.49>the <01:53.67>living <01:53.94>space <01:54.53>aye
[01:54.98]<01:54.98>Would <01:55.15>you <01:55.32>stay <01:55.81>if <01:55.97>your <01:56.15>heart <01:56.50>had <01:56.67>the <01:56.91>power
[01:58.29]<01:58.29>Would <01:58.47>you <01:58.66>run <01:58.91>and <01:59.06>find <01:59.29>another <01:59.69>life <02:00.01>to <02:00.21>imitate
[02:01.65]<02:01.65>It''s <02:01.84>important <02:02.35>that <02:02.54>we <02:02.77>make <02:03.02>the <02:03.19>best <02:03.43>of <02:03.59>short <02:03.90>time
[02:05.17]<02:05.17>You <02:05.33>could <02:05.49>never <02:05.68>be <02:05.93>my <02:06.13>one <02:06.36>and <02:06.57>only <02:06.76>anyway
[02:07.94]<02:07.94>Say <02:08.30>can''t <02:08.49>a <02:08.75>young <02:09.03>man <02:09.87>dream
[02:11.56]<02:11.56>Can''t <02:11.73>we <02:11.94>all <02:12.27>live <02:12.57>the <02:12.74>life <02:13.13>on <02:13.43>a <02:13.61>widescreen
[02:15.01]<02:15.01>What''s <02:15.17>the <02:15.34>point <02:16.61>yeah
[02:18.05]<02:18.05>Yeah <02:18.26>we <02:18.40>had <02:18.56>fun <02:18.82>if <02:19.00>only <02:19.18>for <02:19.40>the <02:19.68>time <02:20.19>being
[02:20.73]<02:20.73>I''ma <02:20.89>ride <02:21.04>it <02:21.19>''til <02:21.35>it''s <02:21.56>over
[02:27.71]<02:27.71>I''ma <02:27.86>ride <02:28.01>it <02:28.16>''til <02:28.31>it''s <02:28.48>over
[02:34.28]<02:34.28>I''ma <02:34.43>ride <02:34.58>it <02:34.73>''til <02:34.87>it''s <02:35.03>over
[02:41.01]<02:41.01>I''ma <02:41.18>ride <02:41.32>it <02:41.47>''til <02:41.59>it''s <02:41.73>over
[02:47.57]<02:47.57>I''ma <02:47.72>ride <02:49.96>I''ma <02:50.10>ride
[02:53.33]<02:53.33>Ride
[02:54.33]<02:54.33>I''ma <02:54.48>ride <02:54.63>it <02:54.79>''til <02:54.94>it''s <02:55.10>over
[02:56.46]<02:56.46>I''ma <02:56.62>ride <02:58.06>I''ma <02:58.20>ride
[03:00.01]<03:00.01>I''ma <03:00.13>ride <03:01.74>I''ma <03:01.89>ride
[03:04.65]<03:04.65>I''ma <03:04.88>ride <03:07.46>I''ma <03:07.62>ride
[03:10.67]<03:10.67>I''ma <03:10.80>ride
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (8605, 'lrc', 'line', 'local_lrc', '[00:28.42]<00:28.42>She<00:28.43> <00:28.72>makes<00:28.75> <00:29.08>my<00:29.10> <00:29.27>shoulders<00:29.33> <00:30.17>deflate
[00:33.02]<00:33.02>Won''t<00:33.53> <00:33.56>ever<00:33.59> <00:34.75>see<00:34.79> <00:35.21>me<00:35.23> <00:35.43>standing<00:35.47> <00:36.13>up<00:36.15> <00:36.47>straight
[00:38.24]<00:38.24>Strawberry<00:38.29> <00:39.04>split<00:39.65> <00:40.00>personality
[00:41.98]<00:41.98>She''s<00:41.99> <00:42.55>got<00:42.57> <00:43.03>so<00:43.06> <00:43.32>many<00:43.36> <00:43.97>years<00:44.02> <00:44.59>hanging<00:44.62> <00:45.12>over<00:45.15> <00:45.60>me
[00:47.34]<00:47.34>I<00:47.34> <00:47.77>would<00:47.80> <00:48.15>not<00:48.18> <00:48.74>wait
[00:52.98]<00:52.98>I<00:52.98> <00:53.27>would<00:53.30> <00:53.51>not<00:53.53> <00:54.06>wait
[00:56.24]<00:56.24>If<00:56.25> <00:56.72>I<00:56.73> <00:57.07>were<00:57.60> <00:57.64>you
[00:59.94]<00:59.94>I<00:59.94> <01:00.37>would<01:00.41> <01:00.95>have<01:00.99> <01:01.80>chosen<01:01.85> <01:02.44>her<01:02.47> <01:02.92>too
[01:13.39]<01:13.39>You''ll<01:13.42> <01:13.73>end<01:13.75> <01:14.03>up<01:14.04> <01:14.21>calling<01:14.25> <01:15.04>her<01:15.07> <01:15.61>babe
[01:17.79]<01:17.79>She''s<01:17.80> <01:18.27>scorching<01:19.19> <01:19.73>hot<01:19.75> <01:20.20>enough<01:20.24> <01:20.67>to<01:20.68> <01:20.91>hit<01:20.92> <01:21.35>save
[01:23.15]<01:23.15>Never<01:23.17> <01:23.64>forgave<01:24.59> <01:24.82>what<01:24.84> <01:25.13>she<01:25.16> <01:25.39>did<01:25.41> <01:25.72>to<01:25.74> <01:25.94>me
[01:26.84]<01:26.84>All<01:27.09> <01:27.34>dolled<01:27.35> <01:27.96>up,<01:27.99> <01:28.81>actin''<01:28.84> <01:29.28>like<01:29.30> <01:29.68>a<01:29.69> <01:29.87>bitch<01:29.90> <01:30.26>would<01:30.29> <01:30.47>be
[01:32.11]<01:32.11>Couldn''t<01:32.16> <01:33.00>behave
[01:36.83]<01:36.83>She<01:36.85> <01:37.16>just<01:37.18> <01:37.65>couldn''t<01:37.71> <01:38.30>behave
[01:40.96]<01:40.96>But<01:40.98> <01:41.24>if<01:41.25> <01:41.67>I<01:41.68> <01:42.03>were<01:42.07> <01:42.52>you
[01:44.85]<01:44.85>I<01:44.85> <01:45.28>would<01:45.31> <01:45.84>have<01:45.91> <01:46.70>chosen<01:46.73> <01:47.35>her<01:47.37> <01:47.79>too
[01:50.95]<01:50.95>Do<01:50.95> <01:51.08>you<01:51.10> <01:51.22>think<01:51.24> <01:51.77>she<01:51.78> <01:52.06>feels<01:52.09> <01:52.68>like<01:52.70> <01:53.16>she''s<01:53.20> <01:53.51>being<01:53.56> <01:54.28>watched?
[01:56.80]<01:56.80>Maybe<01:56.84> <01:57.63>not
[02:00.06]<02:00.06>But<02:00.06> <02:00.18>baby,<02:00.20> <02:01.05>when<02:01.09> <02:01.90>the<02:01.92> <02:02.04>music<02:02.09> <02:03.12>stops<02:04.37> <02:04.66>all<02:04.67> <02:05.21>you<02:05.26> <02:05.49>got
[02:07.70]<02:07.70>Is<02:07.73> <02:07.93>a<02:07.94> <02:08.03>risky<02:08.07> <02:08.76>photo
[02:10.29]<02:10.29>Bathroom<02:10.35> <02:11.38>mirror<02:11.43> <02:12.21>moment,<02:13.12> <02:15.27>bozo
[02:15.47]<02:15.47>Smoke<02:15.49> <02:16.14>show,<02:16.91> <02:17.46>she''s<02:17.51> <02:18.16>fine
[02:20.38]<02:20.38>Perfect<02:20.43> <02:21.17>kissing<02:21.24> <02:22.27>height
[02:24.27]<02:24.27>Yeah,<02:24.31> <02:24.82>she<02:24.86> <02:25.36>suits<02:25.40> <02:26.07>you<02:26.09> <02:26.27>alright
[02:29.02]<02:29.02>But<02:29.04> <02:29.42>I<02:29.43> <02:31.22>won''t<02:31.24> <02:31.50>stop<02:31.53> <02:32.20>until<02:32.23> <02:32.71>you''re<02:32.84> <02:33.05>mine
[02:37.93]<02:37.93>No,<02:37.94> <02:38.27>I<02:38.28> <02:38.45>won''t<02:38.48> <02:40.38>stop<02:40.42> <02:41.12>until<02:41.56> <02:42.06>you''re<02:42.09> <02:42.17>mine
[02:47.07]<02:47.07>But<02:47.10> <02:47.46>I<02:47.46> <02:47.60>won''t<02:47.63> <02:49.45>stop<02:50.09> <02:50.32>until<02:50.34> <02:50.70>you''re<02:50.73> <02:50.94>mine
[02:55.96]<02:55.96>No,<02:55.97> <02:56.20>I<02:56.20> <02:56.38>won''t<02:56.41> <02:58.38>stop<02:58.41> <02:59.11>until<02:59.52> <03:05.71>you''re<03:05.77> <03:05.86>mine
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (21393, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.07>Do<00:00.21> <00:00.28>You<00:00.48> <00:00.55>Remember<00:01.11>)<00:01.18> <00:01.25>The<00:01.46> <00:01.52>Saturday<00:02.08> <00:02.15>Gigs<00:02.43>?<00:02.50> <00:02.56>(<00:02.63>Alternate<00:03.26> <00:03.33>Version<00:03.81>)<00:03.88> <00:03.95>-<00:04.02> <00:04.09>Mott<00:04.37> <00:04.44>The<00:04.64> <00:04.71>Hoople
[00:05.13]<00:05.13>Lyrics<00:05.86> <00:06.59>by<00:07.33>：<00:08.06>I.<00:08.79> <00:09.52>Hunter
[00:10.26]<00:10.26>Composed<00:10.99> <00:11.72>by<00:12.46>：<00:13.19>I.<00:13.92> <00:14.65>Hunter
[00:15.40]<00:15.40>''<00:15.71>69 <00:17.43>was <00:17.68>cheap-<00:18.01>o <00:18.26>wine
[00:19.65]<00:19.65>Have <00:19.86>a <00:20.17>good <00:20.63>time <00:21.63>what''s <00:21.99>your <00:22.28>sign
[00:23.57]<00:23.57>Float <00:23.91>up <00:24.41>to <00:24.90>the <00:25.21>Roundhouse
[00:27.05]<00:27.05>On <00:27.24>a <00:27.48>Sunday <00:28.16>afternoon
[00:31.19]<00:31.19>In <00:31.47>''<00:31.47>70 <00:33.06>we <00:33.29>all <00:33.62>agreed
[00:35.09]<00:35.09>A <00:35.29>King''s <00:35.67>Road <00:36.15>flat <00:36.69>was <00:36.91>the <00:37.19>place <00:37.61>to <00:37.85>be
[00:38.87]<00:38.87>''Cause <00:39.14>Chelsea <00:39.80>girls <00:40.67>are <00:40.91>the <00:41.17>best <00:41.53>in <00:41.74>the <00:42.03>world
[00:42.55]<00:42.55>For <00:42.73>company
[00:46.67]<00:46.67>In <00:46.86>''<00:46.86>71 <00:48.21>all <00:48.46>the <00:48.70>people <00:49.40>come
[00:50.63]<00:50.63>Bust <00:50.90>a <00:51.17>few <00:51.54>seats <00:52.00>but <00:52.23>it''s <00:52.52>just <00:52.91>in <00:53.37>fun
[00:54.40]<00:54.40>Take <00:54.80>the <00:55.08>Mick <00:55.83>out <00:56.04>of <00:56.22>Top <00:56.45>of <00:56.65>the <00:56.92>Pops
[00:58.12]<00:58.12>We <00:58.35>play <00:58.94>better <00:59.43>than <00:59.90>they <01:00.47>do
[01:00.82]<01:00.82>Yeah <01:01.26>yeah <01:01.48>yeah
[01:01.78]<01:01.78>In <01:01.97>''<01:01.97>72 <01:03.43>was <01:03.65>born <01:04.06>to <01:04.32>lose
[01:05.30]<01:05.30>We <01:05.55>slipped <01:05.98>down <01:06.42>snakes <01:06.90>into <01:07.33>yesterday''s <01:08.09>news
[01:08.90]<01:08.90>I <01:09.07>was <01:09.28>ready <01:09.64>to <01:09.92>quit
[01:11.01]<01:11.01>But <01:11.18>then <01:11.27>we <01:11.48>went <01:11.66>to <01:11.97>Croydon
[01:12.74]<01:12.74>Do <01:12.95>you <01:13.39>remember <01:14.60>the <01:14.80>Saturday <01:15.42>gigs
[01:16.64]<01:16.64>We <01:17.13>do <01:18.56>we <01:19.03>do
[01:20.12]<01:20.12>Do <01:20.39>you <01:20.78>remember <01:21.98>the <01:22.24>Saturday <01:22.85>gigs
[01:24.09]<01:24.09>We <01:24.57>do <01:25.97>we <01:26.46>do
[01:27.67]<01:27.67>The <01:27.90>tickets <01:28.46>for <01:29.17>the <01:29.47>fantasy
[01:31.35]<01:31.35>Were <01:31.54>twelve <01:32.00>and <01:32.25>six <01:32.88>a <01:33.08>time
[01:33.96]<01:33.96>A <01:34.19>fairy <01:34.94>tale
[01:37.49]<01:37.49>On <01:38.07>sale
[01:50.24]<01:50.24>Oh <01:50.65>''<01:50.65>73 <01:52.20>was <01:52.37>a <01:52.59>jamboree
[01:54.40]<01:54.40>We <01:54.66>were <01:54.93>the <01:55.16>dudes <01:55.91>and <01:56.10>the <01:56.30>dudes <01:56.62>were <01:56.79>we
[01:56.94]<01:56.94>Oh <01:57.11>oh <01:57.29>oh <01:57.60>oh <01:57.84>oh
[01:58.05]<01:58.05>Did <01:58.23>you <01:58.57>see <01:58.85>the <01:59.11>suits <01:59.57>and <01:59.79>the <02:00.01>platform <02:00.99>boots
[02:01.40]<02:01.40>Oh <02:01.87>dear <02:02.33>oh <02:03.27>oh <02:03.80>boy <02:04.30>oh <02:04.68>boyo
[02:05.44]<02:05.44>In <02:05.67>''<02:05.67>74 <02:07.01>on <02:07.24>the <02:07.46>Broadway <02:08.37>tour
[02:09.12]<02:09.12>We <02:09.33>didn''t <02:09.77>much <02:10.21>like <02:10.70>dressing <02:11.14>up <02:11.57>no <02:12.10>more
[02:12.89]<02:12.89>Don''t <02:13.07>wanna <02:13.48>be <02:13.96>hip
[02:14.70]<02:14.70>But <02:14.92>thanks <02:15.35>for <02:15.54>a <02:15.78>great <02:16.20>trip
[02:16.52]<02:16.52>Do <02:16.73>you <02:17.12>remember <02:18.33>the <02:18.55>Saturday <02:19.13>gigs
[02:20.47]<02:20.47>We <02:20.88>do <02:22.24>we <02:22.72>do
[02:23.92]<02:23.92>Do <02:24.16>you <02:24.50>remember <02:25.76>the <02:25.95>Saturday <02:26.61>gigs
[02:27.83]<02:27.83>We <02:28.25>do <02:29.65>we <02:30.11>do
[02:31.38]<02:31.38>But <02:31.57>now <02:31.95>the <02:32.18>kids <02:32.84>pay <02:33.13>a <02:33.36>couple <02:33.81>of <02:34.03>quid
[02:34.86]<02:34.86>''Cause <02:35.05>they <02:35.26>need <02:35.67>it <02:35.92>just <02:36.49>the <02:36.73>same
[02:37.95]<02:37.95>It''s <02:38.11>all <02:38.59>a <02:38.82>game
[02:41.25]<02:41.25>A <02:41.42>grown-<02:41.69>up <02:41.90>game
[02:58.39]<02:58.39>But <02:58.55>you <02:58.96>got <02:59.20>off <02:59.76>on <03:00.15>those <03:00.38>Saturday <03:00.92>gigs
[03:01.97]<03:01.97>And <03:02.22>we <03:02.63>did <03:03.96>we <03:04.45>did
[03:05.68]<03:05.68>''Cause <03:05.91>you <03:06.28>got <03:06.52>off <03:07.13>on <03:07.33>those <03:07.65>Saturday <03:08.34>gigs
[03:09.29]<03:09.29>And <03:09.47>we <03:09.94>did <03:11.38>we <03:11.82>did
[03:13.05]<03:13.05>And <03:13.25>we <03:13.61>got <03:13.84>off <03:14.40>on <03:14.64>those <03:14.93>Saturday <03:15.65>gigs
[03:16.73]<03:16.73>And <03:16.95>you <03:17.36>did <03:18.78>you <03:19.20>did
[03:20.39]<03:20.39>And <03:20.61>we <03:20.97>got <03:21.22>off <03:21.77>on <03:21.97>those <03:22.36>Saturday <03:23.03>gigs
[03:24.07]<03:24.07>''Cause <03:24.27>you <03:24.69>did <03:26.11>you <03:26.51>did
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (11664, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.82>Don''t<00:01.63> <00:02.45>Fear<00:03.27>)<00:04.08> <00:04.90>The<00:05.72> <00:06.54>Reaper<00:07.35> <00:08.17>-<00:08.99> <00:09.80>Blue<00:10.62> <00:11.44>Oyster<00:12.26> <00:13.07>Cult
[00:13.90]<00:13.90>All <00:15.02>our <00:15.21>times <00:16.14>have <00:16.33>come
[00:20.76]<00:20.76>Here <00:21.57>but <00:21.95>now <00:23.01>they''re <00:23.63>gone
[00:27.31]<00:27.31>Seasons <00:27.88>don''t <00:28.06>fear <00:28.69>the <00:28.87>reaper
[00:30.31]<00:30.31>Nor <00:30.50>do <00:30.68>the <00:30.87>wind <00:31.50>the <00:31.74>sun <00:31.99>or <00:32.24>the <00:32.37>rain
[00:33.55]<00:33.55>(We <00:33.68>can <00:33.87>be <00:34.05>like <00:34.18>they <00:34.55>are)
[00:35.18]<00:35.18>Come <00:35.36>on <00:35.55>baby
[00:36.55]<00:36.55>(Don''t <00:36.74>fear <00:36.92>the <00:37.24>Reaper)
[00:38.17]<00:38.17>Baby <00:38.36>take <00:38.55>my <00:38.98>hand
[00:39.79]<00:39.79>(Don''t <00:39.98>fear <00:40.17>the <00:40.67>Reaper)
[00:41.54]<00:41.54>We''ll <00:41.73>be <00:41.92>able <00:42.10>to <00:42.29>fly
[00:43.16]<00:43.16>(Don''t <00:43.35>fear <00:43.54>the <00:44.10>Reaper)
[00:44.91]<00:44.91>Baby <00:45.16>I''m <00:45.35>your <00:45.72>man
[00:49.65]<00:49.65>La <00:50.53>la <00:51.15>la <00:51.71>la <00:52.65>la
[00:56.27]<00:56.27>La <00:57.27>la <00:57.58>la <00:58.39>la <00:59.08>la
[01:23.54]<01:23.54>Valentine <01:25.97>is <01:26.53>done
[01:30.34]<01:30.34>Here <01:31.27>but <01:31.59>now <01:32.65>they''re <01:33.40>gone
[01:38.01]<01:38.01>Romeo <01:38.51>and <01:38.76>Juliet
[01:40.26]<01:40.26>Are <01:40.88>together <01:41.38>in <01:41.82>eternity
[01:43.19]<01:43.19>(Romeo <01:43.51>and <01:43.88>Juliet)
[01:44.32]<01:44.32>40000 <01:44.57>men <01:45.31>and <01:45.56>women <01:45.94>everyday
[01:46.38]<01:46.38>(Like <01:46.56>Romeo <01:46.81>and <01:47.00>Juliet)
[01:47.56]<01:47.56>40000 <01:47.81>men <01:48.25>and <01:48.68>women <01:48.93>everyday
[01:49.81]<01:49.81>(Redefine <01:50.24>happiness)
[01:51.12]<01:51.12>Another <01:51.31>40000 <01:51.62>coming <01:52.30>everyday
[01:52.99]<01:52.99>(We <01:53.18>can <01:53.36>be <01:53.55>like <01:53.86>they <01:54.11>are)
[01:55.11]<01:55.11>Come <01:55.30>on <01:55.80>baby
[01:56.42]<01:56.42>(Don''t <01:56.61>fear <01:56.98>the <01:57.30>Reaper)
[01:58.11]<01:58.11>Baby <01:58.36>take <01:58.54>my <01:58.86>hand
[01:59.54]<01:59.54>(Don''t <01:59.85>fear <02:00.60>the <02:00.79>Reaper)
[02:01.48]<02:01.48>We''ll <02:01.73>be <02:01.98>able <02:02.23>to <02:02.47>fly
[02:03.16]<02:03.16>(Don''t <02:03.41>fear <02:03.79>the <02:04.03>Reaper)
[02:04.91]<02:04.91>Baby <02:05.10>I''m <02:05.41>your <02:05.78>man
[02:09.59]<02:09.59>La <02:10.52>la <02:10.90>la <02:11.77>la <02:12.40>la
[02:16.28]<02:16.28>La <02:17.28>la <02:17.65>la <02:18.46>la <02:19.21>la
[03:36.96]<03:36.96>Love <03:37.96>of <03:38.15>two <03:39.21>is <03:39.71>one
[03:43.70]<03:43.70>Here <03:44.64>but <03:44.95>now <03:46.01>they''re <03:46.76>gone
[03:50.44]<03:50.44>Came <03:50.76>the <03:51.07>last <03:51.32>night <03:51.88>of <03:52.13>sadness
[03:53.63]<03:53.63>And <03:53.81>it <03:54.00>was <03:54.25>clear <03:54.94>she <03:55.19>couldn''t <03:55.37>go <03:55.56>on
[03:57.31]<03:57.31>Then <03:57.49>the <03:57.68>door <03:57.99>was <03:58.18>open <03:58.56>and <03:58.74>the <03:58.93>wind <03:59.24>appeared
[04:00.55]<04:00.55>The <04:00.80>candles <04:00.99>blew <04:01.55>and <04:01.86>then <04:02.36>disappeared
[04:04.17]<04:04.17>The <04:04.42>curtains <04:04.67>flew <04:05.23>and <04:05.42>then <04:05.79>he <04:05.98>appeared
[04:06.67]<04:06.67>(Saying <04:06.85>don''t <04:07.04>be <04:07.23>afraid)
[04:08.54]<04:08.54>Come <04:08.79>on <04:09.04>baby
[04:09.85]<04:09.85>(And <04:10.04>she <04:10.22>had <04:10.35>no <04:10.66>fear)
[04:11.78]<04:11.78>And <04:12.16>she <04:12.35>ran <04:12.72>to <04:13.09>him
[04:13.34]<04:13.34>(Then <04:13.53>they <04:13.72>started <04:13.91>to <04:14.22>fly)
[04:14.78]<04:14.78>They <04:14.97>looked <04:15.40>backward <04:15.84>and <04:16.15>said <04:16.40>goodbye
[04:16.96]<04:16.96>(She <04:17.15>had <04:17.34>become <04:17.52>like <04:17.65>they <04:17.96>are)
[04:18.21]<04:18.21>She <04:18.40>had <04:18.59>taken <04:18.84>his <04:19.08>hand
[04:19.58]<04:19.58>(She <04:19.77>had <04:19.96>become <04:20.15>like <04:20.33>they <04:20.64>are)
[04:21.89]<04:21.89>Come <04:22.14>on <04:22.52>baby
[04:23.20]<04:23.20>(Don''t <04:23.52>fear <04:23.70>the <04:24.14>reaper)
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (15709, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.68>Everything<00:01.35> <00:02.02>I<00:02.70> <00:03.38>Do<00:04.05>)<00:04.72> <00:05.40>I<00:06.08> <00:06.75>Do<00:07.42> <00:08.10>It<00:08.78> <00:09.45>For<00:10.12> <00:10.80>You<00:11.47> <00:12.15>-<00:12.82> <00:13.50>Bryan<00:14.18> <00:14.85>Adams
[00:15.53]<00:15.53>Look <00:16.13>into <00:16.38>my <00:16.68>eyes <00:19.63>you <00:19.89>will <00:20.29>see
[00:22.39]<00:22.39>What <00:23.19>you <00:23.89>mean <00:24.99>to <00:25.64>me
[00:28.39]<00:28.39>Search <00:29.29>your <00:29.59>heart <00:32.19>search <00:32.89>your <00:33.19>soul
[00:36.39]<00:36.39>And <00:37.09>when <00:37.54>you <00:37.99>find <00:38.49>me <00:38.89>there <00:39.59>you''ll <00:40.54>search <00:41.95>no <00:42.30>more
[00:43.59]<00:43.59>Don''t <00:44.20>tell <00:44.99>me <00:45.95>it''s <00:46.55>not <00:46.74>worth <00:46.99>trying <00:47.24>for
[00:50.80]<00:50.80>You <00:51.35>can''t <00:51.60>tell <00:52.30>me <00:53.20>it''s <00:53.55>not <00:53.85>worth <00:54.15>dying <00:54.95>for
[00:57.70]<00:57.70>You <00:58.30>know <00:58.60>it''s <00:58.90>true
[01:01.50]<01:01.50>Everything <01:02.10>I <01:02.40>do <01:05.30>I <01:05.60>do <01:05.85>it <01:06.15>for <01:06.50>you
[01:14.11]<01:14.11>Look <01:14.71>into <01:15.05>your <01:15.76>heart <01:17.00>you <01:18.20>will <01:18.80>find
[01:20.55]<01:20.55>There''s <01:21.20>nothing <01:22.41>there <01:23.26>to <01:24.16>hide
[01:26.86]<01:26.86>Take <01:27.41>me <01:27.70>as <01:27.95>I <01:28.41>am <01:30.51>take <01:30.96>my <01:31.56>life
[01:34.06]<01:34.06>I <01:35.21>would <01:35.51>give <01:36.01>it <01:36.46>all <01:37.91>I <01:38.66>would <01:38.91>sacrify
[01:42.16]<01:42.16>Don''t <01:42.76>tell <01:43.46>me <01:44.41>it''s <01:44.71>not <01:45.01>worth <01:45.31>fighting <01:46.06>for
[01:49.42]<01:49.42>I <01:50.07>can''t <01:50.32>help <01:50.77>it <01:51.67>there''s <01:52.12>nothing <01:52.62>I <01:53.27>want <01:53.67>more
[01:56.17]<01:56.17>You <01:56.72>know <01:57.07>it''s <01:57.37>true
[01:59.82]<01:59.82>Everything <02:00.37>I <02:00.67>do <02:01.07>I <02:03.57>do <02:03.87>it <02:04.27>for <02:04.57>you
[02:10.92]<02:10.92>There''s <02:11.77>no <02:12.72>love <02:13.77>like <02:14.97>your <02:15.57>love
[02:18.17]<02:18.17>And <02:18.77>no <02:19.12>other <02:21.87>could <02:22.27>give <02:22.67>more <02:23.62>love
[02:25.42]<02:25.42>There''s <02:26.02>no <02:27.07>where <02:29.27>unless <02:30.02>you''re <02:30.97>there
[02:32.72]<02:32.72>All <02:33.37>the <02:33.82>time <02:36.57>all <02:37.32>the <02:37.52>way
[03:12.08]<03:12.08>Oh <03:13.33>you <03:13.58>can''t <03:13.88>tell <03:14.78>me <03:15.83>it''s <03:16.13>not <03:16.43>worth <03:16.73>trying <03:17.53>for
[03:20.28]<03:20.28>I <03:20.88>can''t <03:21.23>help <03:22.08>it <03:22.83>there''s <03:23.23>nothing <03:23.93>I <03:24.38>want <03:24.83>more
[03:27.38]<03:27.38>Yeah <03:28.28>I <03:28.58>would <03:28.88>fight <03:29.63>for <03:30.28>you <03:31.03>I''d <03:31.48>lie <03:32.28>for <03:33.33>you
[03:33.74]<03:33.74>Walk <03:35.38>the <03:35.84>wire <03:36.19>for <03:36.94>you <03:38.29>yeah <03:38.94>I''d <03:39.49>die <03:40.34>for <03:40.69>you
[03:45.94]<03:45.94>You <03:46.69>know <03:46.99>it''s <03:47.29>true
[03:49.59]<03:49.59>Everything <03:50.24>i <03:50.54>do <03:53.64>oh  <03:57.79>I <03:58.29>do <03:58.59>it <03:58.90>for <04:00.65>you
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (3878, 'lrc', 'line', 'local_lrc', '[00:00.00]Welcome to your pleasure
[00:03.00]Just don''t make a sound
[00:09.00]Welcome to your pleasure
[00:12.00]Just don''t make a sound
[00:15.00]I''m laid back, I''m laughing
[00:18.00]I''m making a punch
[00:24.00]Lucky me, I''m leaving
[00:27.00]Make my party boots
[00:30.00]It''s a drag in my mind
[00:33.00]Go to paradise
[00:39.00]Welcome to your pleasure
[00:42.00]Just don''t make a sound
[00:48.00]Can''t stand it, getting weaker
[00:51.00]Like getting nowhere
[00:54.00]Feeling weird and I''m weak
[00:57.00]Just do it again
[01:03.00]It was free and I could have
[01:06.00]I could have kissed him
[01:09.00]Welcome to your pleasure
[01:12.00]Just don''t make a sound
[01:18.00]Dancing downtown
[01:21.00]Dancing downtown, dancing
[01:24.00]Dancing downtown
[01:27.00]Dancing downtown, dancing
[01:33.00]Doing it all over
[01:36.00]All over again
[01:39.00]Pretty face, can you do it?
[01:42.00]We''ve got it made
[01:48.00]You''ll be down, you''ll be downtown
[01:51.00]Dancing
[01:54.00]Welcome to your pleasure
[01:57.00]Just don''t make a sound
[02:03.00]Dancing downtown
[02:06.00]Dancing downtown, dancing
[02:09.00]Dancing downtown
[02:12.00]Dancing downtown, dancing
[02:15.00]Dancing downtown
[02:18.00]Dancing downtown, dancing
[02:24.00]Don''t go away
[02:27.00]Don''t go away
[02:30.00]Don''t go away
[02:33.00]Don''t go away
[02:39.00]Don''t say it, baby, I''ve got a problem
[02:42.00]Don''t say it, baby, I''ve got a problem
[02:48.00]Dancing downtown (I''ve got a problem)
[02:51.00]Dancing downtown, dancing
[02:54.00]Dancing downtown (Don''t say it, baby, I''ve got a problem)
[02:57.00]Dancing downtown, dancing
[03:03.00]I''ve got a problem
[03:06.00]I''ve got a problem
[03:12.00]Welcome to your pleasure
[03:15.00]Just don''t make a s...
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (20232, 'lrc', 'line', 'local_lrc', '[00:33.93]<00:33.93>Like<00:34.17> <00:34.29>Animals<00:35.49> <00:36.00>Tonight<00:36.30> <00:36.60>We<00:36.69> <00:36.78>Make<00:36.98> <00:37.17>It
[00:38.23]<00:38.23>You<00:38.35> <00:38.47>Give<00:38.62> <00:38.77>An<00:38.89> <00:39.01>Inch,<00:39.58> <00:40.33>I''m<00:40.45> <00:40.57>Gonna<00:40.78> <00:40.99>Take<00:41.38> <00:41.62>It
[00:42.38]<00:42.38>I''ll<00:42.45> <00:42.53>Steal<00:42.73> <00:42.92>Your<00:42.99> <00:43.07>Love<00:43.82> <00:44.54>Like<00:44.81> <00:45.08>A<00:45.16> <00:45.23>Thief
[00:46.55]<00:46.55>To<00:46.67> <00:46.79>Be<00:47.01> <00:47.24>As<00:47.33> <00:47.42>One<00:47.73> <00:48.05>Is<00:48.14> <00:48.23>My<00:48.62> <00:48.74>Belief
[00:51.09]<00:51.09>Don''t<00:51.23> <00:51.36>Look<00:51.48> <00:51.60>Back,<00:52.23> <00:52.41>Have<00:52.83> <00:53.61>NoRegrets
[00:55.04]<00:55.04>Like<00:55.16> <00:55.28>Beasts<00:55.46> <00:55.64>Of<00:55.70> <00:55.76>Prey<00:55.94> <00:56.12>We<00:56.23> <00:56.33>Must<00:56.48> <00:56.63>Feed<00:56.93> <00:57.23>On<00:57.47> <00:57.71>It
[00:59.29]<00:59.29>I''ll<00:59.40> <00:59.50>Be<00:59.71> <00:59.92>Your<01:00.01> <01:00.10>One,<01:00.64> <01:01.42>Your<01:01.52> <01:01.63>One<01:01.81> <01:01.99>And<01:02.14> <01:02.20>Only
[01:03.37]<01:03.37>To<01:03.48> <01:03.58>Feel<01:03.76> <01:03.94>Me<01:04.06> <01:04.18>Burning,<01:04.87> <01:05.47>Come<01:05.59> <01:05.71>Close<01:05.92> <01:06.13>And<01:06.19> <01:06.25>Hold<01:06.42> <01:06.58>Me
[01:09.99]<01:09.99>(Come<01:10.01> <01:10.02>Closer<01:10.30> <01:10.59>To<01:10.65> <01:12.42>Me)
[01:12.48]<01:12.48>Oooh,<01:12.97> <01:13.46>Tonight''s<01:13.73> <01:14.00>The<01:14.08> <01:14.15>Night<01:14.31> <01:14.46>We<01:14.56> <01:14.66>Give<01:14.84> <01:15.01>It<01:15.14> <01:15.27>Up
[01:15.70]<01:15.70>Flesh<01:16.02> <01:16.33>And<01:16.54> <01:16.75>Blood<01:17.35> <01:17.68>Sacrifice
[01:20.23]<01:20.23>Melts<01:20.51> <01:20.80>The<01:20.92> <01:21.04>Heart<01:21.31> <01:21.58>Like<01:21.97> <01:22.06>Fire<01:22.38> <01:22.69>And<01:22.95> <01:23.20>Ice
[01:24.07]<01:24.07>Flesh<01:24.46> <01:24.85>And<01:25.15> <01:25.21>Blood<01:25.49> <01:25.78>Like<01:26.20> <01:26.29>Fire<01:26.56> <01:26.83>To<01:27.13> <01:27.16>Ice
[01:28.76]<01:28.76>Are<01:29.00> <01:29.24>You<01:29.39> <01:29.54>Willing<01:29.80> <01:30.05>To<01:30.24> <01:30.44>Sacrifice?
[01:32.96]<01:32.96>There''s<01:33.08> <01:33.20>No<01:33.32> <01:33.44>More<01:33.54> <01:33.65>Time<01:34.37> <01:34.97>Don''t<01:35.13> <01:35.30>Think<01:35.48> <01:35.66>About
[01:37.11]<01:37.11>The<01:37.17> <01:37.23>Flame<01:37.45> <01:37.68>Will<01:37.75> <01:37.83>Die<01:38.52> <01:39.24>If<01:39.37> <01:39.51>You<01:39.72> <01:39.93>Doubt
[01:41.54]<01:41.54>It''s<01:41.70> <01:41.87>A<01:41.91> <01:41.96>Game<01:42.32> <01:42.68>Of<01:42.92> <01:43.70>Love<01:43.85> <01:44.00>And<01:44.08> <01:44.15>Hate
[01:45.42]<01:45.42>To<01:45.65> <01:45.87>Lose<01:45.96> <01:46.05>It<01:46.17> <01:46.23>All''s<01:46.56> <01:46.89>A<01:46.92> <01:46.95>Chance<01:47.28> <01:47.61>We<01:47.94> <01:48.06>Take
[01:49.77]<01:49.77>Uh,<01:49.83> <01:49.89>Come<01:50.11> <01:50.34>To<01:50.38> <01:50.43>Me<01:50.80> <01:51.18>And<01:51.24> <01:51.30>Take<01:51.54> <01:51.78>My<01:52.08> <01:52.38>Hand
[01:53.91]<01:53.91>It''s<01:54.06> <01:54.21>In<01:54.33> <01:54.45>The<01:54.51> <01:54.57>Fire<01:54.91> <01:55.26>That<01:55.35> <01:55.44>We<01:55.65> <01:55.86>Must<01:56.15> <01:56.43>Stand
[01:58.15]<01:58.15>I''ll<01:58.26> <01:58.36>Take<01:58.57> <01:58.78>You<01:58.83> <01:58.87>Down<01:59.53> <01:59.77>Under<02:00.10> <02:00.43>My<02:00.68> <02:00.94>Gun
[02:02.48]<02:02.48>Our<02:02.57> <02:02.66>Flesh<02:02.86> <02:03.07>And<02:03.14> <02:03.21>Blood<02:03.69> <02:03.80>Will<02:03.89> <02:03.98>Be<02:04.40> <02:04.83>As<02:04.89> <02:05.04>One
[02:05.33]<02:05.33>(Oh,<02:05.36> <02:09.29>It''s<02:09.50> <02:10.79>Rockin''<02:10.94> <02:12.26>Tonight)
[02:12.29]<02:12.29>So<02:12.45> <02:12.62>Take<02:12.76> <02:12.91>Me<02:13.02> <02:13.12>Down,<02:13.30> <02:13.48>I''ll<02:13.56> <02:13.65>Take<02:13.83> <02:14.01>You<02:14.10> <02:14.19>Down
[02:14.75]<02:14.75>Flesh<02:15.05> <02:15.35>And<02:15.56> <02:15.77>Blood<02:16.40> <02:16.70>Sacrifice
[02:19.17]<02:19.17>Melts<02:19.45> <02:19.74>The<02:19.87> <02:20.01>Heart<02:20.26> <02:20.52>Like<02:20.91> <02:21.00>Fire<02:21.31> <02:21.63>And<02:21.88> <02:22.14>Ice
[02:23.11]<02:23.11>Flesh<02:23.43> <02:23.74>And<02:23.93> <02:24.13>Blood<02:24.41> <02:24.70>Like<02:25.12> <02:25.24>Fire<02:25.51> <02:25.78>To<02:26.02> <02:26.08>Ice
[02:27.68]<02:27.68>Are<02:27.93> <02:28.19>You<02:28.34> <02:28.49>Willing<02:28.75> <02:29.00>To<02:29.20> <02:29.39>Sacrifice?
[02:32.17]<02:32.17>Our<02:32.29> <02:32.41>Love<02:32.68> <02:32.95>Runs<02:33.20> <02:33.46>Deeper<02:33.85> <02:34.24>Than<02:34.42> <02:34.60>A<02:34.66> <02:34.72>River
[02:40.29]<02:40.29>The<02:40.38> <02:40.47>Less<02:40.65> <02:40.83>You<02:40.92> <02:41.01>Need<02:41.31> <02:41.61>The<02:41.72> <02:41.82>More<02:42.19> <02:42.57>I''m<02:42.61> <02:42.66>Gonna<02:42.88> <02:43.11>Give<02:43.27> <02:43.44>You
[02:43.91]<02:43.91>And<02:44.02> <02:44.12>Give<02:44.26> <02:44.39>You<02:44.72> <02:44.96>And<02:45.06> <02:45.17>Give<02:45.32> <02:45.47>You
[02:46.10]<02:46.10>(But<02:46.12> <02:46.13>Good)
[03:30.42]<03:30.42>(Uhh)
[03:30.49]<03:30.49>Flesh<03:30.79> <03:31.09>And<03:31.30> <03:31.51>Blood<03:32.23> <03:32.47>Sacrifice
[03:34.96]<03:34.96>Melts<03:35.23> <03:35.50>The<03:35.65> <03:35.80>Heart<03:36.04> <03:36.28>Like<03:36.52> <03:36.76>Fire<03:37.09> <03:37.42>And<03:37.66> <03:37.90>Ice
[03:38.82]<03:38.82>Flesh<03:39.15> <03:39.48>And<03:39.67> <03:39.87>Blood<03:40.14> <03:40.41>Like<03:40.86> <03:40.95>Fire<03:41.22> <03:41.49>To<03:41.64> <03:41.79>Ice
[03:43.43]<03:43.43>Are<03:43.67> <03:43.91>You<03:44.06> <03:44.21>Willing<03:44.45> <03:44.69>To<03:44.90> <03:45.11>Sacrifice?
[03:47.35]<03:47.35>Flesh<03:47.68> <03:48.01>And<03:48.22> <03:48.43>Blood<03:49.00> <03:49.39>Sacrifice
[03:51.80]<03:51.80>Melts<03:52.07> <03:52.34>The<03:52.49> <03:52.64>Heart<03:52.90> <03:53.15>Like<03:53.51> <03:53.63>Fire<03:53.95> <03:54.26>And<03:54.52> <03:54.77>Ice
[03:55.71]<03:55.71>Flesh<03:56.04> <03:56.37>And<03:56.56> <03:56.76>Blood<03:57.03> <03:57.30>Like<03:57.72> <03:57.84>Fire<03:58.11> <03:58.38>To<03:58.59> <03:58.68>Ice
[04:00.24]<04:00.24>Are<04:00.48> <04:00.72>You<04:00.87> <04:01.02>Ready<04:01.26> <04:01.50>To<04:01.71> <04:01.92>Sacrifice?
[04:04.10]<04:04.10>Flesh<04:04.42> <04:04.73>And<04:04.94> <04:05.15>Blood<04:05.66> <04:06.17>Sacrifice
[04:08.64]<04:08.64>Melts<04:08.92> <04:09.21>The<04:09.34> <04:09.48>Heart<04:09.73> <04:09.99>Like<04:10.15> <04:10.32>Fire<04:10.71> <04:11.10>And<04:11.36> <04:11.61>Ice
[04:12.52]<04:12.52>Flesh<04:12.84> <04:13.15>And<04:13.36> <04:13.57>Blood<04:13.84> <04:14.11>Like<04:14.59> <04:14.65>Fire<04:14.94> <04:15.22>To<04:15.37> <04:15.52>Ice
[04:17.17]<04:17.17>Are<04:17.41> <04:17.64>You<04:17.79> <04:17.93>Ready<04:18.18> <04:18.43>To<04:18.76> <04:18.79>Sacrifice?
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (19843, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.34>Getting<00:00.67> <00:01.01>Some<00:01.34>)<00:01.68> <00:02.02>Fun<00:02.35> <00:02.69>out<00:03.02> <00:03.36>of<00:03.70> <00:04.03>Life<00:04.37> <00:04.70>(<00:05.04>LP<00:05.38> <00:05.71>Version<00:06.05>)<00:06.38> <00:06.72>-<00:07.06> <00:07.39>Madeleine<00:07.73> <00:08.06>Peyroux
[00:08.42]<00:08.42>When <00:08.67>we <00:08.85>want <00:09.04>to <00:09.24>love  <00:10.48>we <00:10.73>love
[00:12.48]<00:12.48>When <00:12.66>we <00:12.91>want <00:13.29>to <00:13.65>kiss  <00:14.72>we <00:14.98>kiss
[00:16.78]<00:16.78>With <00:17.03>a <00:17.34>little <00:17.78>petting  <00:19.09>we''re <00:20.02>getting
[00:22.11]<00:22.11>Some <00:22.60>fun <00:23.22>out <00:23.66>of <00:23.98>life
[00:25.72]<00:25.72>When <00:25.97>we <00:26.16>want <00:26.60>to <00:26.78>work  <00:27.65>we <00:27.91>work
[00:29.78]<00:29.78>When <00:30.03>we <00:30.35>wanna <00:30.77>play  <00:32.27>we <00:32.46>play
[00:34.02]<00:34.02>In <00:34.21>a <00:34.71>happy <00:35.46>setting  <00:36.57>we''re <00:37.40>getting
[00:39.64]<00:39.64>Some <00:39.95>fun <00:40.65>out <00:41.02>of <00:41.28>life
[00:44.83]<00:44.83>Maybe <00:45.52>we <00:46.02>do <00:46.33>the <00:46.64>right <00:46.96>things
[00:49.20]<00:49.20>Maybe <00:49.64>we <00:49.95>do <00:50.32>the <00:50.57>wrong
[00:52.77]<00:52.77>Spending <00:53.31>each <00:53.57>day
[00:55.06]<00:55.06>Wending <00:55.81>our <00:56.32>way <00:57.00>along
[00:59.43]<00:59.43>But <00:59.75>when <01:00.12>we <01:00.44>want <01:00.81>to <01:01.12>sing  <01:02.06>we <01:02.74>sing
[01:04.17]<01:04.17>When <01:04.61>we <01:04.94>want <01:05.36>to <01:05.68>dance  <01:06.67>we <01:06.98>dance
[01:08.54]<01:08.54>You <01:08.67>can <01:09.21>do <01:09.59>your <01:09.96>betting  <01:11.08>we''re <01:12.65>getting
[01:14.33]<01:14.33>Some <01:14.71>fun <01:15.39>out <01:15.77>of <01:16.02>life
[02:28.23]<02:28.23>Maybe <02:29.41>we <02:29.85>do <02:30.47>the <02:30.72>right <02:30.97>things
[02:33.03]<02:33.03>Maybe <02:34.02>we <02:34.28>do <02:34.65>the <02:34.97>wrong
[02:36.90]<02:36.90>Spending <02:37.53>each <02:37.72>day
[02:39.02]<02:39.02>Wending <02:39.83>our <02:40.27>way <02:40.89>along
[02:43.63]<02:43.63>But <02:43.82>when <02:44.14>we <02:44.51>want <02:44.76>to <02:45.07>sing  <02:46.25>we <02:46.69>sing
[02:48.12]<02:48.12>When <02:48.69>we <02:48.94>wanna <02:49.69>dance  <02:50.81>we <02:51.07>dance
[02:52.74]<02:52.74>You <02:53.12>can <02:53.50>do <02:53.81>your <02:54.25>betting  <02:55.56>we''re <02:56.36>getting
[02:57.74]<02:57.74>Some <02:58.86>fun <02:59.67>out <02:59.99>of <03:00.35>life
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (5214, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.46>Ghost<00:00.92>)<00:01.37> <00:01.83>Riders<00:02.29> <00:02.75>in<00:03.21> <00:03.66>the<00:04.12> <00:04.58>Sky<00:05.04> <00:05.50>-<00:05.95> <00:06.41>Johnny<00:06.87> <00:07.33>Cash<00:07.79> <00:08.24>(<00:08.70>约<00:09.16>翰<00:09.62>尼<00:10.08>·<00:10.53>卡<00:10.99>什<00:11.45>)
[00:11.94]<00:11.94>An <00:12.16>old <00:12.37>cowboy <00:12.98>went <00:13.18>riding <00:13.43>out <00:13.88>one <00:14.20>dark <00:14.56>and <00:14.82>windy <00:15.21>day
[00:18.60]<00:18.60>Upon <00:18.86>a <00:19.10>ridge <00:19.46>he <00:19.87>rested
[00:20.19]<00:20.19>As <00:20.61>he <00:20.82>went <00:21.12>along <00:21.57>his <00:22.00>way
[00:25.15]<00:25.15>When <00:25.35>all <00:25.58>at <00:25.80>once <00:26.16>a <00:26.44>mighty <00:26.82>herd <00:27.15>of
[00:27.47]<00:27.47>Red <00:27.70>eyed <00:28.05>cows <00:28.29>he <00:28.53>saw
[00:29.84]<00:29.84>A <00:30.02>plowing <00:30.67>through <00:30.91>the <00:31.11>ragged <00:31.36>sky
[00:34.00]<00:34.00>And <00:34.20>up <00:34.51>the <00:34.71>cloudy <00:35.13>draw
[00:39.42]<00:39.42>Their <00:39.62>brands <00:40.01>were <00:40.32>still <00:40.53>on <00:40.76>fire
[00:41.15]<00:41.15>And <00:41.42>their <00:41.76>hooves <00:42.16>were <00:42.52>made <00:42.77>of <00:43.00>steel
[00:46.13]<00:46.13>Their <00:46.36>horns <00:46.70>were <00:46.80>black <00:47.06>and <00:47.27>shiny
[00:47.73]<00:47.73>And <00:48.00>their <00:48.35>hot <00:48.58>breath <00:48.94>he <00:49.18>could <00:49.41>feel
[00:52.73]<00:52.73>A <00:52.92>bolt <00:53.17>of <00:53.36>fear <00:53.65>went <00:53.87>through <00:54.11>him
[00:54.62]<00:54.62>As <00:54.82>they <00:54.98>thundered <00:55.68>through <00:55.92>the <00:56.16>sky
[00:57.02]<00:57.02>For <00:57.20>he <00:57.35>saw <00:57.78>the <00:58.01>Riders <00:58.45>coming <00:58.73>hard
[01:01.10]<01:01.10>And <01:01.44>he <01:01.65>heard <01:01.92>their <01:02.14>mournful <01:02.80>cry
[01:06.29]<01:06.29>Yippie <01:06.84>yi <01:07.37>Ohhhhh
[01:10.65]<01:10.65>Yippie <01:11.06>yi <01:11.61>yaaaaay
[01:16.27]<01:16.27>Ghost <01:17.31>Riders <01:18.11>in <01:20.12>the <01:20.40>sky
[01:51.06]<01:51.06>Their <01:51.28>faces <01:51.57>gaunt
[01:52.01]<01:52.01>Their <01:52.23>eyes <01:52.49>were <01:52.69>blurred
[01:53.27]<01:53.27>Their <01:53.52>shirts <01:53.83>all <01:54.04>soaked <01:54.28>with <01:54.48>sweat
[01:57.53]<01:57.53>He''s <01:57.73>riding <01:58.14>hard <01:58.43>to <01:58.66>catch <01:58.86>that <01:59.08>herd
[01:59.70]<01:59.70>But <01:59.90>he <02:00.10>ain''t <02:00.46>caught <02:00.65>''em <02:00.90>yet
[02:03.73]<02:03.73>''Cause <02:03.92>they''ve <02:04.17>got <02:04.44>to <02:04.66>ride <02:05.01>forever <02:05.57>on
[02:06.08]<02:06.08>That <02:06.27>range <02:06.63>up <02:06.87>in <02:07.08>the <02:07.31>sky
[02:08.46]<02:08.46>On <02:08.66>horses <02:09.13>snorting <02:09.50>fire
[02:12.45]<02:12.45>As <02:12.65>they <02:12.85>ride <02:13.04>on <02:13.54>hear <02:13.82>their <02:14.03>cry
[02:19.17]<02:19.17>As <02:19.38>the <02:19.55>riders <02:19.88>loped <02:20.09>on <02:20.41>by <02:20.70>him
[02:21.18]<02:21.18>He <02:21.54>heard <02:21.79>one <02:22.04>call <02:22.37>his <02:22.61>name
[02:27.74]<02:27.74>If <02:27.92>you <02:28.10>want <02:28.36>to <02:28.63>save <02:28.88>your <02:29.19>soul <02:29.62>from <02:29.84>Hell
[02:30.15]<02:30.15>A <02:30.47>riding <02:30.80>on <02:31.06>our <02:31.25>range
[02:32.31]<02:32.31>Then <02:32.49>cowboy <02:32.87>change <02:33.17>your <02:33.45>ways <02:34.01>today
[02:34.32]<02:34.32>Or <02:34.52>with <02:34.81>us <02:35.03>you <02:35.28>will <02:35.63>ride
[02:36.89]<02:36.89>Trying <02:37.19>to <02:37.51>catch <02:37.69>the <02:37.94>Devil''s <02:38.37>herd
[02:41.16]<02:41.16>Across <02:41.44>these <02:41.69>endless <02:42.20>skies
[02:45.70]<02:45.70>Yippie <02:46.20>yi <02:46.66>Ohhhhh
[02:49.94]<02:49.94>Yippie <02:50.40>yi <02:50.94>Yaaaaay
[02:55.51]<02:55.51>Ghost <02:56.24>Riders <02:57.41>in <02:59.30>the <02:59.59>sky
[03:04.12]<03:04.12>Ghost <03:05.14>Riders <03:06.19>in <03:07.84>the <03:08.16>sky
[03:12.71]<03:12.71>Ghost <03:13.69>Riders <03:14.73>in <03:16.49>the <03:16.89>sky
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (21208, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00><00:00.45>Girl <00:00.48>We <00:00.51>Got <00:00.55>A) <00:00.58>Good <00:00.61>Thing <00:00.64>- <00:00.68>Weezer
[00:00.71]<00:00.71>Written <00:00.74>by<00:00.78>：<00:00.81>Rivers Cuomo
[00:06.97]<00:06.97>Girl <00:07.24>we <00:07.43>got <00:07.72>a <00:07.92>good <00:08.21>thing
[00:10.61]<00:10.61>You <00:10.79>know <00:10.96>where <00:11.12>this <00:11.31>is <00:11.49>heading <00:12.76>uh <00:12.96>huh
[00:14.37]<00:14.37>Just <00:14.62>a <00:14.80>couple <00:15.15>lovebirds
[00:18.17]<00:18.17>Happy <00:18.55>to <00:18.78>be <00:19.00>singing <00:20.28>uh <00:20.51>huh
[00:21.89]<00:21.89>Girl <00:22.22>we <00:22.42>got <00:22.64>a <00:22.83>good <00:23.14>thing
[00:25.52]<00:25.52>And <00:25.72>I <00:25.98>don''t <00:26.18>see <00:26.37>this <00:26.56>ending
[00:28.65]<00:28.65>Do <00:28.82>you <00:28.96>want <00:29.09>to <00:29.21>fly <00:29.61>do <00:29.77>you <00:29.90>want <00:30.02>to <00:30.15>flee
[00:30.52]<00:30.52>Do <00:30.67>you <00:30.77>want <00:30.89>to <00:31.01>get <00:31.45>away <00:31.74>with <00:31.89>me
[00:32.32]<00:32.32>Do <00:32.46>you <00:32.60>want <00:32.73>to <00:32.85>face <00:33.26>the <00:33.84>great <00:35.46>unknown
[00:37.45]<00:37.45>Jingle <00:37.77>jingle
[00:39.45]<00:39.45>We''re <00:39.65>as <00:39.79>happy <00:40.23>as <00:40.42>a <00:40.62>couple <00:41.13>hare <00:41.41>krishnas
[00:43.41]<00:43.41>Dancing <00:43.81>twirling <00:44.35>playing <00:44.78>on <00:44.99>the <00:45.22>tambourine
[00:47.16]<00:47.16>We''ll <00:47.38>crush <00:47.77>the <00:48.08>scene <00:48.74>together
[00:52.49]<00:52.49>Marching <00:52.80>onward
[00:54.79]<00:54.79>Oblivious <00:55.51>to <00:55.72>all <00:55.94>the <00:56.13>hate <00:56.43>around <00:56.83>us
[00:57.88]<00:57.88>We <00:58.16>could <00:58.38>self <00:58.66>publish <00:59.20>a <00:59.47>book <00:59.69>of <00:59.90>our <01:00.17>philosophy
[01:02.06]<01:02.06>And <01:02.23>hand <01:02.44>it <01:02.63>to <01:03.65>the <01:03.85>tourists
[01:07.01]<01:07.01>Girl <01:07.22>we <01:07.40>got <01:07.58>a <01:07.79>good <01:08.20>thing
[01:10.65]<01:10.65>You <01:10.84>know <01:11.05>where <01:11.20>this <01:11.41>is <01:11.59>heading <01:12.92>uh <01:13.14>huh
[01:14.40]<01:14.40>Just <01:14.63>a <01:14.81>couple <01:15.17>lovebirds
[01:18.17]<01:18.17>Happy <01:18.55>to <01:18.78>be <01:19.01>singing <01:20.33>uh <01:20.57>huh
[01:21.93]<01:21.93>Girl <01:22.12>we <01:22.32>got <01:22.55>a <01:22.74>good <01:23.09>thing
[01:25.67]<01:25.67>And <01:25.91>I <01:26.10>don''t <01:26.26>see <01:26.48>this <01:26.65>ending
[01:28.63]<01:28.63>Do <01:28.77>you <01:28.91>want <01:29.01>to <01:29.17>fly <01:29.70>do <01:29.85>you <01:29.99>want <01:30.10>to <01:30.23>flee
[01:30.48]<01:30.48>Do <01:30.59>you <01:30.73>want <01:30.87>to <01:30.99>get <01:31.45>away <01:31.95>with <01:32.10>me
[01:32.40]<01:32.40>Do <01:32.56>you <01:32.68>want <01:32.82>to <01:32.94>face <01:33.35>the <01:33.80>great <01:35.40>unknown
[01:37.62]<01:37.62>Puerto <01:38.13>rico
[01:39.30]<01:39.30>Would <01:39.48>be <01:39.66>perfect <01:40.11>for <01:40.33>a <01:40.53>destination <01:41.42>wedding
[01:43.22]<01:43.22>We''ll <01:43.39>drive <01:43.68>into <01:44.23>ventura <01:44.83>on <01:45.08>the <01:45.24>101
[01:47.19]<01:47.19>It <01:47.37>sounds <01:47.59>like <01:47.78>fun <01:48.66>to <01:48.84>me
[01:53.93]<01:53.93>You <01:54.70>scare <01:55.62>me <01:56.53>like <01:57.44>an <01:58.12>open <01:59.28>window
[02:01.29]<02:01.29>Let''s <02:02.04>chalk <02:03.02>it <02:03.97>up <02:04.96>to <02:05.36>stockholm <02:06.76>syndrome
[02:08.90]<02:08.90>I <02:09.57>want <02:10.49>to <02:11.45>crawl <02:12.39>in <02:13.26>crawl <02:13.96>into <02:14.45>a <02:14.95>hole
[02:33.04]<02:33.04>Girl <02:33.29>we <02:33.47>got <02:33.66>a <02:33.86>good <02:34.29>thing
[02:36.76]<02:36.76>You <02:36.96>know <02:37.11>where <02:37.28>this <02:37.44>is <02:37.60>heading <02:39.04>uh <02:39.24>huh
[02:40.60]<02:40.60>Just <02:40.80>a <02:40.96>couple <02:41.40>lovebirds
[02:44.36]<02:44.36>Happy <02:44.64>to <02:44.84>be <02:45.08>singing <02:46.45>uh <02:46.72>huh
[02:48.00]<02:48.00>Girl <02:48.24>we <02:48.44>got <02:48.66>a <02:48.84>good <02:49.14>thing
[02:51.82]<02:51.82>And <02:52.03>I <02:52.20>don''t <02:52.37>see <02:52.55>this <02:52.72>ending
[02:54.66]<02:54.66>Do <02:54.80>you <02:54.95>want <02:55.08>to <02:55.21>fly <02:55.63>do <02:55.78>you <02:55.90>want <02:56.02>to <02:56.18>flee
[02:56.53]<02:56.53>Do <02:56.70>you <02:56.84>want <02:57.01>to <02:57.16>get <02:57.55>away <02:57.80>with <02:57.99>me
[02:58.53]<02:58.53>Do <02:58.70>you <02:58.83>want <02:58.94>to <02:59.07>face <02:59.42>the <02:59.85>great <03:01.51>unknown
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (6128, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.20>I<00:00.39> <00:00.59>Can''t<00:00.78> <00:00.98>Get<00:01.18> <00:01.37>No<00:01.57>)<00:01.76> <00:01.96>Satisfaction<00:02.16> <00:02.35>-<00:02.55> <00:02.74>The<00:02.94> <00:03.14>Rolling<00:03.33> <00:03.53>Stones<00:03.72> <00:03.92>(<00:04.12>滚<00:04.31>石<00:04.51>乐<00:04.70>队<00:04.90>)
[00:05.10]<00:05.10>Lyrics<00:05.67> <00:06.23>by<00:06.80>：<00:07.37>The<00:07.93> <00:08.50>Rolling<00:09.07> <00:09.64>Stines
[00:10.21]<00:10.21>Composed<00:10.67> <00:11.14>by<00:11.60>：<00:12.06>Mick<00:12.53> <00:12.99>Jagger<00:13.45>/<00:13.91>Keith<00:14.38> <00:14.84>Richards
[00:15.32]<00:15.32>I <00:15.97>can''t <00:16.43>get <00:16.78>no <00:18.77>satisfaction
[00:21.97]<00:21.97>I <00:22.68>can''t <00:23.18>get <00:23.79>no <00:25.37>satisfaction
[00:28.06]<00:28.06>''Cause <00:28.62>I <00:28.82>try <00:29.73>and <00:29.84>I <00:30.09>try <00:31.66>and <00:31.82>I <00:32.02>try <00:33.39>and <00:33.54>I <00:34.46>try
[00:35.17]<00:35.17>I <00:35.62>can''t <00:35.83>get <00:36.18>no <00:38.27>I <00:38.42>can''t <00:38.62>get <00:39.23>no
[00:41.37]<00:41.37>When <00:41.87>I''m <00:42.12>drivin'' <00:42.74>in <00:43.19>my <00:43.95>car
[00:44.98]<00:44.98>And <00:45.48>that <00:46.09>man <00:46.29>comes <00:46.75>on <00:46.90>the <00:47.41>radio
[00:49.04]<00:49.04>He''s <00:49.54>tellin'' <00:49.95>me <00:50.20>more <00:50.56>and <00:50.91>more
[00:51.68]<00:51.68>About <00:52.38>some <00:52.84>useless <00:53.86>information
[00:55.55]<00:55.55>Supposed <00:55.73>to <00:56.62>fire <00:56.85>my <00:57.26>imagination
[00:59.09]<00:59.09>I <00:59.59>can''t <01:00.00>get <01:00.41>no <01:01.47>oh <01:02.74>no <01:03.05>no <01:03.35>no
[01:06.00]<01:06.00>Hey <01:06.50>hey <01:06.86>hey <01:08.74>that''s <01:09.50>what <01:09.70>I <01:10.36>say
[01:14.43]<01:14.43>I <01:15.29>can''t <01:15.74>get <01:16.25>no <01:17.98>satisfaction
[01:21.44]<01:21.44>I <01:22.24>can''t <01:22.60>get <01:23.11>no <01:24.88>satisfaction
[01:27.43]<01:27.43>''Cause <01:27.98>I <01:28.49>try <01:28.84>and <01:29.30>I <01:29.76>try <01:31.08>and <01:31.33>I <01:31.69>try <01:32.76>and <01:32.91>I <01:33.31>try
[01:34.59]<01:34.59>I <01:35.14>can''t <01:35.34>get <01:35.55>no <01:36.72>I <01:37.99>can''t <01:38.39>get <01:38.95>no
[01:41.14]<01:41.14>When <01:41.64>I''m <01:41.95>watchin'' <01:42.41>my <01:43.22>TV
[01:44.59]<01:44.59>And <01:45.09>a <01:45.55>man <01:45.91>comes <01:46.36>on <01:46.52>and <01:47.13>tells <01:47.53>me
[01:48.60]<01:48.60>How <01:49.11>white <01:49.46>my <01:49.87>shirts <01:50.33>can <01:50.63>be
[01:51.71]<01:51.71>But <01:52.16>he <01:52.62>can''t <01:52.97>be <01:53.48>a <01:53.89>man <01:54.04>''cause <01:54.30>he <01:54.75>doesn''t <01:54.90>smoke
[01:55.82]<01:55.82>The <01:56.53>same <01:56.73>cigarettes <01:56.94>as <01:57.65>me
[01:58.67]<01:58.67>I <01:59.17>can''t <01:59.47>get <02:00.03>no <02:01.50>oh <02:02.52>no <02:02.72>no <02:03.13>no
[02:05.67]<02:05.67>Hey <02:06.13>hey <02:06.38>hey <02:09.17>that''s <02:09.33>what <02:09.73>I <02:10.09>say
[02:14.41]<02:14.41>I <02:15.06>can''t <02:15.72>get <02:16.08>no <02:17.76>satisfaction
[02:21.11]<02:21.11>I <02:21.77>can''t <02:22.43>get <02:22.99>no <02:24.51>girl <02:25.37>reaction
[02:27.54]<02:27.54>''Cause <02:27.81>I <02:27.96>try <02:29.18>and <02:29.39>I <02:29.79>try <02:30.96>and <02:31.11>I <02:31.26>try <02:32.69>and <02:32.84>I <02:32.99>try
[02:34.27]<02:34.27>I <02:34.82>can''t <02:35.02>get <02:35.28>no <02:38.07>I <02:38.27>can''t <02:38.48>get <02:38.88>no
[02:40.97]<02:40.97>When <02:41.42>I''m <02:41.78>ridin'' <02:42.79>round <02:43.15>the <02:43.40>world
[02:44.78]<02:44.78>And <02:45.23>I''m <02:45.53>doin'' <02:46.14>this <02:46.40>and <02:46.60>I''m <02:46.96>signing <02:47.77>that
[02:48.33]<02:48.33>And <02:48.99>I''m <02:49.34>tryin'' <02:49.55>to <02:49.90>make <02:50.05>some <02:50.56>girl
[02:51.37]<02:51.37>Who <02:51.60>tells <02:51.83>me <02:52.31>baby <02:52.79>better <02:53.23>come <02:53.51>back <02:54.26>maybe <02:54.59>next <02:55.05>week
[02:55.52]<02:55.52>''Cause <02:55.72>you <02:55.92>see <02:56.35>I''m <02:56.87>on <02:57.56>a <02:57.78>losing <02:58.37>streak
[02:58.84]<02:58.84>I <02:59.35>can''t <02:59.65>get <02:59.91>no <03:02.24>oh <03:02.55>no <03:02.70>no <03:03.00>no
[03:05.40]<03:05.40>Hey <03:05.85>hey <03:06.05>hey <03:08.99>that''s <03:09.15>what <03:09.55>I <03:09.96>say
[03:12.81]<03:12.81>I <03:13.26>can''t <03:13.57>get <03:14.07>no <03:16.61>I <03:16.82>can''t <03:17.27>get <03:17.68>no
[03:19.82]<03:19.82>I <03:20.32>can''t <03:20.68>get <03:21.13>no <03:23.57>satisfaction
[03:26.57]<03:26.57>No <03:26.93>satisfaction
[03:30.09]<03:30.09>No <03:30.48>satisfaction
[03:33.60]<03:33.60>No <03:34.00>satisfaction
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (15841, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.01>I<00:00.02> <00:00.02>Can''t<00:00.03> <00:00.04>Help<00:00.05>)<00:00.06> <00:00.06>Falling<00:00.07> <00:00.08>In<00:00.09> <00:00.10>Love<00:00.10> <00:00.11>With<00:00.12> <00:00.13>You<00:00.14> <00:00.14>-<00:00.15> <00:00.16>UB40
[00:00.17]<00:00.17>Lyrics<00:00.18> <00:00.19>by<00:00.20>：<00:00.21>Hugo<00:00.22> <00:00.23>E.<00:00.24> <00:00.25>Peretti<00:00.26>/<00:00.27>George<00:00.28> <00:00.29>Weiss<00:00.30>/<00:00.31>Luigi<00:00.32> <00:00.33>Creatore
[00:00.34]<00:00.34>Composed<00:00.35> <00:00.36>by<00:00.37>：<00:00.38>Hugo<00:00.39> <00:00.40>E.<00:00.41> <00:00.42>Peretti<00:00.43>/<00:00.44>George<00:00.45> <00:00.46>Weiss<00:00.47>/<00:00.48>Luigi<00:00.49> <00:00.50>Creatore
[00:00.52]<00:00.52>Wise <00:01.69>men <00:03.12>say
[00:05.50]<00:05.50>Only <00:06.24>fools <00:07.43>rush <00:08.70>in
[00:11.46]<00:11.46>But <00:11.68>I <00:12.80>can''t <00:14.23>help <00:15.57>falling <00:16.53>in <00:16.83>love <00:18.09>with <00:19.49>you
[00:36.42]<00:36.42>Wise <00:37.65>men <00:39.10>say
[00:41.65]<00:41.65>Only <00:42.26>fools <00:43.56>rush <00:44.65>in
[00:47.56]<00:47.56>But <00:47.76>I <00:48.65>can''t <00:50.06>help <00:51.85>falling <00:52.75>in <00:53.00>love <00:54.40>with <00:55.71>you
[00:58.71]<00:58.71>Shall <01:00.01>I <01:01.32>stay
[01:03.90]<01:03.90>Would <01:04.12>it <01:04.30>be <01:05.42>a <01:06.73>sin
[01:09.89]<01:09.89>If <01:10.16>I <01:10.89>can''t <01:12.45>help <01:14.11>falling <01:15.02>in <01:15.31>love <01:16.61>with <01:17.94>you
[01:21.42]<01:21.42>Like <01:21.67>a <01:21.89>river <01:22.34>flows
[01:23.76]<01:23.76>Surely <01:24.32>to <01:24.68>the <01:24.99>sea
[01:26.83]<01:26.83>Darling <01:27.09>so <01:27.45>it <01:27.79>goes
[01:29.33]<01:29.33>Some <01:29.56>things <01:31.37>are <01:31.56>meant <01:31.90>to <01:32.19>be
[01:35.71]<01:35.71>Take <01:35.96>my <01:37.68>hand
[01:40.04]<01:40.04>Take <01:40.28>my <01:40.83>whole <01:42.24>life <01:43.06>too
[01:45.84]<01:45.84>For <01:46.29>I <01:47.19>can''t <01:48.59>help <01:50.27>falling <01:51.21>in <01:51.50>love <01:52.87>with <01:54.10>you
[01:57.48]<01:57.48>As <01:57.70>a <01:57.90>river <01:58.56>flows
[01:59.97]<01:59.97>Surely <02:00.68>to <02:00.96>the <02:01.22>sea
[02:02.89]<02:02.89>Darling <02:03.46>so <02:03.73>it <02:04.02>goes
[02:05.52]<02:05.52>Some <02:05.74>things <02:07.58>are <02:08.03>meant <02:08.21>to <02:08.39>be
[02:11.87]<02:11.87>Take <02:12.26>my <02:13.78>hand
[02:16.24]<02:16.24>Take <02:16.49>my <02:16.87>whole <02:18.12>life <02:19.19>too
[02:22.26]<02:22.26>For <02:22.50>I <02:23.47>can''t <02:24.89>help <02:26.52>falling <02:27.39>in <02:27.67>love <02:28.99>with <02:30.25>you
[02:33.38]<02:33.38>I <02:34.56>can''t <02:35.96>help <02:37.61>falling <02:38.47>in <02:38.75>love <02:40.12>with <02:41.39>you
[02:44.26]<02:44.26>I <02:45.55>can''t <02:47.06>help <02:48.70>falling <02:49.54>in <02:49.85>love <02:51.23>with <02:52.54>you
[02:55.31]<02:55.31>I <02:56.75>can''t <02:58.19>help <02:59.83>falling <03:00.69>in <03:01.01>love <03:02.43>with <03:03.63>you
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (14653, 'lrc', 'line', 'local_lrc', '[00:02.87]<00:02.87>Yippi-<00:02.90>yay
[00:04.92]<00:04.92>There''ll<00:05.10> <00:05.28>be<00:05.38> <00:05.49>not<00:05.73> <00:05.97>wedding<00:06.30> <00:06.63>bells<00:07.14> <00:07.65>for<00:07.84> <00:08.04>today
[00:12.93]<00:12.93>''Cause<00:13.00> <00:13.08>I<00:13.14> <00:13.20>got<00:13.33> <00:13.46>spurs<00:14.02> <00:14.58>that<00:14.82> <00:16.12>jingle,<00:16.47> <00:16.82>jangle,<00:17.09> <00:17.36>jingle<00:18.21> <00:18.27>(jingle,<00:18.29> <00:18.30>jangle)
[00:18.33]<00:18.33>As<00:18.42> <00:18.51>I<00:18.58> <00:18.66>go<00:19.02> <00:19.38>ridin''<00:19.63> <00:19.89>merrily<00:20.47> <00:21.06>along<00:22.29> <00:23.43>(jingle,<00:23.44> <00:23.46>jangle)
[00:23.58]<00:23.58>And<00:23.62> <00:23.67>they<00:23.74> <00:23.82>sing,<00:24.42> <00:24.54>"Oh,<00:25.44> <00:25.53>ain''t<00:25.66> <00:25.80>you<00:25.84> <00:25.89>glad<00:26.08> <00:26.28>you''re<00:26.35> <00:26.43>single?"<00:27.27> <00:28.32>(Jingle,<00:28.33> <00:28.35>jangle)
[00:28.75]<00:28.75>And<00:28.81> <00:28.87>that<00:28.98> <00:29.08>song<00:29.48> <00:29.89>ain''t<00:29.98> <00:30.07>so<00:30.22> <00:30.37>very<00:30.62> <00:30.88>far<00:31.14> <00:31.39>from<00:31.57> <00:31.75>wrong<00:32.29> <00:32.83>(jingle,<00:32.86> <00:33.25>jangle)
[00:33.29]<00:33.29>Oh,<00:33.45> <00:33.61>Lillie<00:33.84> <00:34.07>Belle<00:34.54> <00:35.02>(oh,<00:35.03> <00:35.05>Lillie<00:35.34> <00:35.71>Belle)
[00:35.74]<00:35.74>Oh,<00:35.91> <00:36.07>Lillie<00:36.34> <00:36.61>Belle<00:37.03> <00:37.45>(oh,<00:37.47> <00:37.48>Lillie<00:37.67> <00:37.87>Belle)
[00:38.79]<00:38.79>Though<00:38.98> <00:39.18>I<00:39.26> <00:39.33>may<00:39.57> <00:39.81>have<00:39.95> <00:40.08>done<00:40.23> <00:40.38>some<00:40.53> <00:40.68>foolin''
[00:41.37]<00:41.37>This<00:41.56> <00:41.76>is<00:41.87> <00:41.97>why<00:42.13> <00:42.30>I<00:42.45> <00:42.60>never<00:42.87> <00:43.14>fell
[00:43.78]<00:43.78>''Cause<00:43.88> <00:43.99>I<00:44.05> <00:44.11>got<00:44.24> <00:44.38>spurs<00:44.98> <00:45.58>that<00:45.76> <00:45.85>jingle,<00:46.42> <00:47.05>jangle,<00:47.42> <00:47.80>jingle<00:48.31> <00:48.55>(jingle,<00:48.56> <00:48.58>jangle)
[00:49.28]<00:49.28>As<00:49.38> <00:49.49>I<00:49.56> <00:49.64>go<00:50.00> <00:50.36>ridin''<00:50.62> <00:50.87>merrily<00:51.45> <00:52.04>along<00:52.59> <00:53.15>(jingle,<00:53.16> <00:53.18>jangle)
[00:54.40]<00:54.40>And<00:54.45> <00:54.49>they<00:54.58> <00:54.67>sing,<00:55.21> <00:55.42>"Oh,<00:56.26> <00:56.29>ain''t<00:56.44> <00:56.59>you<00:56.63> <00:56.68>glad<00:56.98> <00:57.13>you''re<00:57.17> <00:57.22>single?"<00:58.09> <00:58.39>(Jingle,<00:58.40> <00:58.42>jangle)
[00:59.63]<00:59.63>And<00:59.69> <00:59.75>that<00:59.85> <00:59.96>song<01:00.35> <01:00.74>ain''t<01:00.84> <01:00.95>so<01:01.28> <01:01.31>very<01:01.61> <01:01.79>far<01:02.02> <01:02.24>from<01:02.43> <01:02.63>wrong<01:03.32> <01:04.01>(jingle,<01:04.04> <01:56.12>jangle)
[01:56.17]<01:56.17>Oh,<01:56.53> <01:56.62>I<01:56.73> <01:56.83>got<01:56.98> <01:57.13>spurs<01:57.69> <01:58.24>that<01:58.38> <01:58.51>jingle,<01:58.81> <01:59.11>jangle,<01:59.20> <01:59.29>jingle<01:59.38> <01:59.47>(I<01:59.48> <01:59.50>got<01:59.62> <01:59.74>spurs<02:00.28> <02:00.82>that<02:00.93> <02:01.03>jingle,<02:01.41> <02:01.78>jangle,<02:02.41> <02:02.53>jingle)
[02:02.56]<02:02.56>As<02:02.65> <02:02.74>I<02:02.75> <02:02.77>go<02:02.95> <02:03.13>ridin''<02:03.38> <02:03.64>merrily<02:04.08> <02:04.51>along<02:04.74> <02:04.96>(I<02:04.99> <02:05.02>go<02:05.36> <02:05.71>ridin''<02:05.98> <02:06.25>merrily<02:06.73> <02:07.21>along)
[02:07.26]<02:07.26>And<02:07.77> <02:08.28>they<02:08.31> <02:08.34>sing,<02:08.43> <02:08.52>"Oh,<02:09.13> <02:09.19>ain''t<02:09.32> <02:09.45>you<02:09.50> <02:09.54>glad<02:09.83> <02:09.98>you''re<02:10.02> <02:10.07>single?"<02:10.43> <02:10.80>(And<02:10.81> <02:10.83>they<02:10.86> <02:10.89>sing,<02:10.93> <02:10.97>"Oh,<02:11.38> <02:11.47>ain''t<02:11.60> <02:11.73>you<02:11.81> <02:11.88>glad<02:12.04> <02:12.20>you''re<02:12.25> <02:12.29>single")
[02:12.32]<02:12.32>And<02:12.38> <02:12.44>that<02:12.53> <02:12.62>song<02:13.03> <02:13.43>ain''t<02:13.54> <02:13.64>so<02:13.94> <02:13.97>very<02:14.21> <02:14.45>far<02:14.68> <02:14.90>from<02:15.08> <02:15.26>wrong<02:15.39> <02:15.53>(and<02:15.54> <02:15.56>that<02:15.61> <02:15.65>song<02:15.81> <02:15.98>ain''t<02:16.09> <02:16.19>so<02:16.36> <02:16.52>very<02:16.77> <02:17.03>far<02:17.27> <02:17.51>from<02:17.72> <02:20.81>wrong)
[02:20.94]<02:20.94>Oh,<02:21.06> <02:21.18>Lillie<02:21.40> <02:21.63>Belle
[02:23.42]<02:23.42>Oh,<02:23.54> <02:23.66>Lillie<02:23.88> <02:24.11>Belle
[02:25.27]<02:25.27>Though<02:25.41> <02:25.54>I<02:25.59> <02:25.63>may<02:25.79> <02:25.96>have<02:26.09> <02:26.23>done<02:26.38> <02:26.53>some<02:26.71> <02:26.89>foolin''
[02:27.81]<02:27.81>This<02:27.96> <02:28.11>is<02:28.18> <02:28.26>why<02:28.41> <02:28.56>I<02:28.68> <02:28.80>never<02:29.04> <02:29.28>lie<02:29.48> <02:29.67>and<02:29.71> <02:29.76>never<02:29.82> <02:29.88>fell
[02:30.15]<02:30.15>''Cause<02:30.23> <02:30.30>I<02:30.34> <02:30.39>got<02:30.52> <02:30.66>spurs<02:31.23> <02:31.83>that<02:32.01> <02:32.13>jingle,<02:32.42> <02:32.73>jangle,<02:32.83> <02:32.94>jingle<02:33.04> <02:33.15>(I<02:33.16> <02:33.17>got<02:33.22> <02:33.26>spurs<02:33.77> <02:34.28>that<02:34.42> <02:34.55>jingle,<02:34.93> <02:35.30>jangle,<02:35.48> <02:35.66>jingle)
[02:35.69]<02:35.69>As<02:35.72> <02:35.75>I<02:35.82> <02:35.90>go<02:36.26> <02:36.63>ridin''<02:36.88> <02:37.13>merrily<02:37.58> <02:38.02>along<02:38.18> <02:38.34>(as<02:38.36> <02:38.37>I<02:38.40> <02:38.43>go<02:38.78> <02:39.14>ridin''<02:39.40> <02:39.67>merrily<02:39.92> <02:40.17>along)
[02:40.23]<02:40.23>And<02:40.29> <02:40.35>they<02:40.65> <02:41.43>sing,<02:41.52> <02:41.79>"Oh,<02:42.60> <02:42.66>ain''t<02:42.80> <02:42.93>you<02:42.98> <02:43.02>glad<02:43.08> <02:43.14>you''re<02:43.19> <02:43.23>single?"<02:43.32> <02:43.41>(And<02:43.42> <02:43.44>they<02:43.48> <02:43.53>sing,<02:44.01> <02:44.04>"Oh,<02:44.91> <02:44.94>ain''t<02:45.06> <02:45.18>you<02:45.28> <02:45.39>glad<02:45.62> <02:45.84>you''re<02:45.88> <02:45.93>single?")
[02:46.09]<02:46.09>And<02:46.21> <02:46.30>that<02:46.39> <02:46.48>song<02:46.76> <02:47.04>ain''t<02:47.15> <02:47.25>so<02:47.55> <02:47.61>very<02:47.83> <02:48.05>far<02:48.14> <02:48.23>from<02:48.28> <02:48.32>wrong<02:48.44> <02:48.56>(and<02:48.58> <02:48.59>that<02:48.77> <02:48.86>song<02:49.22> <02:49.57>ain''t<02:49.69> <02:49.81>so<02:49.98> <02:50.14>very<02:50.41> <02:50.68>far<02:50.90> <02:51.12>from<02:51.36> <02:52.82>wrong)
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (4722, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.42>I<00:00.84> <00:01.25>Just<00:01.67>)<00:02.09> <00:02.51>Died<00:02.93> <00:03.34>In<00:03.76> <00:04.18>Your<00:04.60> <00:05.02>Arms<00:05.43> <00:05.85>-<00:06.27> <00:06.69>Cutting<00:07.11> <00:07.52>Crew
[00:07.96]<00:07.96>Written<00:09.10> <00:10.23>by<00:11.37>：<00:12.51>Nicholas<00:13.64> <00:14.78>Eede
[00:15.93]<00:15.93>Oh <00:16.30>I <00:17.61>I <00:17.86>just <00:18.11>died <00:18.55>in <00:18.86>your <00:19.05>arms <00:19.42>tonight
[00:21.98]<00:21.98>It <00:22.23>must <00:22.48>have <00:22.73>been <00:23.17>something <00:23.42>you <00:23.92>said
[00:25.41]<00:25.41>I <00:25.60>just <00:25.85>died <00:26.22>in <00:26.54>your <00:26.72>arms <00:27.11>tonight
[00:39.87]<00:39.87>I <00:40.05>keep <00:40.24>looking <00:40.43>for <00:40.93>something <00:41.36>I <00:42.05>can''t <00:42.30>get
[00:43.55]<00:43.55>Broken <00:43.99>hearts <00:45.00>lie <00:45.43>all <00:45.68>around <00:45.93>me
[00:46.77]<00:46.77>And <00:47.02>I <00:47.40>don''t <00:47.65>see <00:48.21>an <00:48.96>easy <00:49.18>way
[00:50.49]<00:50.49>To <00:50.75>get <00:51.06>out <00:51.25>of <00:51.56>this
[00:54.65]<00:54.65>Her <00:54.87>diary <00:55.12>it <00:55.68>sits <00:55.87>on <00:56.43>the <00:56.93>bedside <00:57.74>table
[00:58.36]<00:58.36>The <00:58.80>curtains <00:59.17>are <00:59.80>closed
[01:00.11]<01:00.11>The <01:00.42>cats <01:00.73>in <01:00.98>the <01:01.23>cradle
[01:02.23]<01:02.23>Who <01:02.79>would''ve <01:03.35>thought <01:03.91>that
[01:04.23]<01:04.23>A <01:04.48>boy <01:04.91>like <01:05.10>me <01:06.04>could <01:06.23>come <01:06.67>to <01:07.10>this
[01:08.66]<01:08.66>Oh <01:10.23>I <01:11.30>I <01:11.55>just <01:11.86>died <01:12.29>in <01:12.54>your <01:12.79>arms <01:13.23>tonight
[01:15.91]<01:15.91>It <01:16.10>must''ve <01:16.35>been <01:16.60>something <01:17.04>you <01:17.29>said
[01:19.22]<01:19.22>I <01:19.41>just <01:19.66>died <01:19.97>in <01:20.34>your <01:20.59>arms <01:21.03>tonight
[01:23.53]<01:23.53>Oh <01:25.46>I <01:26.83>I <01:27.02>just <01:27.21>died <01:27.64>in <01:27.89>your <01:28.14>arms <01:28.58>tonight
[01:31.12]<01:31.12>It <01:31.31>must''ve <01:31.74>been <01:32.05>some <01:32.49>kind <01:32.74>of <01:33.12>kiss
[01:35.05]<01:35.05>I <01:35.24>should''ve <01:35.67>walked <01:36.61>away
[01:39.42]<01:39.42>I <01:39.67>should''ve <01:39.92>walked <01:40.29>away
[01:52.22]<01:52.22>Is <01:52.41>there <01:52.60>any <01:52.85>just <01:53.22>cause <01:53.66>for
[01:54.35]<01:54.35>Feeling <01:54.53>like <01:54.78>this
[01:56.67]<01:56.67>On <01:56.92>the <01:57.11>surface <01:57.67>I''m <01:57.85>a <01:58.17>name <01:58.54>on <01:58.79>a <01:59.17>list
[01:59.54]<01:59.54>I <01:59.79>try <02:00.23>to <02:00.91>be <02:01.16>discreet
[02:03.10]<02:03.10>But <02:03.47>then <02:03.72>blow <02:04.22>it <02:04.47>again
[02:07.21]<02:07.21>I''ve <02:07.46>lost <02:07.78>and <02:08.15>found
[02:09.21]<02:09.21>It''s <02:09.46>my <02:09.77>final <02:10.15>mistake
[02:11.02]<02:11.02>She''s <02:11.46>loving <02:11.89>by <02:12.27>proxy
[02:13.02]<02:13.02>No <02:13.52>give <02:13.77>and <02:14.02>all <02:14.39>take
[02:15.14]<02:15.14>''Cos <02:15.33>I''ve <02:15.83>been <02:16.14>thrilled <02:16.64>to
[02:17.14]<02:17.14>Fantasy <02:18.85>one <02:19.04>too <02:19.29>many <02:19.60>times
[02:21.66]<02:21.66>Oh <02:23.03>I <02:24.28>I <02:24.53>just <02:24.78>died <02:25.21>in <02:25.46>your <02:25.78>arms <02:26.34>tonight
[02:28.72]<02:28.72>It <02:28.97>must''ve <02:29.22>been <02:29.72>something <02:30.16>you <02:30.53>said
[02:32.28]<02:32.28>I <02:32.47>just <02:32.66>died <02:32.90>in <02:33.28>your <02:33.53>arms <02:34.03>tonight
[02:37.90]<02:37.90>Oh <02:38.52>I <02:39.89>I <02:40.08>just <02:40.33>died <02:40.70>in <02:40.95>your <02:41.20>arms <02:41.70>tonight
[02:44.20]<02:44.20>It <02:44.45>must''ve <02:44.76>been <02:45.13>some <02:45.51>kind <02:45.76>of <02:46.01>kiss
[02:48.00]<02:48.00>I <02:48.19>should''ve <02:48.63>walked <02:49.31>away
[02:52.37]<02:52.37>I <02:52.56>should''ve <02:52.81>walked <02:53.43>away
[02:55.30]<02:55.30>It <02:55.49>was <02:55.68>a <02:55.86>long <02:56.17>hot <02:56.61>night
[02:57.42]<02:57.42>And <02:57.67>she <02:58.05>made <02:58.42>it <02:58.80>easy
[02:59.23]<02:59.23>She <02:59.54>made <02:59.79>it <03:00.11>feel <03:00.42>right
[03:01.96]<03:01.96>But <03:02.15>now <03:02.40>it''s <03:02.77>over
[03:03.27]<03:03.27>The <03:03.65>moment <03:03.90>has <03:04.33>gone
[03:05.08]<03:05.08>I <03:05.39>followed <03:05.71>my <03:05.96>hands <03:06.21>not <03:06.70>my <03:07.02>head
[03:08.08]<03:08.08>I <03:08.33>know <03:08.64>I <03:08.89>was <03:09.20>wrong
[03:23.88]<03:23.88>Oh <03:24.25>I <03:25.87>I <03:26.12>just <03:26.37>died <03:26.81>in <03:27.06>your <03:27.50>arms <03:27.93>tonight
[03:30.33]<03:30.33>It <03:30.52>must''ve <03:30.70>been <03:31.14>something <03:31.64>you <03:32.02>said
[03:33.58]<03:33.58>I <03:33.76>just <03:34.01>died <03:34.45>in <03:34.70>your <03:34.95>arms <03:35.57>tonight
[03:39.76]<03:39.76>I <03:41.13>I <03:41.43>just <03:41.69>died <03:42.12>in <03:42.44>your <03:42.68>arms <03:43.18>tonight
[03:45.44]<03:45.44>It <03:45.75>must''ve <03:46.00>been <03:46.50>some <03:46.93>kind <03:47.18>of <03:47.62>kiss
[03:49.55]<03:49.55>I <03:49.74>should''ve <03:50.24>walked <03:50.98>away
[03:53.85]<03:53.85>I <03:54.04>should''ve <03:54.23>walked <03:54.48>away
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (2195, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.11>I''m<00:00.22> <00:00.33>Not<00:00.44> <00:00.55>Your<00:00.66>)<00:00.77> <00:00.88>Steppin''<00:00.99> <00:01.10>Stone<00:01.21> <00:01.32>-<00:01.43> <00:01.54>The<00:01.65> <00:01.76>Monkees<00:01.87> <00:01.98>(<00:02.09>顽<00:02.20>童<00:02.31>合<00:02.42>唱<00:02.53>团<00:02.64>)
[00:02.77]<00:02.77>Lyrics<00:03.02> <00:03.27>by<00:03.52>：<00:03.77>Bobby<00:04.02> <00:04.27>Hart<00:04.52>/<00:04.78>Tommy<00:05.03> <00:05.28>Boyce
[00:05.53]<00:05.53>Composed<00:05.79> <00:06.04>by<00:06.29>：<00:06.54>Bobby<00:06.79> <00:07.04>Hart<00:07.29>/<00:07.54>Tommy<00:07.79> <00:08.04>Boyce
[00:08.30]<00:08.30>I <00:08.65>I <00:09.07>I <00:09.52>I <00:10.00>I''m <00:10.46>not <00:10.69>your <00:10.96>steppin'' <00:11.61>stone
[00:15.46]<00:15.46>I <00:15.90>I <00:16.33>I <00:16.75>I <00:17.25>I''m <00:17.73>not <00:17.92>your <00:18.19>steppin'' <00:18.93>stone
[00:22.42]<00:22.42>You''re <00:22.66>tryin'' <00:22.89>to <00:23.09>make <00:23.37>your <00:23.56>mark <00:24.05>in <00:24.23>society
[00:26.14]<00:26.14>You''re <00:26.34>usin'' <00:26.79>all <00:27.02>the <00:27.25>tricks <00:27.73>that <00:27.96>you <00:28.18>used <00:28.47>on <00:28.97>me
[00:29.85]<00:29.85>You''re <00:30.05>readin'' <00:30.50>all <00:30.73>them <00:30.95>high-<00:31.41>fashion <00:31.86>magazines
[00:33.48]<00:33.48>The <00:33.72>clothes <00:33.96>you''re <00:34.22>wearin'' <00:34.72>girl <00:34.93>are <00:35.15>causin'' <00:35.59>public <00:36.26>scenes
[00:36.82]<00:36.82>I <00:36.99>said <00:37.39>I <00:37.84>I <00:38.26>I <00:38.74>I <00:39.17>I''m <00:39.60>not <00:39.84>your <00:40.07>steppin'' <00:40.77>stone
[00:44.63]<00:44.63>I <00:45.01>I <00:45.44>I <00:45.87>I <00:46.32>I''m <00:46.85>not <00:47.06>your <00:47.27>steppin'' <00:47.99>stone
[00:54.55]<00:54.55>Not <00:54.76>your <00:54.97>steppin'' <00:55.38>stone
[00:58.34]<00:58.34>Not <00:58.52>your <00:58.71>steppin'' <00:59.18>stone
[01:10.10]<01:10.10>I <01:10.55>I <01:10.98>I <01:11.45>I <01:11.88>I''m <01:12.36>not <01:12.61>your <01:12.85>steppin'' <01:13.48>stone
[01:17.19]<01:17.19>When <01:17.41>I <01:17.65>first <01:17.86>met <01:18.05>you <01:18.32>girl <01:18.55>you <01:18.79>didn''t <01:19.30>have <01:19.51>no <01:19.98>shoes
[01:20.84]<01:20.84>But <01:21.06>now <01:21.29>you''re <01:21.51>walkin'' <01:22.03>''round <01:22.49>like <01:22.70>you''re <01:22.91>front <01:23.18>page <01:23.57>news
[01:24.65]<01:24.65>You''ve <01:24.89>been <01:25.11>awful <01:25.58>careful <01:26.01>''bout <01:26.22>the <01:26.46>friends <01:26.73>you <01:27.17>choose
[01:28.16]<01:28.16>But <01:28.35>you <01:28.56>won''t <01:28.78>find <01:28.99>my <01:29.23>name <01:29.65>in <01:29.90>your <01:30.13>book <01:30.37>of <01:30.62>Who''s <01:31.08>Who
[01:31.25]<01:31.25>I <01:31.46>said <01:31.94>I <01:32.37>I <01:32.79>I <01:33.23>I <01:33.76>I''m <01:34.19>not <01:34.40>your <01:34.65>steppin'' <01:35.31>stone
[01:36.86]<01:36.86>No <01:37.10>girl <01:37.35>not <01:37.54>me
[01:39.24]<01:39.24>I <01:39.60>I <01:39.99>I <01:40.40>I <01:40.85>I''m <01:41.35>not <01:41.60>your <01:41.84>steppin'' <01:42.56>stone
[01:49.12]<01:49.12>Not <01:49.37>your <01:49.60>steppin'' <01:50.02>stone
[01:52.66]<01:52.66>I''m <01:52.86>not <01:53.07>your <01:53.30>steppin'' <01:53.73>stone
[02:03.74]<02:03.74>Not <02:04.00>your <02:04.18>steppin'' <02:04.59>stone
[02:05.58]<02:05.58>Not <02:05.79>your <02:06.01>steppin'' <02:06.41>stone
[02:07.37]<02:07.37>Not <02:07.63>your <02:07.85>steppin'' <02:08.24>stone
[02:09.20]<02:09.20>Not <02:09.44>your <02:09.63>steppin'' <02:10.11>stone
[02:11.29]<02:11.29>Not <02:11.76>your <02:12.36>steppin'' <02:13.01>stone
[02:14.09]<02:14.09>Not <02:14.46>your <02:14.70>steppin'' <02:15.09>stone
[02:15.90]<02:15.90>Not <02:16.32>your <02:16.54>steppin'' <02:16.92>stone
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (6233, 'lrc', 'line', 'local_lrc', '[00:00.24]<00:00.24>(<00:00.40>If) <00:00.56>You <00:00.67>Want <00:00.80>Trouble <00:00.94>- <00:01.08>Nick <00:01.22>Waterhouse
[00:01.38]<00:01.38>Written <00:01.53>by<00:01.69>：<00:01.87>Nicholas <00:02.02>Waterhouse
[00:40.77]<00:40.77>If <00:40.94>you <00:41.07>want <00:41.20>trouble
[00:42.95]<00:42.95>You <00:43.10>got <00:43.22>it
[00:44.05]<00:44.05>Ooh <00:44.70>ooh
[00:45.95]<00:45.95>Said <00:46.46>you <00:46.60>been <00:46.75>thinking <00:46.96>all <00:47.80>night
[00:48.72]<00:48.72>About <00:48.86>it
[00:49.78]<00:49.78>Ooh <00:50.34>ooh
[00:51.54]<00:51.54>Well <00:51.72>if <00:51.88>you <00:52.03>look
[00:53.73]<00:53.73>You <00:53.93>know <00:54.08>where <00:54.54>I&apos;ll <00:54.70>be
[00:55.74]<00:55.74>Ooh <00:56.54>ooh
[00:57.30]<00:57.30>It&apos;s <00:57.48>the <00:57.61>last <00:58.19>place <00:58.56>that
[00:59.67]<00:59.67>You <00:59.82>might <01:00.01>have <01:00.17>seen <01:00.32>me
[01:01.03]<01:01.03>Ooh <01:01.65>ooh
[01:14.53]<01:14.53>Oh <01:14.72>if <01:14.93>you <01:15.09>want <01:15.53>trouble <01:16.83>trouble
[01:20.57]<01:20.57>Boy <01:20.79>if <01:21.05>you <01:21.21>want <01:21.36>trouble
[01:23.35]<01:23.35>Say <01:23.66>if <01:23.84>you <01:24.01>want <01:24.29>trouble <01:26.15>if <01:26.68>you <01:26.89>want <01:27.07>trouble
[01:29.47]<01:29.47>Well <01:29.65>if <01:29.84>you <01:30.00>want <01:30.18>trouble
[01:32.08]<01:32.08>And <01:32.25>if <01:32.40>you <01:32.63>want <01:32.85>trouble <01:34.29>trouble
[01:35.31]<01:35.31>Well <01:35.67>if <01:35.84>you <01:36.01>want <01:36.16>trouble
[02:01.25]<02:01.25>Well <02:01.43>you <02:01.57>look <02:01.69>there
[02:03.97]<02:03.97>Tell <02:04.17>me <02:04.66>what&apos;s <02:04.82>best <02:05.21>yeah
[02:05.80]<02:05.80>Ooh <02:06.19>ooh
[02:06.84]<02:06.84>She <02:07.06>said <02:07.45>you <02:07.61>been <02:07.76>thinking <02:08.18>that <02:08.31>I
[02:09.66]<02:09.66>Like <02:09.81>it
[02:10.74]<02:10.74>Ooh <02:11.21>ooh
[02:12.59]<02:12.59>Please <02:12.75>don&apos;t <02:12.99>come <02:13.17>a <02:13.32>looking
[02:14.73]<02:14.73>I <02:14.91>know <02:15.42>where <02:15.59>I&apos;ll <02:15.72>be
[02:16.73]<02:16.73>Ooh <02:17.11>ooh
[02:18.40]<02:18.40>Oh <02:18.63>it&apos;s <02:19.15>the <02:19.36>last <02:19.52>place <02:19.78>that
[02:20.48]<02:20.48>You <02:20.64>might <02:20.82>have <02:20.96>seen <02:21.40>me
[02:22.20]<02:22.20>Ooh <02:22.58>ooh
[02:22.96]<02:22.96>Oh
[02:55.34]<02:55.34>If <02:55.64>you <02:55.84>want <02:55.99>trouble <02:56.87>trouble
[02:58.13]<02:58.13>Well <02:58.30>if <02:58.50>you <02:58.72>want <02:58.91>trouble
[02:59.97]<02:59.97>If <03:00.18>if <03:00.45>you <03:00.69>want <03:01.33>trouble <03:01.86>yeah <03:02.33>trouble
[03:03.33]<03:03.33>If <03:03.55>you <03:03.72>want <03:04.11>trouble
[03:06.13]<03:06.13>If <03:06.30>if <03:06.46>you <03:06.63>want <03:06.80>trouble <03:08.37>trouble
[03:09.07]<03:09.07>Said <03:09.23>you <03:09.43>want <03:09.63>trouble
[03:11.88]<03:11.88>Oh <03:12.09>if <03:12.30>you <03:12.48>want <03:12.67>trouble <03:14.17>trouble
[03:15.34]<03:15.34>If <03:15.52>you <03:15.66>want <03:15.81>trouble
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (5073, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.21>Just<00:00.42> <00:00.64>Like<00:00.85>)<00:01.06> <00:01.27>Starting<00:01.48> <00:01.70>Over<00:01.91> <00:02.12>(<00:02.33>Ultimate<00:02.54> <00:02.76>Mix<00:02.97>)<00:03.18> <00:03.39>-<00:03.60> <00:03.82>John<00:04.03> <00:04.24>Lennon<00:04.45> <00:04.66>(<00:04.88>约<00:05.09>翰<00:05.30>·<00:05.51>列<00:05.72>侬<00:05.94>)
[00:06.17]<00:06.17>Our <00:06.40>life <00:08.12>together <00:09.96>is <00:10.13>so <00:10.54>precious <00:11.93>together
[00:14.54]<00:14.54>We <00:14.67>have <00:14.81>grown  <00:18.91>we <00:19.08>have <00:19.37>grown
[00:22.93]<00:22.93>Although <00:23.51>our <00:23.95>love <00:26.08>is <00:26.23>still <00:26.51>special
[00:30.95]<00:30.95>Let''s <00:31.54>take <00:32.20>a <00:32.34>chance <00:33.47>and
[00:34.00]<00:34.00>Fly <00:34.33>away <00:36.78>somewhere <00:39.97>alone
[00:43.20]<00:43.20>It''s <00:43.38>been <00:43.64>too <00:43.81>long <00:44.00>since <00:44.17>we <00:44.37>took <00:44.52>the <00:44.67>time
[00:45.32]<00:45.32>No-one''s <00:45.49>to <00:45.77>blame  <00:46.03>I <00:46.41>know <00:46.70>time <00:46.86>flies <00:47.20>so <00:47.69>quickly
[00:53.47]<00:53.47>But <00:53.89>when <00:54.13>I <00:54.73>see <00:55.50>you <00:56.04>darling
[00:58.12]<00:58.12>It''s <00:58.56>like <00:59.03>we <00:59.75>both <01:00.37>are <01:00.87>falling <01:02.32>in <01:02.64>love <01:03.14>again
[01:04.50]<01:04.50>It''ll <01:04.66>be <01:05.28>just <01:05.85>like <01:06.33>starting <01:06.57>over  <01:11.68>starting <01:12.02>over
[01:16.66]<01:16.66>Everyday <01:17.29>we <01:17.48>used <01:17.73>to <01:17.95>make <01:18.26>it <01:18.45>love
[01:19.05]<01:19.05>Why <01:19.21>can''t <01:19.74>we <01:20.14>be <01:20.43>making <01:21.08>love <01:21.35>nice <01:21.79>and <01:21.95>easy
[01:27.40]<01:27.40>It''s <01:27.63>time <01:28.09>to <01:28.63>spread <01:29.48>our <01:30.04>wings <01:30.55>and <01:30.97>fly
[01:31.95]<01:31.95>Don''t <01:32.29>let <01:32.86>another <01:34.64>day <01:35.45>go <01:35.92>by <01:36.56>my <01:37.10>love
[01:38.19]<01:38.19>It''ll <01:38.39>be <01:39.15>just <01:39.63>like <01:40.02>starting <01:40.70>over  <01:45.45>starting <01:45.76>over
[01:51.28]<01:51.28>Why <01:51.45>don''t <01:51.63>we <01:52.03>take <01:52.30>off <01:52.56>alone
[01:56.40]<01:56.40>Take <01:56.59>a <01:56.97>trip <01:57.32>somewhere <01:57.80>far  <01:58.31>far <01:58.56>away
[02:01.02]<02:01.02>We''ll <02:01.20>be <02:01.39>together <02:02.54>all <02:02.76>alone <02:03.61>again
[02:06.28]<02:06.28>Like <02:06.49>we <02:06.76>used <02:06.99>to <02:07.45>in <02:07.69>the <02:08.22>early <02:08.41>days
[02:10.44]<02:10.44>Well  <02:10.79>well  <02:11.25>well <02:11.71>darling
[02:12.44]<02:12.44>It''s <02:12.64>been <02:12.83>too <02:13.08>long <02:13.39>since <02:13.68>we <02:13.88>took <02:14.09>the <02:14.27>time
[02:15.12]<02:15.12>No-one''s <02:15.30>to <02:15.52>blame  <02:16.47>I <02:16.63>know <02:16.80>time <02:16.98>flies <02:17.18>so <02:17.37>quickly
[02:23.05]<02:23.05>But <02:23.22>when <02:23.70>I <02:24.48>see <02:24.98>you <02:25.65>darling
[02:27.50]<02:27.50>It''s <02:28.08>like <02:28.63>we <02:29.30>both <02:29.89>are <02:30.55>falling <02:31.51>in <02:31.73>love <02:32.35>again
[02:34.11]<02:34.11>It''ll <02:34.30>be <02:35.06>just <02:35.43>like <02:35.87>starting <02:36.59>over  <02:41.39>starting <02:41.59>over
[02:46.20]<02:46.20>Our <02:46.36>life <02:48.51>together <02:50.76>is <02:51.07>so <02:51.25>precious <02:53.42>together
[02:55.79]<02:55.79>We <02:55.96>have <02:56.12>grown  <03:00.88>we <03:01.08>have <03:01.25>grown
[03:05.21]<03:05.21>Although <03:05.71>our <03:06.00>love <03:07.40>is <03:07.77>still <03:08.22>special
[03:13.17]<03:13.17>Let''s <03:13.65>take <03:13.89>a <03:14.21>chance <03:14.98>and <03:15.40>fly <03:15.71>away <03:17.46>somewhere
[03:35.60]<03:35.60>Starting <03:35.86>over
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (7216, 'lrc', 'line', 'local_lrc', '[00:03.25]<00:03.25>Baby <00:03.51>let <00:03.90>me <00:04.27>be
[00:05.89]<00:05.89>Your <00:06.14>lovin'' <00:06.88>Teddy <00:07.13>Bear
[00:08.76]<00:08.76>Put <00:09.07>a <00:09.32>chain <00:09.88>around <00:10.13>my <00:10.44>neck
[00:11.32]<00:11.32>And <00:11.63>lead <00:11.88>me <00:12.26>anywhere
[00:13.37]<00:13.37>Oh <00:13.61>let <00:13.86>me <00:14.05>be
[00:15.60]<00:15.60>Your <00:15.96>teddy <00:16.52>bear
[00:19.52]<00:19.52>I <00:19.77>don''t <00:20.02>wanna <00:20.27>be <00:20.65>a <00:20.96>tiger
[00:22.09]<00:22.09>''Cause <00:22.34>tigers <00:22.77>play <00:23.46>too <00:23.71>rough
[00:24.71]<00:24.71>I <00:25.02>don''t <00:25.21>wanna <00:25.59>be <00:25.90>a <00:26.15>lion
[00:27.46]<00:27.46>''Cause <00:27.71>lions <00:27.96>ain''t <00:28.34>the <00:28.59>kind
[00:29.40]<00:29.40>You <00:29.71>love <00:29.96>enough
[00:32.21]<00:32.21>But <00:32.46>I <00:32.65>just <00:32.90>wanna <00:33.21>be
[00:34.59]<00:34.59>Your <00:35.27>Teddy <00:35.52>Bear
[00:38.49]<00:38.49>Put <00:38.74>a <00:38.93>chain <00:39.43>around <00:39.81>my <00:40.12>neck
[00:40.99]<00:40.99>And <00:41.24>lead <00:41.48>me <00:41.79>anywhere
[00:42.85]<00:42.85>Oh <00:43.16>let <00:43.42>me <00:43.79>be
[00:45.13]<00:45.13>Your <00:45.88>teddy <00:46.26>bear
[00:49.32]<00:49.32>Baby <00:49.63>let <00:50.07>me <00:50.44>be
[00:51.95]<00:51.95>Around <00:52.20>you <00:52.51>every <00:53.01>night
[00:54.57]<00:54.57>Run <00:54.82>your <00:55.13>fingers <00:55.80>through <00:56.24>my <00:56.49>hair
[00:57.31]<00:57.31>And <00:57.56>cuddle <00:57.80>me <00:58.18>real <00:58.43>tight
[00:59.49]<00:59.49>Oh <00:59.74>let <00:59.99>me <01:00.24>be
[01:01.74]<01:01.74>Your <01:02.24>teddy <01:02.56>bear
[01:05.37]<01:05.37>I <01:05.68>don''t <01:05.99>wanna <01:06.24>be <01:06.49>a <01:06.74>tiger
[01:07.99]<01:07.99>''Cause <01:08.31>tigers <01:08.56>play <01:08.81>too <01:09.18>rough
[01:10.62]<01:10.62>I <01:10.87>don''t <01:11.18>wanna <01:11.68>be <01:11.99>a <01:12.24>lion
[01:13.24]<01:13.24>''Cause <01:13.49>lions <01:13.87>ain''t <01:14.18>the <01:14.43>kind
[01:14.90]<01:14.90>You <01:15.59>love <01:15.90>enough
[01:17.90]<01:17.90>But <01:18.15>I <01:18.33>just <01:18.59>wanna <01:18.77>be
[01:20.40]<01:20.40>Your <01:21.09>Teddy <01:21.46>Bear
[01:24.27]<01:24.27>Put <01:24.66>a <01:24.91>chain <01:25.35>around <01:25.79>my <01:26.04>neck
[01:26.98]<01:26.98>And <01:27.22>lead <01:27.41>me <01:27.85>anywhere
[01:28.62]<01:28.62>Oh <01:28.93>let <01:29.18>me <01:29.37>be
[01:30.91]<01:30.91>Your <01:32.09>teddy <01:32.34>bear
[01:33.96]<01:33.96>Oh <01:34.27>let <01:34.53>me <01:34.77>be
[01:36.63]<01:36.63>Your <01:36.94>teddy <01:37.26>bear
[01:40.38]<01:40.38>I <01:40.57>just <01:40.82>wanna <01:41.00>be <01:41.26>your <01:41.82>teddy <01:42.13>bear
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (7689, 'lrc', 'line', 'local_lrc', '[00:06.22]<00:06.22>A<00:06.24> <00:06.27>very<00:06.28> <00:06.64>old<00:06.65> <00:06.85>friend<00:07.17> <00:08.20>came<00:08.22> <00:08.56>by<00:08.57> <00:08.85>today
[00:10.73]<00:10.73>''Cause<00:10.75> <00:11.10>he<00:11.11> <00:11.33>was<00:11.36> <00:11.61>telling<00:11.65> <00:12.26>everyone<00:12.31> <00:12.98>in<00:12.99> <00:13.13>town
[00:13.63]<00:13.63>Of<00:13.63> <00:14.02>the<00:14.05> <00:14.14>love<00:14.16> <00:14.47>that<00:14.49> <00:14.71>he<00:14.73> <00:14.96>just<00:14.97> <00:15.30>found
[00:16.38]<00:16.38>And<00:16.38> <00:16.60>Marie''s<00:16.62> <00:17.07>the<00:17.07> <00:17.14>name<00:17.60> <00:18.69>of<00:18.77> <00:18.92>his<00:18.93> <00:19.05>latest<00:19.08> <00:19.59>flame
[00:23.05]<00:23.05>He<00:23.06> <00:23.29>talked<00:23.30> <00:23.74>and<00:23.75> <00:23.93>talked<00:24.40> <00:25.23>and<00:25.34> <00:25.63>I<00:25.64> <00:25.67>heard<00:25.70> <00:26.00>him<00:26.01> <00:26.21>say
[00:28.09]<00:28.09>That<00:28.11> <00:28.35>she<00:28.36> <00:28.64>had<00:28.67> <00:29.11>the<00:29.12> <00:29.24>longest,<00:29.28> <00:29.82>blackest<00:29.84> <00:30.29>hair
[00:30.94]<00:30.94>The<00:30.95> <00:31.03>prettiest<00:31.05> <00:31.47>green<00:31.49> <00:31.88>eyes<00:31.92> <00:32.25>anywhere
[00:33.50]<00:33.50>And<00:33.51> <00:33.70>Marie''s<00:33.72> <00:34.16>the<00:34.20> <00:34.26>name<00:34.73> <00:35.82>of<00:35.89> <00:36.03>his<00:36.05> <00:36.17>latest<00:36.19> <00:36.72>flame
[00:41.37]<00:41.37>Though<00:41.39> <00:41.56>I<00:41.56> <00:41.69>smiled,<00:41.71> <00:42.24>the<00:42.27> <00:42.36>tears<00:42.39> <00:42.82>inside<00:42.85> <00:43.33>were<00:43.33> <00:43.58>burning
[00:45.58]<00:45.58>I<00:45.72> <00:45.86>wished<00:45.90> <00:46.25>him<00:46.26> <00:46.45>luck<00:46.48> <00:46.82>and<00:46.84> <00:47.06>then<00:47.08> <00:47.26>he<00:47.27> <00:47.41>said<00:47.43> <00:47.91>goodbye
[00:50.38]<00:50.38>He<00:50.39> <00:50.68>was<00:50.69> <00:50.86>gone<00:50.88> <00:51.22>but<00:51.24> <00:51.42>still<00:51.43> <00:51.74>his<00:51.76> <00:51.93>words<00:51.96> <00:52.32>kept<00:52.34> <00:52.59>returning
[00:54.30]<00:54.30>What<00:54.32> <00:54.50>else<00:54.51> <00:54.81>was<00:54.83> <00:55.14>there<00:55.16> <00:55.42>for<00:55.44> <00:55.64>me<00:55.65> <00:55.91>to<00:55.92> <00:56.07>do<00:56.08> <00:56.56>but<00:56.57> <00:56.71>cry?
[01:01.03]<01:01.03>Would<01:01.06> <01:01.36>you<01:01.38> <01:01.69>believe<01:02.60> <01:03.38>that<01:03.42> <01:03.66>yesterday
[01:05.27]<01:05.27>This<01:05.29> <01:05.55>girl<01:05.56> <01:05.83>was<01:05.84> <01:06.17>in<01:06.17> <01:06.46>my<01:06.47> <01:06.64>arms<01:06.66> <01:07.03>and<01:07.05> <01:07.16>swore<01:07.18> <01:07.54>to<01:07.55> <01:07.66>me
[01:08.06]<01:08.06>She''d<01:08.07> <01:08.52>be<01:08.54> <01:08.66>mine<01:08.69> <01:09.05>eternally
[01:10.77]<01:10.77>And<01:10.78> <01:10.96>Marie''s<01:10.97> <01:11.38>the<01:11.41> <01:11.50>name<01:11.98> <01:13.07>of<01:13.13> <01:13.29>his<01:13.30> <01:13.41>latest<01:13.44> <01:13.94>flame
[01:18.81]<01:18.81>Though<01:18.85> <01:19.05>I<01:19.07> <01:19.19>smiled,<01:19.22> <01:19.73>the<01:19.75> <01:19.81>tears<01:19.84> <01:20.29>inside<01:20.32> <01:20.85>were<01:20.86> <01:21.03>a-<01:21.12>burning
[01:22.64]<01:22.64>I<01:22.77> <01:22.89>wished<01:22.92> <01:23.32>him<01:23.34> <01:23.48>luck<01:23.50> <01:23.89>and<01:23.91> <01:24.14>then<01:24.16> <01:24.31>he<01:24.32> <01:24.43>said<01:24.45> <01:24.97>goodbye
[01:27.92]<01:27.92>He<01:27.94> <01:28.20>was<01:28.21> <01:28.40>gone<01:28.42> <01:28.75>but<01:28.77> <01:28.95>still<01:28.97> <01:29.28>his<01:29.29> <01:29.46>words<01:29.48> <01:29.86>kept<01:29.86> <01:30.06>returning
[01:32.27]<01:32.27>What<01:32.29> <01:32.49>else<01:32.51> <01:32.79>was<01:32.80> <01:33.16>there<01:33.18> <01:33.41>for<01:33.43> <01:33.63>me<01:33.64> <01:33.90>to<01:33.91> <01:34.06>do<01:34.07> <01:34.55>but<01:34.57> <01:34.71>cry?
[01:38.61]<01:38.61>Would<01:38.64> <01:38.84>you<01:38.84> <01:39.23>believe<01:40.28> <01:40.84>that<01:40.85> <01:41.15>yesterday
[01:42.75]<01:42.75>This<01:42.78> <01:43.04>girl<01:43.04> <01:43.35>was<01:43.37> <01:43.71>in<01:43.71> <01:44.00>my<01:44.02> <01:44.19>arms<01:44.21> <01:44.56>and<01:44.58> <01:44.70>swore<01:44.72> <01:45.10>to<01:45.11> <01:45.20>me
[01:46.18]<01:46.18>She''d<01:46.19> <01:46.65>be<01:46.66> <01:46.78>mine<01:46.80> <01:47.16>eternally
[01:48.43]<01:48.43>And<01:48.44> <01:48.60>Marie''s<01:48.63> <01:49.06>the<01:49.08> <01:49.13>name<01:49.55> <01:50.75>of<01:50.76> <01:50.91>his<01:50.92> <01:51.04>latest<01:51.06> <01:51.56>flame
[01:53.29]<01:53.29>Yeah,<01:53.32> <01:53.52>Marie''s<01:53.54> <01:54.02>the<01:54.03> <01:54.08>name<01:54.60> <01:55.68>of<01:55.69> <01:55.85>his<01:55.87> <01:55.96>latest<01:55.99> <01:56.50>flame
[01:57.85]<01:57.85>Oh,<01:57.86> <01:58.07>Marie''s<01:58.09> <01:58.58>the<01:58.58> <01:58.64>name<01:59.11> <02:00.14>of<02:00.14> <02:00.42>his<02:00.45> <02:00.53>latest<02:00.56> <02:01.06>flame
[02:02.16]<02:02.16>And<02:02.17> <02:02.31>Marie''s<02:02.34> <02:02.78>the<02:02.82> <02:02.91>name<02:03.18> <02:04.39>of<02:04.39> <02:04.63>his<02:04.64> <02:04.72>latest<02:04.75> <02:05.25>flame
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (8287, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.29>Now<00:00.59> <00:00.89>and<00:01.18> <00:01.48>Then<00:01.77> <00:02.06>There''s<00:02.36>)<00:02.65> <00:02.95>A<00:03.25> <00:03.54>Fool<00:03.83> <00:04.13>Such<00:04.42> <00:04.72>as<00:05.01> <00:05.31>I<00:05.61> <00:05.90>-<00:06.20> <00:06.49>Elvis<00:06.79> <00:07.08>Presley<00:07.38> <00:07.67>(<00:07.96>猫<00:08.26>王<00:08.55>)
[00:08.88]<00:08.88>(Now <00:09.12>and <00:09.63>then <00:09.88>there''s <00:10.50>a <00:10.76>fool <00:11.00>such <00:11.44>as <00:12.00>I)
[00:16.69]<00:16.69>Pardon <00:17.25>me  <00:18.00>if <00:18.25>I''m <00:18.56>sentimental
[00:21.44]<00:21.44>When <00:21.75>we <00:21.94>say <00:22.19>goodbye
[00:24.32]<00:24.32>Don''t <00:24.56>be <00:24.82>angry <00:25.82>with <00:26.06>me <00:27.50>should <00:27.82>I <00:28.06>cry
[00:31.69]<00:31.69>When <00:31.94>you''re <00:32.19>gone  <00:33.19>yet <00:33.44>I''ll <00:33.69>dream
[00:35.25]<00:35.25>A <00:35.50>little <00:36.25>dream <00:36.69>as <00:36.94>years <00:37.56>go <00:37.81>by
[00:39.44]<00:39.44>Now <00:39.69>and <00:40.31>then <00:41.06>there''s <00:41.31>a <00:41.50>fool <00:41.88>such <00:42.81>as <00:43.06>I
[00:46.76]<00:46.76>Now <00:47.25>and <00:47.50>then <00:47.81>there''s <00:48.81>a <00:49.07>fool <00:50.57>such <00:50.88>as <00:51.19>I <00:51.75>am <00:52.50>over <00:52.76>you
[00:54.81]<00:54.81>You <00:55.13>taught <00:55.44>me <00:55.81>how <00:56.57>to <00:56.88>love
[00:57.88]<00:57.88>And <00:58.19>now <00:58.44>you <00:58.76>say <00:59.26>that <00:59.50>we <01:00.06>are <01:00.32>through
[01:02.00]<01:02.00>I''m <01:02.25>a <01:02.50>fool  <01:03.81>but <01:04.13>I''ll <01:04.44>love <01:04.76>you <01:05.00>dear
[01:06.25]<01:06.25>Until <01:06.69>the <01:07.25>day <01:07.57>I <01:07.88>die
[01:09.63]<01:09.63>Now <01:09.88>and <01:10.32>then <01:11.25>there''s <01:11.69>a <01:11.88>fool <01:13.25>such <01:13.50>as <01:13.69>I
[01:41.13]<01:41.13>(Now <01:41.38>and <01:41.75>then <01:42.44>there''s <01:42.69>a <01:42.94>fool <01:43.32>such <01:43.57>as <01:43.76>I)
[01:47.01]<01:47.01>Now <01:47.63>and <01:47.88>then <01:48.26>there''s <01:49.32>a <01:49.57>fool <01:50.94>such <01:51.19>as <01:51.44>I <01:52.00>am <01:52.88>over <01:53.19>you
[01:54.82]<01:54.82>You <01:55.07>taught <01:55.38>me <01:56.07>how <01:56.32>to <01:56.63>love
[01:59.07]<01:59.07>And <01:59.32>now <01:59.57>you <01:59.81>say <02:00.07>that <02:00.32>we <02:00.57>are <02:00.75>through
[02:02.32]<02:02.32>I''m <02:02.57>a <02:02.76>fool  <02:04.26>but <02:04.51>I''ll <02:04.76>love <02:05.26>you <02:05.44>dear
[02:05.88]<02:05.88>Until <02:06.38>the <02:06.64>day <02:07.57>I <02:08.01>die
[02:10.01]<02:10.01>Now <02:10.13>and <02:10.32>then <02:11.57>there''s <02:11.76>a <02:11.95>fool <02:12.14>such <02:13.26>as <02:13.51>I
[02:16.13]<02:16.13>Now <02:17.01>and <02:17.25>then <02:19.13>there''s <02:19.32>a <02:19.94>fool <02:20.70>such <02:20.95>as <02:21.20>I
[02:24.76]<02:24.76>Now <02:25.01>and <02:25.26>then <02:26.32>there''s <02:26.51>a <02:26.76>fool <02:27.57>such <02:27.82>as <02:28.07>I
[02:28.32]<02:28.32>END
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (4869, 'lrc', 'line', 'local_lrc', '[00:00.71]<00:00.71>Elvis <00:00.96>Presley <00:01.27>- <00:01.53>(You''re <00:01.77>The) <00:02.02>Devil <00:02.40>In <00:02.71>Disguise
[00:04.53]<00:04.53>You <00:04.84>look <00:05.15>like <00:05.46>an <00:05.77>angel
[00:08.71]<00:08.71>Walk <00:08.96>like <00:09.28>an <00:09.59>angel
[00:12.65]<00:12.65>Talk <00:12.89>like <00:13.21>an <00:13.65>angel
[00:15.27]<00:15.27>But <00:15.59>I <00:16.02>got <00:16.53>wise
[00:19.03]<00:19.03>You''re <00:19.40>the <00:19.71>devil <00:20.09>in <00:20.46>disguise
[00:21.96]<00:21.96>Oh <00:22.27>yes <00:22.52>you <00:22.89>are
[00:23.71]<00:23.71>The <00:23.96>devil <00:24.21>in <00:24.52>disguise
[00:25.46]<00:25.46>You <00:25.84>fooled <00:29.09>me <00:29.84>with <00:30.21>your <00:30.71>kisses
[00:32.52]<00:32.52>You <00:32.96>cheated <00:33.46>and <00:33.90>you <00:34.34>schemed
[00:36.15]<00:36.15>Heaven <00:36.52>knows <00:37.02>how <00:37.53>you <00:37.96>lied <00:38.46>to <00:38.84>me
[00:40.08]<00:40.08>You''re <00:40.40>not <00:40.77>the <00:41.02>way <00:41.34>you <00:41.71>seemed
[00:43.96]<00:43.96>You <00:44.27>look <00:44.59>like <00:44.90>an <00:45.21>angel
[00:48.10]<00:48.10>Walk <00:48.35>like <00:48.66>an <00:48.98>angel
[00:51.98]<00:51.98>Talk <00:52.29>like <00:52.54>an <00:52.91>angel
[00:54.60]<00:54.60>But <00:54.91>I <00:55.35>got <00:55.98>wise
[00:58.41]<00:58.41>You''re <00:58.85>the <00:59.10>devil <00:59.48>in <00:59.91>disguise
[01:01.35]<01:01.35>Oh <01:01.66>yes <01:01.98>you <01:02.29>are
[01:03.16]<01:03.16>The <01:03.48>devil <01:03.73>in <01:03.98>disguise
[01:05.16]<01:05.16>I <01:05.48>thought <01:08.60>that <01:09.10>I <01:09.54>was <01:09.91>in <01:10.35>heaven
[01:12.22]<01:12.22>But <01:12.54>I <01:12.91>was <01:13.35>sure <01:13.66>surprised
[01:15.54]<01:15.54>Heaven <01:15.85>help <01:16.29>me  <01:17.29>I <01:17.60>didn''t <01:17.91>see
[01:20.79]<01:20.79>The <01:21.04>devil <01:21.29>in <01:21.54>your <01:21.79>eyes
[01:23.35]<01:23.35>You <01:23.60>look <01:23.91>like <01:24.16>an <01:24.60>angel
[01:27.42]<01:27.42>Walk <01:27.66>like <01:27.98>an <01:28.29>angel
[01:31.41]<01:31.41>Talk <01:31.73>like <01:32.04>an <01:32.35>angel
[01:33.85]<01:33.85>But <01:34.23>I <01:34.79>got <01:35.29>wise
[01:37.73]<01:37.73>You''re <01:38.10>the <01:38.48>devil <01:38.85>in <01:39.35>disguise
[01:40.60]<01:40.60>Oh <01:40.98>yes <01:41.29>you <01:41.60>are
[01:42.41]<01:42.41>The <01:42.73>devil <01:43.04>in <01:43.35>disguise
[01:44.54]<01:44.54>You''re <02:01.29>the <02:01.54>devil <02:01.92>in <02:02.29>disguise
[02:03.79]<02:03.79>Oh <02:04.10>yes <02:04.42>you <02:04.73>are
[02:05.60]<02:05.60>The <02:05.85>devil <02:06.16>in <02:06.42>disguise
[02:07.47]<02:07.47>Oh <02:07.92>yes <02:08.29>you <02:08.66>are
[02:09.35]<02:09.35>The <02:09.60>devil <02:09.91>in <02:10.23>disguise
[02:11.47]<02:11.47>Oh <02:11.79>yes <02:12.04>you <02:12.29>are
[02:13.23]<02:13.23>The <02:13.54>devil <02:13.79>in <02:14.04>disguise
[02:14.54]<02:14.54>Oh <02:14.85>yes <02:15.10>you <02:15.54>are
[02:15.98]<02:15.98>The <02:16.29>devil <02:16.54>in <02:16.85>disguise
[02:17.41]<02:17.41>END
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (5468, 'lrc', 'line', 'local_lrc', '[00:22.45]They love me like I was a brother
[00:30.02]They protect me, listen to me
[00:38.62]They dug me my very own garden
[00:46.88]Gave me sunshine, made me happy
[00:55.19]Nice dream
[00:59.00]Nice dream
[01:03.43]Nice dream
[01:08.06]
[01:12.26]I call up my friend, the good angel
[01:20.46]But she''s out with her answerphone
[01:28.99]She says that she''d love to come help but
[01:37.32]The sea would electrocute us all
[01:45.68]Nice dream
[01:49.78]Nice dream
[01:53.66]Nice dream
[01:57.73]Nice dream
[02:01.83]Nice dream
[02:05.94]Nice dream
[02:10.37]Nice dream
[02:11.76](If you think that you''re strong enough) nice dream
[02:15.50](If you think you belong enough) nice dream
[02:19.88](If you think that you''re strong enough) nice dream
[02:24.63]If you think you belong enough
[02:45.17]I''m coming home
[02:48.50]I''m coming home
[02:52.67]I''m coming home
[02:56.98]I''m coming home
[03:03.46]
[03:24.91]Nice dream
[03:29.27]Nice dream
[03:33.60]Nice dream
[03:37.75]Nice dream
[03:39.03]
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (6297, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.69>Nothing''s<00:01.37> <00:02.06>Too<00:02.74> <00:03.43>Good<00:04.12>)<00:04.80> <00:05.49>for<00:06.17> <00:06.86>My<00:07.55> <00:08.23>Baby<00:08.92> <00:09.60>(<00:10.29>1999<00:10.98> <00:11.66>Digital<00:12.35> <00:13.03>Remaster<00:13.72>)<00:14.41> <00:15.09>-<00:15.78> <00:16.46>Louis<00:17.15> <00:17.84>Prima
[00:18.54]<00:18.54>''Cause <00:19.00>nothing <00:19.69>is <00:19.89>too <00:20.04>good <00:20.25>for <00:20.66>my <00:20.99>baby
[00:22.70]<00:22.70>For <00:23.09>my <00:23.50>baby  <00:25.16>sugar <00:25.55>baby
[00:27.98]<00:27.98>Nothing <00:28.34>is <00:28.55>too <00:28.79>good <00:29.30>for <00:29.52>my <00:29.85>baby
[00:31.71]<00:31.71>''Cause <00:32.05>baby <00:32.87>is <00:33.08>so <00:33.26>good <00:33.59>and <00:33.78>kind <00:34.22>to <00:34.41>me
[00:35.61]<00:35.61>Now <00:35.90>when <00:36.12>he <00:36.30>holds <00:36.56>me <00:36.73>in <00:36.92>his <00:37.10>arms
[00:38.09]<00:38.09>In <00:38.32>his <00:38.48>big <00:38.95>and <00:39.14>brownie <00:39.81>arms
[00:40.89]<00:40.89>My <00:41.09>happy <00:41.34>heart <00:41.77>goes <00:42.01>right <00:42.38>up <00:42.56>to <00:42.74>the <00:43.06>sky
[00:44.75]<00:44.75>Makes <00:44.96>me <00:45.13>think <00:45.42>of <00:45.59>pretty <00:45.79>things
[00:46.27]<00:46.27>So <00:46.45>I <00:46.65>even <00:47.25>buy <00:47.63>the <00:47.80>wed <00:48.11>rings
[00:49.48]<00:49.48>And <00:49.83>if <00:50.02>it <00:50.30>was <00:50.53>for <00:50.72>him <00:51.00>I <00:51.18>bake <00:51.52>a <00:51.85>pie
[00:53.73]<00:53.73>''Cause <00:54.52>nothing <00:54.97>is <00:55.16>too <00:55.38>good <00:55.79>for <00:56.14>my <00:56.36>baby
[00:57.23]<00:57.23>(<00:57.42>For <00:57.59>my <00:57.77>baby)
[00:58.37]<00:58.37>For <00:58.56>my <00:58.74>baby
[00:59.31]<00:59.31>(<00:59.50>For <00:59.67>my <00:59.86>baby)
[01:00.29]<01:00.29>Sugar <01:01.23>baby
[01:01.79]<01:01.79>(<01:02.00>Sugar <01:02.45>baby)
[01:03.38]<01:03.38>Nothing <01:03.72>is <01:03.90>too <01:04.09>good <01:04.61>for <01:04.85>my <01:05.04>baby
[01:05.80]<01:05.80>(<01:05.97>For <01:06.15>my <01:06.34>baby)
[01:07.41]<01:07.41>''Cause <01:07.62>baby <01:07.83>is <01:08.28>so <01:08.46>good <01:08.86>and <01:09.05>kind <01:09.38>to <01:09.56>me
[01:24.97]<01:24.97>And <01:25.15>just <01:25.33>for <01:25.54>you <01:25.73>I''d <01:26.01>learn <01:26.48>to <01:26.69>bake <01:27.13>a <01:27.32>pie
[01:46.82]<01:46.82>''Cause <01:47.68>nothing <01:48.05>is <01:48.23>too <01:48.43>good <01:48.74>for <01:49.15>my <01:49.43>baby
[01:50.11]<01:50.11>(<01:50.30>For <01:50.47>my <01:50.62>baby)
[01:51.43]<01:51.43>For <01:51.62>my <01:51.77>baby
[01:52.39]<01:52.39>(<01:52.57>For <01:52.76>my <01:52.90>baby)
[01:53.68]<01:53.68>Umm <01:54.32>baby
[01:54.89]<01:54.89>(<01:55.20>Umm <01:55.37>baby)
[01:56.06]<01:56.06>And <01:56.30>nothing <01:56.67>is <01:56.88>too <01:57.19>good <01:57.62>for <01:57.90>my <01:58.14>baby
[01:58.99]<01:58.99>(<01:59.17>For <01:59.36>my <01:59.52>baby)
[02:00.32]<02:00.32>''Cause <02:00.56>baby <02:01.14>is <02:01.42>so <02:01.62>good <02:02.08>and <02:02.40>kind <02:02.74>to <02:02.92>me
[02:17.47]<02:17.47>Babe  <02:18.10>and <02:18.42>just <02:18.59>for <02:18.84>you <02:19.18>I <02:19.73>itch <02:19.92>you <02:20.13>in <02:20.30>your <02:20.62>eye
[02:22.46]<02:22.46>And <02:22.69>just <02:22.91>for <02:23.29>you <02:23.69>I''d <02:23.87>like <02:24.19>to <02:24.35>see <02:24.51>you <02:24.86>try
[02:26.93]<02:26.93>And <02:27.23>just <02:27.42>for <02:27.69>you <02:27.92>I''d <02:28.37>learn <02:28.74>to <02:28.97>bake <02:29.27>a <02:29.42>pie
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (990, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:01.07>Rock<00:02.13>)<00:03.20> <00:04.27>Superstar<00:05.33> <00:06.40>(<00:07.47>Explicit<00:08.54> <00:09.60>LP<00:10.67> <00:11.74>Version<00:12.80>)<00:13.87> <00:14.94>-<00:16.00> <00:17.07>Cypress<00:18.14> <00:19.21>Hill
[00:20.28]<00:20.28>So <00:20.53>you <00:20.72>wanna <00:20.91>be <00:21.16>a <00:21.53>rock <00:21.97>superstar
[00:22.41]<00:22.41>And <00:22.59>live <00:22.84>large
[00:23.22]<00:23.22>Your <00:23.47>big <00:23.72>house  <00:24.09>five <00:24.46>cars
[00:25.03]<00:25.03>You''re <00:25.29>in <00:25.53>charge
[00:26.09]<00:26.09>Comin'' <00:26.40>up <00:26.59>in <00:26.77>the <00:26.96>world <00:27.46>don''t <00:27.71>trust <00:28.02>nobody
[00:28.71]<00:28.71>Gotta <00:28.89>look <00:29.09>over <00:29.33>your <00:29.58>shoulder <00:30.14>constantly
[00:30.89]<00:30.89>I <00:31.08>remember <00:31.20>the <00:31.39>days <00:31.77>when <00:31.95>I <00:32.08>was <00:32.26>a <00:32.51>young <00:32.70>kid  <00:33.01>growin'' <00:33.20>up
[00:33.51]<00:33.51>Lookin'' <00:33.76>in <00:34.01>the <00:34.27>mirror  <00:34.70>dreamin'' <00:34.89>about <00:35.07>blowin'' <00:35.38>up
[00:35.76]<00:35.76>The <00:35.88>rock <00:36.14>crowd  <00:36.57>make <00:36.89>money
[00:37.51]<00:37.51>True <00:37.64>with <00:37.88>the <00:38.07>honeys
[00:38.44]<00:38.44>Sign <00:38.70>autographs <00:39.19>or <00:39.45>whatever <00:39.75>the <00:39.93>people <00:40.19>want <00:40.43>from <00:40.69>me
[00:41.00]<00:41.00>Sh*t''s <00:41.38>funny
[00:41.74]<00:41.74>How <00:42.00>impossible <00:42.43>dreams <00:42.82>manifest
[00:43.44]<00:43.44>And <00:43.68>the <00:43.81>games <00:44.13>that <00:44.30>be <00:44.50>comin'' <00:44.75>with <00:44.93>it  <00:45.31>nevertheless
[00:45.94]<00:45.94>You <00:46.11>gotta <00:46.24>go <00:46.50>for <00:46.74>the <00:46.92>gusto
[00:47.37]<00:47.37>But <00:47.55>you <00:47.74>don''t <00:47.99>know
[00:48.43]<00:48.43>About <00:48.67>the <00:48.86>blood <00:49.12>sweat <00:49.36>and <00:49.62>tears
[00:50.11]<00:50.11>And <00:50.37>losin'' <00:50.49>some <00:50.67>of <00:50.86>your <00:51.11>fears
[00:51.35]<00:51.35>And <00:51.55>losin'' <00:51.74>some <00:51.85>of <00:52.04>yourself <00:52.42>to <00:52.66>the <00:52.80>years <00:53.05>past  <00:53.29>gone <00:53.55>by
[00:54.05]<00:54.05>Hopefully <00:54.23>it <00:54.47>don''t <00:54.73>manifest <00:55.22>for <00:55.48>the <00:55.78>wrong <00:56.04>guy
[00:56.53]<00:56.53>Egomaniac <00:56.79>in <00:57.16>the <00:57.42>brainiac
[00:57.98]<00:57.98>Don''t <00:58.17>know <00:58.41>how <00:58.85>to <00:59.16>act
[00:59.54]<00:59.54>Sh*t''s <00:59.78>deep  <01:00.21>48 <01:00.54>tracks
[01:01.03]<01:01.03>Studio <01:01.36>gangsta <01:01.80>max
[01:02.16]<01:02.16>Signed <01:02.49>the <01:02.79>deal <01:02.98>thinks <01:03.41>he''s <01:03.67>gonna <01:03.86>make <01:04.10>a <01:04.37>mil
[01:04.68]<01:04.68>But <01:04.86>never <01:05.12>will <01:05.36>''til <01:05.62>he <01:05.87>crosses <01:06.05>over  <01:06.56>still
[01:06.80]<01:06.80>Fillin'' <01:07.06>your <01:07.23>head <01:07.42>with <01:07.68>fantasies
[01:07.98]<01:07.98>Come <01:08.24>with <01:08.55>me
[01:08.87]<01:08.87>Show <01:09.12>the <01:09.29>sacrifice <01:09.55>it <01:09.74>takes <01:09.98>to <01:10.18>make <01:10.42>the <01:10.68>Gs
[01:11.23]<01:11.23>So <01:11.43>you <01:11.61>wanna <01:11.79>be <01:11.99>a <01:12.24>rock <01:12.48>superstar  <01:12.86>in <01:13.10>the <01:13.48>biz
[01:13.98]<01:13.98>And <01:14.23>take <01:14.41>sh*t <01:14.61>from <01:14.86>people <01:15.17>who <01:15.54>don''t <01:15.78>know <01:16.04>what <01:16.28>it <01:16.41>is
[01:16.73]<01:16.73>I <01:16.97>wish <01:17.17>it <01:17.35>was <01:17.53>all <01:17.79>fun <01:18.04>and <01:18.22>games
[01:18.48]<01:18.48>But <01:18.66>the <01:18.78>price <01:18.97>of <01:19.16>fame <01:19.35>is <01:19.53>high <01:19.72>and <01:19.91>some <01:20.15>can''t <01:20.35>pay <01:20.60>the <01:20.78>way
[01:21.28]<01:21.28>Feel <01:21.46>trapped <01:21.72>in <01:21.96>what <01:22.15>you <01:22.41>rappin'' <01:22.71>about
[01:23.27]<01:23.27>Tell <01:23.59>me <01:23.72>what <01:23.96>happened <01:24.28>when <01:24.47>you <01:24.64>lost <01:24.97>cloud
[01:25.40]<01:25.40>What <01:25.65>route <01:25.89>you <01:26.09>took <01:26.34>started <01:26.58>collapsin''
[01:27.20]<01:27.20>No <01:27.46>fans  <01:27.76>no <01:28.02>fame
[01:28.45]<01:28.45>No <01:28.65>respect  <01:29.07>no <01:29.27>change
[01:29.71]<01:29.71>No <01:29.96>women <01:30.21>and <01:30.39>everybody <01:30.58>shits <01:30.83>on <01:31.01>your <01:31.27>name
[01:31.77]<01:31.77>So <01:31.97>you <01:32.16>wanna <01:32.34>be <01:32.53>a <01:32.72>rock <01:32.96>superstar
[01:33.58]<01:33.58>And <01:33.84>live <01:34.14>large
[01:34.54]<01:34.54>Your <01:34.78>big <01:35.04>house  <01:35.47>five <01:35.79>cars
[01:36.35]<01:36.35>You''re <01:36.60>in <01:36.84>charge
[01:37.18]<01:37.18>Comin'' <01:37.44>up <01:37.62>in <01:37.88>the <01:38.12>world <01:38.57>don''t <01:38.87>trust <01:39.25>nobody
[01:39.81]<01:39.81>Gotta <01:40.05>look <01:40.31>over <01:40.56>your <01:40.80>shoulder <01:41.36>constantly
[01:41.93]<01:41.93>To <01:42.18>be <01:42.43>a <01:42.56>rock <01:42.87>superstar
[01:43.56]<01:43.56>And <01:43.81>live <01:44.05>large
[01:44.61]<01:44.61>Your <01:44.87>big <01:45.06>house  <01:45.43>five <01:45.79>cars
[01:46.42]<01:46.42>You''re <01:46.61>in <01:46.86>charge
[01:47.61]<01:47.61>Comin'' <01:47.79>up <01:47.92>in <01:48.18>the <01:48.36>world <01:48.67>don''t <01:49.10>trust <01:49.49>nobody
[01:50.05]<01:50.05>Gotta <01:50.23>look <01:50.41>over <01:50.67>your <01:50.91>shoulder <01:51.54>constantly
[02:23.16]<02:23.16>You <02:23.29>ever <02:23.41>have <02:23.60>big <02:23.79>dreams
[02:24.23]<02:24.23>Of <02:24.48>makin'' <02:24.72>real <02:25.10>cream
[02:25.72]<02:25.72>Big <02:25.91>shot <02:26.41>heavy <02:26.66>hitter <02:26.91>on <02:27.16>the <02:27.47>mainstream
[02:28.03]<02:28.03>You <02:28.28>wanna <02:28.47>look <02:28.72>shanty
[02:29.40]<02:29.40>In <02:29.65>the <02:29.90>Bentley
[02:30.59]<02:30.59>Be <02:30.84>a <02:31.09>star <02:31.46>bam <02:31.70>never <02:32.09>act <02:32.40>friendly
[02:32.84]<02:32.84>You <02:33.02>wanna <02:33.34>have <02:33.65>big <02:33.90>fame
[02:34.52]<02:34.52>Let <02:34.77>me <02:34.96>explain
[02:35.52]<02:35.52>What <02:35.71>happens <02:35.83>to <02:36.08>these <02:36.27>stars <02:36.64>and <02:36.89>their <02:37.08>big <02:37.33>brains
[02:37.83]<02:37.83>First <02:38.14>they <02:38.33>get <02:38.58>played <02:38.89>like <02:39.26>all <02:39.58>damn <02:39.95>day
[02:40.39]<02:40.39>Long <02:40.64>as <02:40.89>you <02:41.14>sell <02:41.45>everything <02:42.07>will <02:42.32>be <02:42.70>ok
[02:43.20]<02:43.20>Then <02:43.44>you <02:43.63>get <02:43.82>dissed <02:44.21>by <02:44.40>the <02:44.58>media <02:44.96>and <02:45.21>fans
[02:45.76]<02:45.76>Things <02:45.96>never <02:46.14>stay <02:46.39>the <02:46.64>same <02:46.89>way <02:47.16>they <02:47.53>began
[02:48.22]<02:48.22>I <02:48.41>heard <02:48.59>that <02:48.78>some <02:48.96>ni**a <02:49.34>gave <02:49.59>full <02:49.84>to <02:50.11>the <02:50.36>fullest
[02:50.92]<02:50.92>That''s <02:51.10>why <02:51.29>foos <02:51.60>end <02:51.85>up <02:52.10>dining <02:52.29>on <02:52.60>the <02:52.85>bullet
[02:53.37]<02:53.37>Think <02:53.55>everything''s <02:54.05>fine <02:54.36>in <02:54.61>the <02:54.80>big <02:55.11>time
[02:55.80]<02:55.80>See <02:55.99>me <02:56.30>in <02:56.49>my <02:56.74>lace <02:57.05>with <02:57.24>the <02:57.48>chrome <02:57.86>ring <02:58.11>shine
[02:58.48]<02:58.48>So <02:58.73>you <02:58.98>wanna <02:59.17>go <02:59.42>far <02:59.79>and <03:00.04>live <03:00.42>large
[03:00.92]<03:00.92>It <03:01.10>ain''t <03:01.35>all <03:01.54>that <03:01.78>goes <03:02.10>with <03:02.35>bein'' <03:02.73>a <03:02.98>rock <03:03.29>star
[03:03.54]<03:03.54>So <03:03.79>you <03:03.97>wanna <03:04.16>be <03:04.29>a <03:04.54>rock <03:04.85>superstar
[03:05.28]<03:05.28>And <03:05.53>live <03:05.78>large
[03:06.16]<03:06.16>Your <03:06.41>big <03:06.66>house <03:06.90>and <03:07.16>five <03:07.41>cars
[03:07.97]<03:07.97>You''re <03:08.16>in <03:08.40>charge
[03:08.97]<03:08.97>Comin'' <03:09.22>up <03:09.40>in <03:09.65>the <03:09.90>world <03:10.15>don''t <03:10.53>trust <03:10.96>nobody
[03:11.65]<03:11.65>Gotta <03:11.84>look <03:12.02>over <03:12.27>your <03:12.52>shoulder <03:13.08>constantly
[03:13.90]<03:13.90>To <03:14.08>be <03:14.21>a <03:14.40>rock <03:14.64>superstar
[03:15.33]<03:15.33>And <03:15.52>live <03:15.77>large
[03:16.33]<03:16.33>Your <03:16.52>big <03:16.70>house <03:17.02>and <03:17.33>five <03:17.70>cars
[03:18.20]<03:18.20>You''re <03:18.45>in <03:18.70>charge
[03:19.26]<03:19.26>Comin'' <03:19.51>up <03:19.70>in <03:19.89>the <03:20.15>world <03:20.53>don''t <03:20.78>trust <03:21.21>nobody
[03:21.77]<03:21.77>Gotta <03:21.96>look <03:22.15>over <03:22.34>your <03:22.59>shoulder <03:23.16>constantly
[03:23.85]<03:23.85>My <03:24.04>own <03:24.22>son <03:24.47>don''t <03:24.85>know <03:25.10>me
[03:25.41]<03:25.41>I''m <03:25.66>chillin'' <03:25.85>in <03:26.03>the <03:26.30>hotel <03:26.49>room <03:26.74>lonely
[03:27.42]<03:27.42>But <03:27.61>I <03:27.80>thank <03:27.98>God <03:28.17>I''m <03:28.36>with <03:28.61>my <03:28.86>holmies
[03:29.15]<03:29.15>But <03:29.34>sometimes <03:29.77>I <03:29.96>wish <03:30.15>I <03:30.34>was <03:30.59>back <03:30.77>home
[03:31.08]<03:31.08>But <03:31.27>only <03:31.52>no <03:31.77>radio <03:32.21>or <03:32.46>video''s <03:33.08>gonna <03:33.46>show <03:33.71>me
[03:34.20]<03:34.20>No <03:34.39>love  <03:34.83>they''re <03:35.08>phony
[03:35.64]<03:35.64>Gotta <03:35.83>hit <03:36.08>the <03:36.26>road <03:36.51>solely
[03:37.08]<03:37.08>So <03:37.32>the <03:37.51>record <03:37.76>gets <03:38.09>pushed <03:38.32>by <03:38.64>Sony
[03:39.07]<03:39.07>I''m <03:39.26>in <03:39.45>the <03:39.70>middle <03:40.01>like <03:40.20>mony
[03:40.57]<03:40.57>And <03:40.76>the <03:40.94>press <03:41.13>say <03:41.38>that
[03:41.76]<03:41.76>My <03:41.94>own <03:42.13>people <03:42.44>disown <03:42.88>me
[03:43.13]<03:43.13>And <03:43.32>the <03:43.50>best <03:43.75>way <03:44.00>back
[03:44.25]<03:44.25>Is <03:44.44>to <03:44.56>keep <03:44.81>your <03:45.00>head <03:45.25>straight  <03:45.76>never <03:45.94>inflate <03:46.12>the <03:46.37>cranium
[03:46.81]<03:46.81>You''re <03:47.00>too <03:47.18>worried <03:47.62>about <03:47.81>them <03:48.00>honeys <03:48.24>at <03:48.49>the <03:48.74>Peladium
[03:49.31]<03:49.31>Who <03:49.56>just <03:49.74>wanna <03:49.99>cling <03:50.26>on  <03:50.69>swing <03:51.01>on
[03:51.44]<03:51.44>And <03:51.57>so <03:51.75>forth  <03:52.19>go <03:52.38>on  <03:52.75>fall <03:53.00>off  <03:53.33>the <03:53.58>whole <03:53.89>foot <03:54.14>long
[03:54.33]<03:54.33>To <03:54.52>the <03:54.77>next <03:54.95>rock <03:55.20>superstar  <03:55.58>with <03:56.01>no <03:56.34>shame
[03:57.03]<03:57.03>Give <03:57.21>him <03:57.40>a <03:57.59>year <03:57.96>and <03:58.15>they''ll <03:58.40>be <03:58.53>right <03:58.77>out <03:58.96>the <03:59.23>game
[03:59.54]<03:59.54>The <03:59.73>same <03:59.98>as <04:00.23>the <04:00.35>last <04:00.54>one <04:00.72>who <04:00.97>came <04:01.29>before <04:01.66>him
[04:02.42]<04:02.42>Gained <04:02.60>fame <04:02.78>started <04:03.10>gettin'' <04:03.28>ignored  <04:03.84>I <04:04.03>warned <04:04.28>him
[04:04.78]<04:04.78>Assured <04:05.03>him
[04:05.47]<04:05.47>This <04:05.72>ain''t <04:06.03>easy
[04:06.40]<04:06.40>Take <04:06.59>it <04:06.78>from <04:07.03>Wheezy
[04:07.46]<04:07.46>Sleazy <04:07.65>people <04:07.84>wanna <04:08.03>be <04:08.28>so <04:08.52>cheesy
[04:08.84]<04:08.84>They''re <04:09.02>f**kin <04:09.27>evil
[04:14.42]<04:14.42>So <04:14.61>you <04:14.86>wanna <04:15.05>be <04:15.23>a <04:15.48>rock <04:15.73>superstar
[04:16.48]<04:16.48>And <04:16.73>live <04:16.98>large
[04:17.48]<04:17.48>Your <04:17.67>big <04:17.92>house  <04:18.23>five <04:18.66>cars
[04:19.29]<04:19.29>You''re <04:19.48>in <04:19.73>charge
[04:20.29]<04:20.29>Comin'' <04:20.47>up <04:20.72>in <04:20.91>the <04:21.16>world <04:21.60>don''t <04:21.85>trust <04:22.24>nobody
[04:22.86]<04:22.86>Gotta <04:23.05>look <04:23.30>over <04:23.55>your <04:23.80>shoulder <04:24.36>constantly
[04:25.13]<04:25.13>To <04:25.31>be <04:25.50>a <04:25.75>rock <04:26.00>superstar
[04:26.62]<04:26.62>And <04:26.87>live <04:27.12>large
[04:27.62]<04:27.62>Your <04:27.87>big <04:28.13>house  <04:28.51>five <04:28.88>cars
[04:29.44]<04:29.44>You''re <04:29.69>in <04:29.94>charge
[04:30.44]<04:30.44>Comin'' <04:30.69>up <04:30.89>in <04:31.07>the <04:31.27>world <04:31.77>don''t <04:32.08>trust <04:32.46>nobody
[04:33.02]<04:33.02>Gotta <04:33.20>look <04:33.46>over <04:33.70>your <04:33.95>shoulder <04:34.45>constantly
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (1074, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.36>Sittin<00:00.72>''<00:01.08> <00:01.44>On<00:01.79>)<00:02.15> <00:02.51>the<00:02.87> <00:03.23>Dock<00:03.59> <00:03.95>of<00:04.31> <00:04.67>the<00:05.03> <00:05.38>Bay<00:05.74> <00:06.10>(<00:06.46>Mono<00:06.82>)<00:07.18> <00:07.54>-<00:07.90> <00:08.26>Otis<00:08.62> <00:08.97>Redding
[00:09.35]<00:09.35>Sitting <00:09.81>in <00:10.03>the <00:10.41>morning <00:11.24>sun
[00:13.39]<00:13.39>I''ll <00:13.63>be <00:13.89>sitting <00:14.38>when <00:14.62>the <00:14.94>evening <00:15.92>come
[00:18.65]<00:18.65>Watching <00:19.43>the <00:19.67>ships <00:20.07>roll <00:20.59>in
[00:22.59]<00:22.59>And <00:22.82>then <00:23.08>I <00:23.37>watch <00:23.69>em <00:23.98>roll <00:24.29>away <00:24.96>again  <00:26.29>yeah
[00:27.60]<00:27.60>I''m <00:27.82>sitting <00:28.18>on <00:28.39>the <00:28.70>dock <00:29.23>of <00:29.50>the <00:29.77>bay
[00:31.32]<00:31.32>Watching <00:31.62>the <00:31.99>tide <00:33.51>roll <00:34.29>away
[00:35.66]<00:35.66>Ooo  <00:36.39>I''m <00:36.61>just <00:37.10>sitting <00:37.49>on <00:37.80>the <00:38.08>dock <00:38.47>of <00:38.83>the <00:39.24>bay
[00:40.90]<00:40.90>Wastin'' <00:41.23>time
[00:46.11]<00:46.11>I <00:46.36>left <00:46.69>my <00:47.05>home <00:47.97>in <00:48.51>Georgia
[00:51.01]<00:51.01>Headed <00:51.56>for <00:51.84>the <00:52.15>Frisco <00:52.74>bay
[00:55.53]<00:55.53>Cause <00:55.76>I''ve <00:56.02>had <00:56.53>nothing <00:57.23>to <00:57.44>live <00:58.00>for
[00:59.44]<00:59.44>And <00:59.79>look <01:00.09>like <01:00.46>nothing''s <01:00.83>gonna <01:01.28>come <01:01.75>my <01:02.22>way
[01:03.39]<01:03.39>So <01:04.12>I''m <01:04.34>just <01:04.49>gonna <01:04.81>sit <01:05.47>on <01:05.71>the <01:05.98>dock <01:06.42>of <01:06.66>the <01:06.94>bay
[01:08.55]<01:08.55>Watching <01:08.84>the <01:09.18>tide <01:10.82>roll <01:11.47>away
[01:12.94]<01:12.94>Ooo  <01:14.07>I''m <01:14.29>sittin'' <01:14.66>on <01:14.95>the <01:15.25>dock <01:15.73>of <01:16.05>the <01:16.36>bay
[01:17.82]<01:17.82>Wastin'' <01:18.39>time
[01:24.61]<01:24.61>Look <01:24.97>like <01:25.70>nothing''s <01:26.00>gonna <01:26.52>change
[01:29.43]<01:29.43>Everything <01:30.57>still <01:31.16>remains <01:31.68>the <01:31.89>same
[01:33.80]<01:33.80>I <01:34.02>can''t <01:34.27>do <01:34.69>what <01:34.98>ten <01:35.45>people <01:35.88>tell <01:36.24>me <01:36.55>to <01:36.77>do
[01:38.06]<01:38.06>So <01:38.41>I <01:38.70>guess <01:39.01>I''ll <01:39.31>remain <01:40.23>the <01:40.45>same  <01:41.41>yes
[01:42.23]<01:42.23>Sittin'' <01:42.66>here <01:43.28>resting <01:43.80>my <01:44.16>bones
[01:46.06]<01:46.06>And <01:46.28>this <01:46.59>loneliness <01:47.67>won''t <01:48.22>leave <01:48.58>me <01:48.88>alone
[01:50.66]<01:50.66>It''s <01:51.46>two <01:51.82>thousand <01:52.61>miles <01:53.10>I <01:53.32>roamed
[01:55.53]<01:55.53>Just <01:55.74>to <01:55.95>make <01:56.63>this <01:57.09>dock <01:57.57>my <01:58.02>home
[01:59.29]<01:59.29>Now  <01:59.87>I''m <02:00.09>just <02:00.52>gonna <02:00.88>sit <02:01.27>at <02:01.48>the <02:01.73>dock <02:02.19>of <02:02.41>the <02:02.70>bay
[02:04.27]<02:04.27>Watching <02:04.56>the <02:04.86>tide <02:06.41>roll <02:06.93>away
[02:08.60]<02:08.60>Ooh <02:09.18>wee  <02:09.82>sitting <02:10.39>on <02:10.62>the <02:10.90>dock <02:11.44>of <02:11.69>the <02:12.03>bay
[02:13.84]<02:13.84>Wastin'' <02:14.22>time
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (15536, 'lrc', 'line', 'local_lrc', '[00:06.27]<00:06.27>Sittin''<00:06.75> <00:06.84>in<00:06.89> <00:07.01>the<00:07.18> <00:07.22>mornin''<00:07.92> <00:08.16>sun
[00:14.96]<00:14.96>I''ll<00:15.09> <00:15.12>be<00:15.22> <00:15.39>sittin''<00:15.68> <00:15.81>when<00:16.20> <00:16.25>the<00:16.39> <00:16.43>evenin''<00:17.05> <00:17.26>come
[00:19.65]<00:19.65>Watching<00:20.32> <00:20.69>the<00:20.75> <00:20.90>ships<00:21.01> <00:21.61>roll<00:21.78> <00:21.81>in
[00:24.33]<00:24.33>And<00:24.44> <00:24.49>I<00:24.50> <00:24.64>watch<00:25.12> <00:25.43>''em<00:25.51> <00:25.61>roll<00:25.87> <00:25.92>away<00:26.60> <00:26.62>again,<00:27.43> <00:28.23>yeah
[00:28.95]<00:28.95>I''m<00:29.07> <00:29.11>sittin''<00:29.60> <00:29.64>on<00:29.77> <00:29.89>the<00:29.98> <00:30.09>dock<00:30.34> <00:30.50>of<00:30.66> <00:30.82>the<00:31.05> <00:31.15>bay
[00:32.64]<00:32.64>Watching<00:33.24> <00:33.28>the<00:33.48> <00:33.53>tide<00:34.34> <00:34.44>roll<00:34.77> <00:35.13>away
[00:38.20]<00:38.20>Ooh,<00:38.57> <00:38.84>I''m<00:39.30> <00:39.35>just<00:39.41> <00:39.69>sittin''<00:40.11> <00:40.25>on<00:40.38> <00:40.42>the<00:40.58> <00:40.62>dock<00:40.87> <00:41.01>of<00:41.16> <00:41.20>the<00:41.41> <00:41.45>bay
[00:42.96]<00:42.96>Wastin''<00:43.39> <00:43.85>time
[00:48.57]<00:48.57>I<00:48.59> <00:48.77>left<00:48.95> <00:49.10>my<00:49.45> <00:49.49>home<00:50.05> <00:50.11>in<00:50.43> <00:50.47>Georgia
[00:52.31]<00:52.31>Headed<00:53.20> <00:53.26>for<00:53.42> <00:53.47>the<00:53.69> <00:53.74>''Frisco<00:54.48> <00:54.51>bay
[00:56.68]<00:56.68>''Cause<00:56.94> <00:57.36>I''ve<00:57.52> <00:57.56>had<00:58.16> <00:58.19>nothing<00:58.69> <00:58.94>to<00:59.10> <00:59.13>live<00:59.48> <00:59.69>for
[01:01.35]<01:01.35>And<01:01.49> <01:01.53>look<01:01.75> <01:01.80>like<01:01.99> <01:02.04>nothing''s<01:02.58> <01:02.65>gonna<01:03.13> <01:03.17>come<01:03.42> <01:03.48>my<01:03.94> <01:03.98>way
[01:06.11]<01:06.11>So<01:06.35> <01:06.38>I''m<01:06.45> <01:06.54>just<01:06.70> <01:06.96>gonna<01:07.18> <01:07.22>sit<01:07.74> <01:07.78>on<01:08.00> <01:08.05>the<01:08.26> <01:08.32>dock<01:08.65> <01:08.90>of<01:09.05> <01:09.10>the<01:09.37> <01:09.40>bay
[01:10.13]<01:10.13>Watching<01:10.73> <01:10.80>the<01:11.14> <01:11.19>tide<01:12.38> <01:12.41>roll<01:12.54> <01:13.01>away
[01:15.41]<01:15.41>Ooh,<01:15.70> <01:16.24>I''m<01:16.39> <01:16.42>sittin''<01:16.88> <01:16.96>on<01:17.11> <01:17.17>the<01:17.26> <01:17.41>dock<01:17.66> <01:17.70>of<01:17.90> <01:17.95>the<01:18.14> <01:18.19>bay
[01:19.57]<01:19.57>Wastin''<01:19.88> <01:20.27>time
[01:25.49]<01:25.49>Look<01:26.07> <01:26.29>like<01:26.73> <01:26.76>nothing''s<01:27.79> <01:27.91>gonna<01:28.14> <01:28.43>change
[01:29.99]<01:29.99>Everything<01:31.16> <01:31.64>still<01:31.95> <01:32.32>remains<01:33.14> <01:33.17>the<01:33.40> <01:33.48>same
[01:35.29]<01:35.29>I<01:35.30> <01:35.45>can''t<01:35.91> <01:35.94>do<01:36.15> <01:36.26>what<01:36.62> <01:36.70>ten<01:36.94> <01:37.04>people<01:37.54> <01:37.57>tell<01:37.76> <01:37.94>me<01:38.15> <01:38.19>to<01:38.38> <01:38.42>do
[01:39.92]<01:39.92>So<01:39.98> <01:40.26>I<01:40.37> <01:40.52>guess<01:40.72> <01:40.79>I''ll<01:40.94> <01:41.09>remain<01:41.60> <01:41.98>the<01:42.17> <01:42.20>same,<01:42.77> <01:43.10>yes
[01:43.93]<01:43.93>Sittin''<01:44.12> <01:44.49>here<01:44.84> <01:44.98>resting<01:45.40> <01:45.61>my<01:45.85> <01:45.92>bones
[01:47.68]<01:47.68>And<01:47.84> <01:47.89>this<01:48.17> <01:48.32>loneliness<01:48.66> <01:49.04>won''t<01:49.38> <01:49.48>leave<01:49.81> <01:49.87>me<01:50.12> <01:50.13>alone
[01:52.25]<01:52.25>It''s<01:52.30> <01:52.72>two<01:52.98> <01:53.02>thousand<01:53.82> <01:54.01>miles<01:54.37> <01:54.42>I<01:54.45> <01:54.76>roamed
[01:56.94]<01:56.94>Just<01:57.15> <01:57.33>to<01:57.50> <01:57.53>make<01:57.85> <01:57.88>this<01:58.26> <01:58.32>dock<01:58.59> <01:58.70>my<01:59.02> <01:59.05>home
[02:01.50]<02:01.50>Now,<02:01.78> <02:01.81>I''m<02:01.86> <02:01.96>just<02:02.25> <02:02.36>gonna<02:02.55> <02:02.60>sit<02:02.94> <02:03.00>at<02:03.04> <02:03.17>the<02:03.29> <02:03.62>dock<02:03.85> <02:03.92>of<02:04.13> <02:04.29>the<02:04.51> <02:04.56>bay
[02:05.99]<02:05.99>Watching<02:06.55> <02:06.71>the<02:06.90> <02:06.95>tide<02:07.79> <02:08.03>roll<02:08.12> <02:08.44>away
[02:10.92]<02:10.92>Ooh-<02:11.30>wee,<02:11.61> <02:12.11>sittin''<02:12.25> <02:12.54>on<02:12.86> <02:12.97>the<02:13.15> <02:13.37>dock<02:13.60> <02:13.63>of<02:13.85> <02:14.01>the<02:14.31> <02:14.37>bay
[02:15.19]<02:15.19>Wastin''<02:15.70> <02:16.16>time
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (7169, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.11>S<00:00.23>i<00:00.34>t<00:00.45>t<00:00.57>i<00:00.68>n<00:00.80>''<00:00.91> <00:01.02>O<00:01.14>n<00:01.25>)<00:01.36> <00:01.48>t<00:01.59>h<00:01.71>e<00:01.82> <00:01.93>D<00:02.05>o<00:02.16>c<00:02.27>k<00:02.39> <00:02.50>o<00:02.61>f<00:02.73> <00:02.84>t<00:02.96>h<00:03.07>e<00:03.18> <00:03.30>B<00:03.41>a<00:03.52>y<00:03.64> <00:03.75>-<00:03.87> <00:03.98>O<00:04.09>t<00:04.21>i<00:04.32>s<00:04.43> <00:04.55>R<00:04.66>e<00:04.77>d<00:04.89>d<00:05.00>i<00:05.12>n<00:05.23>g
[00:05.34]<00:05.34>W<00:05.46>r<00:05.57>i<00:05.68>t<00:05.80>t<00:05.91>e<00:06.03>n<00:06.14> <00:06.25>b<00:06.37>y<00:06.48>：<00:06.59>S<00:06.71>t<00:06.82>e<00:06.93>v<00:07.05>e<00:07.16> <00:07.28>C<00:07.39>r<00:07.50>o<00:07.62>p<00:07.73>p<00:07.84>e<00:07.96>r<00:08.07>/<00:08.19>O<00:08.30>t<00:08.41>i<00:08.53>s<00:08.64> <00:08.75>R<00:08.87>e<00:08.98>d<00:09.10>d<00:09.21>i<00:09.32>n<00:09.44>g
[00:09.55]<00:09.55>Sittin''<00:10.46> <00:10.46>in<00:10.77> <00:10.77>the<00:10.88> <00:10.88>mornin''<00:11.78> <00:11.78>sun
[00:13.91]<00:13.91>I''ll<00:14.23> <00:14.23>be<00:14.40> <00:14.40>sittin''<00:15.10> <00:15.10>when<00:15.39> <00:15.39>the<00:15.64> <00:15.64>evenin''<00:16.50> <00:16.50>come
[00:19.14]<00:19.14>Watching<00:20.05> <00:20.06>the<00:20.20> <00:20.20>ships<00:20.74> <00:20.74>roll<00:21.30> <00:21.30>in
[00:22.99]<00:22.99>And<00:23.21> <00:23.21>then<00:23.59> <00:23.59>I<00:23.74> <00:23.74>watch<00:24.18> <00:24.18>em<00:24.36> <00:24.36>roll<00:24.75> <00:24.75>away<00:25.67> <00:25.67>again<00:26.32> <00:26.91>yeah
[00:28.17]<00:28.17>I''m<00:28.29> <00:28.29>sittin''<00:29.04> <00:29.05>on<00:29.36> <00:29.36>the<00:29.47> <00:29.47>dock<00:29.95> <00:29.95>of<00:30.19> <00:30.19>the<00:30.37> <00:30.37>bay
[00:31.92]<00:31.92>Watching<00:32.49> <00:32.49>the<00:32.63> <00:32.63>tide<00:34.23> <00:34.24>roll<00:34.79> <00:34.79>away
[00:36.31]<00:36.31>Oh<00:36.92> <00:36.92>I''m<00:37.12> <00:37.12>just<00:37.71> <00:37.71>sittin''<00:38.34> <00:38.35>on<00:38.66> <00:38.66>the<00:38.77> <00:38.77>dock<00:39.26> <00:39.26>of<00:39.46> <00:39.46>the<00:39.65> <00:39.65>bay
[00:41.19]<00:41.19>Wastin''<00:42.01> <00:42.01>time
[00:46.73]<00:46.73>I<00:46.89> <00:46.89>left<00:47.28> <00:47.28>my<00:47.82> <00:47.82>home<00:48.80> <00:48.80>in<00:49.02> <00:49.02>Georgia
[00:51.38]<00:51.38>Headed<00:52.23> <00:52.23>for<00:52.55> <00:52.55>the<00:52.72> <00:52.72>Frisco<00:53.64> <00:53.64>bay
[00:56.14]<00:56.14>Cause<00:56.45> <00:56.45>I''ve<00:56.70> <00:56.70>had<00:57.23> <00:57.58>nothing<00:58.16> <00:58.16>to<00:58.34> <00:58.45>live<00:58.94> <00:58.95>for
[01:00.24]<01:00.24>And<01:00.50> <01:00.50>look<01:00.78> <01:00.78>like<01:01.07> <01:01.07>nothin''s<01:01.69> <01:01.69>gonna<01:02.18> <01:02.18>come<01:02.59> <01:02.59>my<01:03.12> <01:03.12>way
[01:04.18]<01:04.18>So<01:04.99> <01:04.99>I''m<01:05.18> <01:05.18>just<01:05.52> <01:05.52>gonna<01:05.72> <01:05.73>sit<01:06.13> <01:06.50>on<01:06.75> <01:06.75>the<01:06.87> <01:06.87>dock<01:07.37> <01:07.37>of<01:07.61> <01:07.61>the<01:07.81> <01:07.81>bay
[01:09.35]<01:09.35>Watching<01:09.96> <01:09.96>the<01:10.11> <01:10.11>tide<01:11.70> <01:11.71>roll<01:12.29> <01:12.29>away
[01:13.74]<01:13.74>Oh<01:14.53> <01:14.90>I''m<01:15.06> <01:15.06>sittin''<01:15.83> <01:15.83>on<01:16.13> <01:16.13>the<01:16.23> <01:16.31>dock<01:16.72> <01:16.72>of<01:16.96> <01:16.96>the<01:17.14> <01:17.14>bay
[01:18.64]<01:18.64>Wastin''<01:19.51> <01:19.51>time
[01:25.37]<01:25.37>Look<01:25.84> <01:25.84>like<01:26.21> <01:26.44>nothing''s<01:27.15> <01:27.15>gonna<01:27.64> <01:27.64>change
[01:30.00]<01:30.00>Everything<01:31.50> <01:31.50>still<01:32.08> <01:32.08>remains<01:32.80> <01:32.80>the<01:32.87> <01:32.87>same
[01:34.71]<01:34.71>I<01:34.94> <01:34.94>can''t<01:35.29> <01:35.29>do<01:35.65> <01:35.65>what<01:36.13> <01:36.13>ten<01:36.48> <01:36.48>people<01:36.93> <01:36.93>tell<01:37.38> <01:37.41>me<01:37.62> <01:37.62>to<01:37.90> <01:37.90>do
[01:38.95]<01:38.95>So<01:39.45> <01:39.45>I<01:39.66> <01:39.66>guess<01:40.08> <01:40.08>I''ll<01:40.30> <01:40.30>remain<01:41.14> <01:41.21>the<01:41.36> <01:41.37>same<01:42.34> <01:42.34>yes
[01:43.06]<01:43.06>Sittin''<01:43.78> <01:43.78>here<01:44.38> <01:44.39>resting<01:44.97> <01:44.97>my<01:45.17> <01:45.17>bones
[01:47.08]<01:47.08>And<01:47.34> <01:47.34>this<01:47.55> <01:47.55>loneliness<01:48.77> <01:48.77>won''t<01:49.06> <01:49.06>leave<01:49.41> <01:49.41>me<01:49.66> <01:49.66>alone
[01:51.68]<01:51.68>It''s<01:52.10> <01:52.55>two<01:52.79> <01:52.79>thousand<01:53.70> <01:53.70>miles<01:54.36> <01:54.36>I<01:54.49> <01:54.49>roamed
[01:56.60]<01:56.60>Just<01:56.91> <01:56.91>to<01:57.07> <01:57.07>make<01:57.76> <01:57.76>this<01:58.15> <01:58.25>dock<01:58.66> <01:58.66>my<01:59.20> <01:59.20>home
[02:00.32]<02:00.32>Now<02:01.00> <02:01.01>I''m<02:01.26> <02:01.26>just<02:01.59> <02:01.59>gonna<02:01.79> <02:01.80>sit<02:02.37> <02:02.42>at<02:02.75> <02:02.75>the<02:02.89> <02:02.89>dock<02:03.41> <02:03.41>of<02:03.68> <02:03.68>the<02:03.81> <02:03.81>bay
[02:05.35]<02:05.35>Watching<02:05.93> <02:05.93>the<02:06.09> <02:06.09>tide<02:07.59> <02:07.59>roll<02:08.38> <02:08.38>away
[02:09.71]<02:09.71>Oh<02:10.29> <02:10.29>wee<02:10.72> <02:11.03>sittin''<02:11.81> <02:11.82>on<02:12.13> <02:12.13>the<02:12.23> <02:12.23>dock<02:12.74> <02:12.74>of<02:13.04> <02:13.04>the<02:13.16> <02:13.16>bay
[02:14.61]<02:14.61>Wastin''<02:15.59> <02:15.59>time
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (6717, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.44>There''s<00:00.87>)<00:01.31> <00:01.75>Always<00:02.19> <00:02.62>Something<00:03.06> <00:03.50>There<00:03.93> <00:04.37>To<00:04.81> <00:05.24>Remind<00:05.68> <00:06.12>Me<00:06.55> <00:06.99>(<00:07.43>2004<00:07.87> <00:08.30>Digital<00:08.74> <00:09.18>Remaster<00:09.61>)<00:10.05> <00:10.49>-<00:10.93> <00:11.36>Sandie<00:11.80> <00:12.24>Shaw
[00:12.68]<00:12.68>I <00:12.88>walk <00:13.16>along <00:13.95>those <00:14.18>city <00:14.59>streets
[00:15.27]<00:15.27>You <00:15.47>used <00:15.67>to <00:15.88>walk <00:16.46>along <00:17.08>with <00:17.38>me
[00:19.45]<00:19.45>And <00:19.64>every <00:20.12>step <00:20.64>I <00:20.87>take <00:21.44>recalls
[00:22.14]<00:22.14>How <00:22.37>much <00:22.73>in <00:22.96>love <00:23.30>we <00:23.69>used <00:24.00>to <00:24.40>be
[00:25.05]<00:25.05>Oh <00:25.28>how <00:25.52>can <00:25.76>I <00:27.12>forget <00:27.51>you
[00:29.25]<00:29.25>When <00:29.45>there <00:29.73>is <00:29.97>always <00:30.68>something <00:31.34>there <00:31.62>to <00:31.94>remind <00:32.39>me
[00:35.65]<00:35.65>Always <00:36.10>something <00:36.72>there <00:37.03>to <00:37.31>remind <00:37.72>me
[00:40.45]<00:40.45>I <00:40.64>was <00:40.95>born <00:42.04>to <00:42.31>love <00:42.64>you
[00:44.35]<00:44.35>And <00:44.56>I <00:44.80>will <00:45.12>never <00:46.25>be <00:46.45>free
[00:47.23]<00:47.23>You''ll <00:47.47>always <00:47.74>be <00:47.94>a <00:48.12>part <00:48.76>of <00:48.98>me
[00:50.04]<00:50.04>Awho <00:50.35>ooo <00:50.76>ohhh <00:51.14>oh
[00:54.20]<00:54.20>When <00:54.38>shadows <00:55.01>fall
[00:55.73]<00:55.73>I <00:55.95>pass <00:56.22>the <00:56.41>small <00:56.94>cafe
[00:57.52]<00:57.52>Where <00:57.78>we <00:58.13>would <00:58.38>dance <00:58.75>at <00:59.13>night
[01:01.23]<01:01.23>And <01:01.40>I <01:01.67>can''t <01:01.96>help
[01:02.62]<01:02.62>Recalling <01:03.30>how <01:03.62>it <01:03.86>felt
[01:04.36]<01:04.36>To <01:04.67>kiss <01:05.02>and <01:05.32>hold <01:05.62>you <01:05.94>tight
[01:06.75]<01:06.75>Oh <01:06.98>how <01:07.24>can <01:07.48>I <01:08.76>forget <01:09.24>you
[01:10.93]<01:10.93>When <01:11.15>there <01:11.40>is <01:11.68>always <01:12.34>something <01:12.99>there <01:13.31>to <01:13.69>remind <01:14.09>me
[01:17.29]<01:17.29>Always <01:17.80>something <01:18.43>there <01:18.78>to <01:19.12>remind <01:19.53>me
[01:22.18]<01:22.18>I <01:22.39>was <01:22.69>born <01:23.78>to <01:24.08>love <01:24.39>you
[01:26.11]<01:26.11>And <01:26.31>I <01:26.55>will <01:26.85>never <01:27.98>be <01:28.19>free
[01:28.93]<01:28.93>You''ll <01:29.14>always <01:29.47>be <01:29.68>a <01:29.86>part <01:30.48>of <01:30.76>me
[01:31.71]<01:31.71>Awho <01:31.98>ooo <01:32.45>ohhh <01:33.23>oh
[01:33.67]<01:33.67>Whoa <01:34.09>oh <01:34.34>ooo <01:34.73>ohhh
[01:50.02]<01:50.02>If <01:50.20>you <01:50.43>should <01:50.65>find <01:51.30>you <01:51.53>miss
[01:52.09]<01:52.09>The <01:52.41>sweet <01:52.73>and <01:53.01>tender <01:53.44>love
[01:53.76]<01:53.76>We <01:54.11>used <01:54.44>to <01:54.73>share
[01:56.91]<01:56.91>Just <01:57.10>come <01:57.38>back <01:57.60>to <01:58.29>the <01:58.49>places
[01:58.84]<01:58.84>Where <01:59.29>we <01:59.65>used <01:59.98>to <02:00.30>go
[02:01.07]<02:01.07>And <02:01.29>I''ll <02:01.52>be <02:01.75>there
[02:02.46]<02:02.46>Oh <02:02.68>how <02:02.91>can <02:03.12>I <02:04.43>forget <02:04.88>you
[02:06.67]<02:06.67>When <02:06.87>there <02:07.34>is <02:07.51>always <02:08.06>something <02:08.73>there <02:09.03>to <02:09.43>remind <02:09.82>me
[02:13.06]<02:13.06>Always <02:13.57>something <02:14.17>there <02:14.51>to <02:14.90>remind <02:15.35>me
[02:17.95]<02:17.95>I <02:18.16>was <02:18.45>born <02:19.52>to <02:19.77>love <02:20.16>you
[02:21.90]<02:21.90>And <02:22.12>I <02:22.40>will <02:22.68>never <02:23.70>be <02:23.93>free
[02:24.69]<02:24.69>When <02:24.90>there <02:25.13>is <02:25.39>always <02:26.12>something <02:26.76>there <02:27.08>to <02:27.43>remind <02:27.79>me
[02:31.09]<02:31.09>Always <02:31.57>something <02:32.16>there <02:32.49>to <02:32.88>remind <02:33.35>me
[02:36.50]<02:36.50>Always <02:37.08>something <02:37.67>there <02:38.02>to <02:38.37>remind <02:38.77>me
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (9832, 'lrc', 'line', 'local_lrc', '[00:09.97]<00:09.97>Why <00:10.69>do <00:11.32>birds <00:12.55>suddenly <00:13.94>appear
[00:16.33]<00:16.33>Everytime <00:18.55>you <00:18.89>are <00:19.20>near
[00:22.23]<00:22.23>Just <00:22.66>like <00:23.19>me
[00:24.72]<00:24.72>They <00:25.40>long <00:26.01>to <00:26.27>be
[00:27.45]<00:27.45>Close <00:28.07>to <00:28.85>you
[00:31.57]<00:31.57>Why <00:32.98>stars <00:34.54>fall <00:35.08>down <00:35.52>from <00:35.98>the <00:36.16>sky
[00:38.20]<00:38.20>Everytime <00:40.41>you <00:40.73>walk <00:41.13>by
[00:44.07]<00:44.07>Just <00:44.48>like <00:45.10>me
[00:46.87]<00:46.87>They <00:47.34>long <00:47.93>to <00:48.24>be
[00:49.33]<00:49.33>Close <00:50.01>to <00:50.74>you
[00:54.92]<00:54.92>On <00:55.44>the <00:55.62>day <00:56.12>that <00:56.30>you <00:56.64>were <00:57.00>born
[00:57.52]<00:57.52>The <00:57.62>angels <00:58.38>got <00:58.72>together
[00:59.94]<00:59.94>They <01:00.14>decided <01:01.02>to <01:01.45>create <01:02.02>a <01:02.12>dream <01:02.79>come <01:03.37>true
[01:04.99]<01:04.99>So <01:05.57>they <01:05.76>sprinkled <01:06.49>moon <01:06.91>dust <01:07.19>in <01:07.37>your <01:07.60>hair
[01:08.50]<01:08.50>And <01:08.72>put <01:08.96>a <01:09.09>starlight <01:09.91>in <01:10.21>your <01:10.56>eyes <01:10.90>so <01:11.04>blue
[01:15.30]<01:15.30>That <01:15.97>is <01:16.61>why <01:17.99>all <01:18.52>the <01:18.76>girls <01:19.29>in <01:19.65>town
[01:21.70]<01:21.70>Follow <01:23.11>you
[01:23.92]<01:23.92>All <01:24.65>around
[01:27.45]<01:27.45>Just <01:27.84>like <01:28.42>me
[01:30.23]<01:30.23>They <01:30.75>long <01:31.37>to <01:31.61>be
[01:32.76]<01:32.76>Close <01:33.47>to <01:34.36>you
[02:00.06]<02:00.06>On <02:00.53>the <02:00.75>day <02:01.25>that <02:01.42>you <02:01.85>were <02:02.10>born
[02:02.61]<02:02.61>The <02:02.87>angels <02:03.49>got <02:03.84>together
[02:05.04]<02:05.04>They <02:05.25>decided <02:06.16>to <02:06.58>create <02:07.11>a <02:07.26>dream <02:07.84>come <02:08.49>true
[02:09.94]<02:09.94>So <02:10.59>they <02:10.73>sprinkled <02:11.52>moon <02:11.91>dust <02:12.18>in <02:12.36>your <02:12.58>hair
[02:13.48]<02:13.48>And <02:13.82>put <02:13.96>a <02:14.07>starlight <02:14.89>in <02:15.19>your <02:15.43>eyes <02:16.34>so <02:18.86>blue
[02:20.14]<02:20.14>That <02:20.83>is <02:21.46>why <02:22.82>all <02:23.51>the <02:24.08>girls <02:24.55>in <02:24.65>town
[02:26.40]<02:26.40>Follow <02:27.82>you
[02:28.69]<02:28.69>All <02:29.41>around
[02:32.23]<02:32.23>Just <02:32.67>like <02:33.32>me
[02:34.99]<02:34.99>They <02:35.45>long <02:38.02>to <02:43.46>be
[02:49.63]<02:49.63>Close <02:50.27>to <02:50.82>you
[03:06.03]<03:06.03>Just <03:11.41>like <03:16.28>me
[03:16.79]<03:16.79>They <03:22.11>long <03:27.48>to <03:31.02>be
[03:35.32]<03:35.32>Close <03:38.15>to <03:40.10>you
[03:59.72]<03:59.72>Close <04:03.73>to <04:04.93>you
[04:09.34]<04:09.34>Close <04:15.00>to <04:16.19>you
[04:19.86]<04:19.86>Close <04:21.22>to <04:25.25>you
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (22520, 'lrc', 'line', 'local_lrc', '[00:17.23]<00:17.23>So <00:17.99>I&apos;ll <00:18.23>sit <00:18.61>here <00:19.04>waiting
[00:19.49]<00:19.49>While <00:19.79>the <00:20.17>worlds <00:20.67>will <00:21.29>pass <00:21.48>me
[00:21.86]<00:21.86>You <00:22.10>can&apos;t <00:22.49>have <00:22.73>me
[00:23.73]<00:23.73>Walk <00:23.98>by <00:24.16>me
[00:24.48]<00:24.48>I <00:24.72>can&apos;t <00:25.10>say <00:25.35>a <00:25.73>word
[00:26.35]<00:26.35>I&apos;m <00:26.72>helpless
[00:27.47]<00:27.47>Can&apos;t <00:27.72>help <00:28.16>me
[00:29.03]<00:29.03>You <00:29.41>can&apos;t <00:29.60>stop <00:29.91>me <00:30.46>now
[00:32.22]<00:32.22>&apos;Cause <00:33.14>I <00:34.14>wanna <00:34.88>be <00:35.32>bad <00:37.48>for <00:39.16>you
[00:41.47]<00:41.47>You <00:41.78>just <00:42.04>lead <00:42.48>the <00:42.78>way
[00:43.53]<00:43.53>I&apos;ll <00:43.73>follow
[00:44.59]<00:44.59>I <00:45.03>don&apos;t <00:46.35>wanna <00:46.98>think <00:48.59>it <00:50.28>through
[00:52.46]<00:52.46>Never <00:53.33>give <00:53.76>a <00:54.15>thought <00:54.45>tomorrow
[01:06.74]<01:06.74>Lost <01:07.06>in <01:07.31>thoughts <01:07.99>of <01:08.25>empty <01:08.81>dispositions
[01:09.99]<01:09.99>My <01:10.55>position <01:11.24>stops <01:12.05>me
[01:12.98]<01:12.98>Can&apos;t <01:13.18>stand <01:13.48>up
[01:13.86]<01:13.86>I <01:14.11>can&apos;t <01:14.49>breathe
[01:14.98]<01:14.98>You&apos;re <01:15.30>telling <01:15.86>me
[01:16.67]<01:16.67>You&apos;re <01:16.85>not <01:17.11>for <01:17.35>me
[01:17.79]<01:17.79>But <01:18.04>I&apos;ll <01:18.48>just <01:18.79>make <01:19.04>you <01:19.47>see
[01:21.60]<01:21.60>That <01:22.47>I <01:23.66>wanna <01:24.34>be <01:25.21>bad <01:26.65>for <01:28.45>you
[01:30.71]<01:30.71>You <01:31.27>just <01:31.52>lead <01:31.89>the <01:32.27>way
[01:32.71]<01:32.71>I&apos;ll <01:32.95>follow
[01:33.89]<01:33.89>I <01:34.52>don&apos;t <01:35.82>wanna <01:36.51>think <01:38.07>it <01:39.69>through
[01:42.00]<01:42.00>Never <01:42.81>give <01:43.18>a <01:43.44>thought <01:43.88>tomorrow
[01:44.69]<01:44.69>I&apos;ll <01:45.62>never <01:46.99>be <01:47.43>like <01:48.05>your <01:48.86>kind <01:49.55>of <01:50.30>girl
[01:51.86]<01:51.86>Never <01:52.68>be <01:53.05>right <01:53.98>there <01:54.54>in <01:55.10>that <01:55.85>world
[01:57.54]<01:57.54>But <01:57.91>I&apos;ll <01:58.23>entertain <01:58.79>the <01:59.22>notion
[02:01.72]<02:01.72>That <02:02.34>I <02:02.60>could <02:02.97>live <02:03.47>there <02:04.22>too
[02:30.18]<02:30.18>So <02:30.48>I&apos;ll <02:30.74>sit <02:30.98>here <02:31.36>waiting
[02:31.99]<02:31.99>While <02:32.48>the <02:32.73>worlds <02:33.23>all <02:33.67>pass <02:34.10>me
[02:34.41]<02:34.41>You <02:34.61>can&apos;t <02:34.86>help <02:35.35>me
[02:36.04]<02:36.04>I <02:36.29>won&apos;t <02:36.48>sleep <02:36.67>tonight
[02:41.92]<02:41.92>&apos;Cause <02:42.74>I <02:44.12>wanna <02:44.68>be <02:45.50>bad <02:47.32>for <02:48.24>you
[02:51.11]<02:51.11>You <02:51.61>just <02:51.86>lead <02:52.23>the <02:52.49>way
[02:53.18]<02:53.18>I&apos;ll <02:53.42>follow
[02:54.73]<02:54.73>I <02:55.04>don&apos;t <02:56.29>wanna <02:56.66>think <02:58.60>it <03:00.09>through
[03:02.34]<03:02.34>Never <03:03.09>give <03:03.78>a <03:04.03>thought <03:04.40>tomorrow
[03:09.96]<03:09.96>For <03:10.58>you
[03:16.82]<03:16.82>I <03:18.19>wanna <03:18.88>be <03:19.56>bad <03:21.18>for <03:23.05>you
[03:38.34]<03:38.34>Tomorrow
[03:39.47]<03:39.47>I <03:40.90>wanna <03:41.59>be <03:42.34>bad <03:43.96>for <03:45.66>you
[03:47.77]<03:47.77>Never <03:48.64>give <03:49.02>a <03:49.26>thought <03:49.71>tomorrow
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (17580, 'lrc', 'line', 'local_lrc', '[00:06.99]Love''s a surprise to me
[00:10.05]Brings out the highs in me
[00:13.06]That''s not the way it used to be
[00:16.42]I used to be blue
[00:18.48]Then you showed me that love was near
[00:21.77]And that''s all I had to hear
[00:24.39]I can''t believe that dreams come true
[00:27.82]''Til I run with you
[00:29.90]Now, I can''t sleep at night
[00:34.72]''Til I run with you
[00:37.70]''Til I do, nothin'' will be right
[00:41.37]I''m goin'' downtown takin'' in a look or two
[00:46.23]''Til I run with you
[00:49.14]What else can I do?
[00:52.56]You came one lonely day (you know, you came one lonely day)
[00:55.40]It took me another way (took me another way)
[00:57.91]I''d pack up my heart and run away (run away)
[01:00.84]But I got some things to do
[01:03.70]I''ll have to wait another day
[01:07.24]''Til I run with you
[01:08.82]Whoa, oh-oh...
[01:13.66]
[01:20.67]Nothin'' will be right
[01:22.71]''Til I run with you
[01:25.23]Until I run with you
[01:27.96]
[01:31.62]Nothin'' will be right ''til I run with you (I can''t sleep at night)
[01:36.94]''Til I run with you (nothin'' will be right)
[01:39.81]''Til I run with you (I can''t sleep at night)
[01:42.64]''Til I run with you (nothin'' will be right)
[01:45.68]''Til I run with you (I can''t sleep at night)
[01:48.51]''Til I run with you (nothin'' will be right)
[01:50.98]
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (7234, 'lrc', 'line', 'local_lrc', '[00:17.88]<00:17.88>(<00:17.88>Ummmm<00:18.00>,<00:18.00> <00:19.67>Oh<00:19.98> <00:19.98>Yeah<00:20.22>)<00:20.22> <00:20.22>Dearest<00:20.94> <00:20.94>-<00:20.94> <00:20.95>Buddy<00:21.71> <00:21.71>Holly
[00:24.64]<00:24.64>Please<00:25.16> <00:25.16>don''t<00:25.78> <00:25.79>ever<00:27.25> <00:27.25>-<00:27.25> <00:27.25>umm<00:27.27> <00:27.28>ya
[00:28.81]<00:28.81>Ever<00:29.12> <00:29.12>say<00:29.59> <00:29.59>we''ll<00:30.12> <00:30.12>part
[00:33.29]<00:33.29>You<00:33.93> <00:33.93>scold<00:36.36> <00:36.98>and<00:37.37> <00:37.37>you<00:37.66> <00:37.66>were<00:37.96> <00:37.96>so<00:38.58> <00:38.58>bold
[00:41.47]<00:41.47>Yes<00:42.04> <00:42.04>together<00:44.17> <00:44.17>-<00:44.17> <00:44.17>umm<00:44.19> <00:44.20>ya
[00:45.46]<00:45.46>Our<00:45.95> <00:45.96>love<00:46.29> <00:46.29>will<00:46.53> <00:46.53>grow<00:47.04> <00:47.05>old<00:47.97> <00:47.97>-<00:47.97> <00:47.97>umm<00:47.99> <00:48.40>ya
[00:49.75]<00:49.75>Our<00:50.23> <00:50.23>love<00:50.54> <00:50.54>will<00:50.81> <00:50.81>grow<00:51.31> <00:51.32>old
[00:54.70]<00:54.70>You<00:55.15> <00:55.15>may<00:57.40> <00:57.40>be<00:58.27> <00:58.27>a<00:58.41> <00:58.41>million<00:58.93> <00:58.93>miles<00:59.45> <00:59.45>away
[01:02.86]<01:02.86>Please<01:03.36> <01:03.36>believe<01:04.85> <01:04.85>me<01:05.77> <01:05.77>-<01:05.77> <01:05.77>umm<01:05.89> <01:05.89>ya
[01:06.91]<01:06.91>When<01:07.19> <01:07.19>you<01:07.33> <01:07.33>hear<01:07.70> <01:07.70>me<01:08.06> <01:08.06>say
[01:11.47]<01:11.47>I<01:11.90> <01:12.27>love<01:12.74> <01:12.74>you<01:12.82> <01:12.82>-<01:12.82> <01:15.56>i<01:16.54> <01:16.54>love<01:16.86> <01:16.86>you
[01:19.96]<01:19.96>Come<01:20.53> <01:20.53>home<01:22.63> <01:22.63>-<01:22.63> <01:22.63>keep<01:23.43> <01:23.43>me<01:23.69> <01:23.69>from<01:24.06> <01:24.06>these<01:24.23> <01:24.23>sleepless<01:24.98> <01:24.98>nights
[01:27.83]<01:27.83>Try<01:28.35> <01:28.35>my<01:28.95> <01:28.95>love<01:29.52> <01:29.52>again<01:30.83> <01:30.83>-<01:30.83> <01:30.83>umm<01:30.85> <01:30.85>ya
[01:31.76]<01:31.76>I''m<01:32.07> <01:32.07>gonna<01:32.32> <01:32.33>treat<01:32.92> <01:32.92>you<01:33.35> <01:33.35>right<01:34.26> <01:34.26>-<01:34.26> <01:34.37>umm<01:34.39> <01:34.99>ya
[01:36.12]<01:36.12>I''m<01:36.39> <01:36.39>gonna<01:36.64> <01:36.64>treat<01:37.23> <01:37.23>you<01:37.65> <01:37.65>right
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (7612, 'lrc', 'line', 'local_lrc', '[00:00.17]<00:00.17>(<00:00.29>What <00:00.39>A) <00:00.49>Wonderful <00:00.59>World - <00:00.68>Sam <00:00.77>Cooke
[00:00.89]<00:00.89>Lyrics <00:00.99>by：<00:01.11>Lou <00:01.20>Adler/<00:01.29>Alpert <00:01.41>Herb/<00:01.53>Sam <00:01.61>Cooke
[00:01.72]<00:01.72>Composed <00:01.83>by：<00:01.93>Lou <00:02.05>Adler/<00:02.29>Alpert <00:02.46>Herb/<00:02.65>Sam <00:02.98>Cooke
[00:04.73]<00:04.73>Don''t <00:04.86>know <00:05.02>much <00:05.40>about <00:05.89>history
[00:08.37]<00:08.37>Don''t <00:08.51>know <00:08.75>much <00:09.19>biology
[00:12.09]<00:12.09>Don''t <00:12.24>know <00:12.53>much <00:12.94>about <00:13.06>a <00:13.48>science <00:14.21>book
[00:15.74]<00:15.74>Don''t <00:15.89>know <00:16.19>much <00:16.64>about <00:16.77>the <00:17.20>French <00:17.64>I <00:17.97>took
[00:19.61]<00:19.61>But <00:19.77>I <00:19.89>do <00:20.34>know <00:20.52>that <00:20.86>I <00:21.02>love <00:21.69>you
[00:23.26]<00:23.26>And <00:23.41>I <00:23.76>know <00:24.11>that <00:24.35>if <00:24.51>you <00:24.89>love <00:25.25>me <00:25.55>too
[00:26.37]<00:26.37>What <00:26.66>a <00:26.87>wonderful <00:27.49>world <00:27.93>this <00:28.13>would <00:28.53>be
[00:30.92]<00:30.92>Don''t <00:31.08>know <00:31.25>much <00:31.62>about <00:32.13>geography
[00:34.51]<00:34.51>Don''t <00:34.65>know <00:34.90>much <00:35.41>trigonometry
[00:38.32]<00:38.32>Don''t <00:38.46>know <00:38.69>much <00:39.12>about <00:39.63>algebra
[00:41.90]<00:41.90>Don''t <00:42.05>know <00:42.23>what <00:42.40>a <00:42.79>slide <00:43.30>rule <00:43.74>is <00:44.12>for
[00:45.85]<00:45.85>But <00:45.99>I <00:46.12>do <00:46.46>know <00:46.78>one <00:46.95>and <00:47.20>one <00:47.70>is <00:47.96>two
[00:49.42]<00:49.42>And <00:49.57>if <00:49.86>this <00:50.08>one <00:50.63>could <00:50.99>be <00:51.35>with <00:51.63>you
[00:52.45]<00:52.45>What <00:52.64>a <00:52.76>wonderful <00:53.57>world <00:54.01>this <00:54.39>would <00:54.72>be
[00:55.79]<00:55.79>Now <00:56.27>I <00:56.44>don''t <00:57.09>claim <00:58.20>to <00:58.34>be <00:58.71>an <00:59.04>A <00:59.28>student
[01:00.79]<01:00.79>But <01:00.96>I''m <01:01.14>trying <01:01.86>to <01:02.06>be
[01:03.86]<01:03.86>For <01:04.02>maybe <01:04.45>by <01:04.60>being <01:05.69>an <01:05.85>A <01:06.29>student <01:06.65>baby
[01:08.08]<01:08.08>I <01:08.24>can <01:08.56>win <01:08.88>your <01:09.50>love <01:09.87>for <01:10.22>me
[01:11.82]<01:11.82>Don''t <01:12.07>know <01:12.29>much <01:12.66>about <01:13.10>history
[01:15.38]<01:15.38>Don''t <01:15.54>know <01:15.86>much <01:16.43>biology
[01:19.26]<01:19.26>Don''t <01:19.40>know <01:19.65>much <01:19.87>about <01:20.11>a <01:20.59>science <01:21.33>book
[01:22.78]<01:22.78>Don''t <01:22.94>know <01:23.29>much <01:23.66>about <01:23.79>the <01:24.28>French <01:24.71>I <01:25.05>took
[01:26.69]<01:26.69>But <01:26.83>I <01:27.00>do <01:27.34>know <01:27.87>that <01:28.04>I <01:28.40>love <01:28.81>you
[01:30.26]<01:30.26>And <01:30.39>I <01:30.80>know <01:31.03>that <01:31.22>if <01:31.49>you <01:31.74>love <01:32.19>me <01:32.52>too
[01:33.38]<01:33.38>What <01:33.56>a <01:33.72>wonderful <01:34.38>world <01:34.92>this <01:35.13>would <01:35.59>be
[01:36.55]<01:36.55>La-<01:36.75>ta-<01:36.91>ta-<01:37.12>ta-<01:37.40>ta-<01:37.94>ta-<01:38.43>ta <01:39.35>history
[01:40.51]<01:40.51>Hm <01:42.59>Biology
[01:44.07]<01:44.07>Wa-<01:44.22>la-<01:44.37>ta-<01:44.55>ta-<01:44.73>ta-<01:44.93>ta-<01:45.18>ta-<01:45.57>ta-<01:45.97>ta-<01:46.41>ta <01:46.79>Science <01:47.28>book
[01:47.63]<01:47.63>Hm <01:50.50>French <01:50.64>I <01:50.86>took <01:51.49>yeah
[01:52.59]<01:52.59>But <01:52.76>I <01:52.88>do <01:53.26>know <01:53.76>that <01:53.92>I <01:54.25>love <01:54.64>you
[01:56.13]<01:56.13>And <01:56.27>I <01:56.60>know <01:56.97>that <01:57.12>if <01:57.40>you <01:57.76>love <01:58.05>me <01:58.39>too
[01:59.10]<01:59.10>What <01:59.26>a <01:59.48>wonderful <02:00.07>world <02:00.68>this <02:01.01>would <02:01.38>be
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (7613, 'lrc', 'line', 'local_lrc', '[00:00.17]<00:00.17>(<00:00.29>What <00:00.39>A) <00:00.49>Wonderful <00:00.59>World - <00:00.68>Sam <00:00.77>Cooke
[00:00.89]<00:00.89>Lyrics <00:00.99>by：<00:01.11>Lou <00:01.20>Adler/<00:01.29>Alpert <00:01.41>Herb/<00:01.53>Sam <00:01.61>Cooke
[00:01.72]<00:01.72>Composed <00:01.83>by：<00:01.93>Lou <00:02.05>Adler/<00:02.29>Alpert <00:02.46>Herb/<00:02.65>Sam <00:02.98>Cooke
[00:04.73]<00:04.73>Don''t <00:04.86>know <00:05.02>much <00:05.40>about <00:05.89>history
[00:08.37]<00:08.37>Don''t <00:08.51>know <00:08.75>much <00:09.19>biology
[00:12.09]<00:12.09>Don''t <00:12.24>know <00:12.53>much <00:12.94>about <00:13.06>a <00:13.48>science <00:14.21>book
[00:15.74]<00:15.74>Don''t <00:15.89>know <00:16.19>much <00:16.64>about <00:16.77>the <00:17.20>French <00:17.64>I <00:17.97>took
[00:19.61]<00:19.61>But <00:19.77>I <00:19.89>do <00:20.34>know <00:20.52>that <00:20.86>I <00:21.02>love <00:21.69>you
[00:23.26]<00:23.26>And <00:23.41>I <00:23.76>know <00:24.11>that <00:24.35>if <00:24.51>you <00:24.89>love <00:25.25>me <00:25.55>too
[00:26.37]<00:26.37>What <00:26.66>a <00:26.87>wonderful <00:27.49>world <00:27.93>this <00:28.13>would <00:28.53>be
[00:30.92]<00:30.92>Don''t <00:31.08>know <00:31.25>much <00:31.62>about <00:32.13>geography
[00:34.51]<00:34.51>Don''t <00:34.65>know <00:34.90>much <00:35.41>trigonometry
[00:38.32]<00:38.32>Don''t <00:38.46>know <00:38.69>much <00:39.12>about <00:39.63>algebra
[00:41.90]<00:41.90>Don''t <00:42.05>know <00:42.23>what <00:42.40>a <00:42.79>slide <00:43.30>rule <00:43.74>is <00:44.12>for
[00:45.85]<00:45.85>But <00:45.99>I <00:46.12>do <00:46.46>know <00:46.78>one <00:46.95>and <00:47.20>one <00:47.70>is <00:47.96>two
[00:49.42]<00:49.42>And <00:49.57>if <00:49.86>this <00:50.08>one <00:50.63>could <00:50.99>be <00:51.35>with <00:51.63>you
[00:52.45]<00:52.45>What <00:52.64>a <00:52.76>wonderful <00:53.57>world <00:54.01>this <00:54.39>would <00:54.72>be
[00:55.79]<00:55.79>Now <00:56.27>I <00:56.44>don''t <00:57.09>claim <00:58.20>to <00:58.34>be <00:58.71>an <00:59.04>A <00:59.28>student
[01:00.79]<01:00.79>But <01:00.96>I''m <01:01.14>trying <01:01.86>to <01:02.06>be
[01:03.86]<01:03.86>For <01:04.02>maybe <01:04.45>by <01:04.60>being <01:05.69>an <01:05.85>A <01:06.29>student <01:06.65>baby
[01:08.08]<01:08.08>I <01:08.24>can <01:08.56>win <01:08.88>your <01:09.50>love <01:09.87>for <01:10.22>me
[01:11.82]<01:11.82>Don''t <01:12.07>know <01:12.29>much <01:12.66>about <01:13.10>history
[01:15.38]<01:15.38>Don''t <01:15.54>know <01:15.86>much <01:16.43>biology
[01:19.26]<01:19.26>Don''t <01:19.40>know <01:19.65>much <01:19.87>about <01:20.11>a <01:20.59>science <01:21.33>book
[01:22.78]<01:22.78>Don''t <01:22.94>know <01:23.29>much <01:23.66>about <01:23.79>the <01:24.28>French <01:24.71>I <01:25.05>took
[01:26.69]<01:26.69>But <01:26.83>I <01:27.00>do <01:27.34>know <01:27.87>that <01:28.04>I <01:28.40>love <01:28.81>you
[01:30.26]<01:30.26>And <01:30.39>I <01:30.80>know <01:31.03>that <01:31.22>if <01:31.49>you <01:31.74>love <01:32.19>me <01:32.52>too
[01:33.38]<01:33.38>What <01:33.56>a <01:33.72>wonderful <01:34.38>world <01:34.92>this <01:35.13>would <01:35.59>be
[01:36.55]<01:36.55>La-<01:36.75>ta-<01:36.91>ta-<01:37.12>ta-<01:37.40>ta-<01:37.94>ta-<01:38.43>ta <01:39.35>history
[01:40.51]<01:40.51>Hm <01:42.59>Biology
[01:44.07]<01:44.07>Wa-<01:44.22>la-<01:44.37>ta-<01:44.55>ta-<01:44.73>ta-<01:44.93>ta-<01:45.18>ta-<01:45.57>ta-<01:45.97>ta-<01:46.41>ta <01:46.79>Science <01:47.28>book
[01:47.63]<01:47.63>Hm <01:50.50>French <01:50.64>I <01:50.86>took <01:51.49>yeah
[01:52.59]<01:52.59>But <01:52.76>I <01:52.88>do <01:53.26>know <01:53.76>that <01:53.92>I <01:54.25>love <01:54.64>you
[01:56.13]<01:56.13>And <01:56.27>I <01:56.60>know <01:56.97>that <01:57.12>if <01:57.40>you <01:57.76>love <01:58.05>me <01:58.39>too
[01:59.10]<01:59.10>What <01:59.26>a <01:59.48>wonderful <02:00.07>world <02:00.68>this <02:01.01>would <02:01.38>be
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (22192, 'lrc', 'line', 'local_lrc', '[00:13.28]<00:13.28>One<00:13.44> <00:13.60>day<00:13.74> <00:13.88>you''re<00:13.98> <00:14.08>up<00:14.27> <00:14.45>and<00:14.60> <00:14.68>the<00:14.74> <00:14.80>next<00:14.97> <00:15.14>day<00:15.29> <00:15.43>you''re<00:15.50> <00:15.57>down
[00:16.23]<00:16.23>You<00:16.29> <00:16.35>can''t<00:16.68> <00:16.74>face<00:16.91> <00:17.07>the<00:17.10> <00:17.13>world<00:17.34> <00:17.55>with<00:17.73> <00:17.82>your<00:17.89> <00:17.97>head<00:18.12> <00:18.27>to<00:18.43> <00:18.60>the<00:18.68> <00:18.75>ground
[00:19.56]<00:19.56>The<00:19.62> <00:19.68>grass<00:19.83> <00:19.98>is<00:20.04> <00:20.10>always<00:20.30> <00:20.49>greener<00:20.70> <00:20.91>on<00:21.02> <00:21.12>the<00:21.23> <00:21.33>other<00:21.46> <00:21.60>side,<00:21.75> <00:21.90>they<00:21.96> <00:22.02>say
[00:22.70]<00:22.70>So<00:22.76> <00:22.82>don''t<00:23.02> <00:23.21>worry,<00:23.39> <00:23.57>boys,<00:23.80> <00:24.02>life<00:24.16> <00:24.29>will<00:24.34> <00:24.38>be<00:24.56> <00:24.74>sweet<00:24.88> <00:25.01>some<00:25.10> <00:25.19>day
[00:26.07]<00:26.07>Oh,<00:26.09> <00:26.10>oh,<00:26.11> <00:26.13>oh,<00:26.14> <00:26.16>oh,<00:26.18> <00:26.19>oh,<00:26.46> <00:26.52>oh,<00:29.10> <00:29.28>oh,<00:29.30> <00:29.31>oh,<00:29.32> <00:29.34>oh
[00:29.38]<00:29.38>Oh,<00:29.39> <00:29.41>oh,<00:29.43> <00:29.44>oh,<00:29.74> <00:30.04>oh,<00:30.16> <00:30.28>oh,<00:30.43> <00:30.76>oh,<00:31.45> <00:32.14>oh,<00:32.15> <00:32.17>oh,<00:32.18> <00:32.20>oh
[00:32.31]<00:32.31>We<00:32.34> <00:32.37>made<00:32.85> <00:33.15>enough<00:33.39> <00:33.63>mistakes
[00:36.07]<00:36.07>But<00:36.22> <00:36.37>you<00:36.42> <00:36.48>know<00:36.69> <00:36.90>we<00:37.07> <00:37.25>got<00:37.45> <00:37.55>what<00:37.65> <00:37.75>it<00:37.81> <00:37.87>takes
[00:38.40]<00:38.40>Oh,<00:38.56> <00:38.73>we<00:38.80> <00:38.88>ain''t<00:39.05> <00:39.21>got<00:39.39> <00:39.57>nothin''<00:39.90> <00:40.23>yet
[00:41.45]<00:41.45>No,<00:41.65> <00:41.84>we<00:42.03> <00:42.23>ain''t<00:42.32> <00:42.38>got<00:42.58> <00:42.77>nothin''<00:43.07> <00:43.37>yet
[00:58.44]<00:58.44>Nothin''<00:58.78> <00:59.13>can<00:59.20> <00:59.28>hold<00:59.49> <00:59.70>us<00:59.87> <01:00.03>and<01:00.07> <01:00.12>nothin''<01:00.43> <01:00.75>can<01:00.84> <01:00.93>keep<01:01.09> <01:01.26>us<01:01.32> <01:01.38>down
[01:01.65]<01:01.65>And<01:01.70> <01:01.74>someday<01:02.10> <01:02.46>our<01:02.52> <01:02.58>names<01:02.91> <01:03.18>will<01:03.23> <01:03.27>be<01:03.31> <01:03.36>spread<01:03.60> <01:03.84>all<01:04.04> <01:04.23>over<01:04.42> <01:04.62>town
[01:05.07]<01:05.07>We<01:05.25> <01:05.43>can<01:05.59> <01:05.76>get<01:05.85> <01:05.94>in<01:06.12> <01:06.30>while<01:06.43> <01:06.57>the<01:06.61> <01:06.66>getting<01:07.00> <01:07.35>is<01:07.42> <01:07.50>good
[01:08.08]<01:08.08>So<01:08.17> <01:08.26>make<01:08.39> <01:08.53>it<01:08.61> <01:08.68>on<01:08.80> <01:08.92>your<01:09.03> <01:09.13>own,<01:09.27> <01:09.40>yeah,<01:09.54> <01:09.67>you<01:09.76> <01:09.85>know<01:10.04> <01:10.24>that<01:10.38> <01:10.51>you<01:10.59> <01:10.66>could
[01:11.44]<01:11.44>Oh,<01:11.46> <01:11.47>oh,<01:11.48> <01:11.50>oh,<01:11.52> <01:11.53>oh,<01:12.05> <01:12.58>oh,<01:12.59>oh,<01:12.59>oh,<01:12.60>oh,<01:12.61> <01:13.00>oh
[01:13.17]<01:13.17>Oh,<01:14.52> <01:14.73>oh,<01:14.74> <01:14.76>oh,<01:15.60> <01:16.20>oh,<01:16.27> <01:16.35>oh,<01:16.55>oh,<01:16.74>oh,<01:16.93> <01:17.13>oh,<01:17.14> <01:17.16>oh
[01:17.67]<01:17.67>We<01:17.73> <01:17.91>got<01:18.06> <01:18.21>to<01:18.45> <01:18.69>make<01:18.91> <01:19.14>the<01:19.23> <01:19.32>break
[01:21.59]<01:21.59>''Cause<01:21.74> <01:21.89>we<01:21.93> <01:21.98>got<01:22.22> <01:22.31>too<01:22.52> <01:22.73>much<01:22.97> <01:23.36>at<01:23.39> <01:23.42>stake
[01:23.93]<01:23.93>Oh,<01:24.12> <01:24.32>we<01:24.53> <01:24.74>ain''t<01:24.79> <01:24.83>got<01:25.02> <01:25.22>nothin''<01:25.57> <01:25.91>yet
[01:27.03]<01:27.03>No,<01:27.25> <01:27.48>we<01:27.57> <01:27.66>ain''t<01:27.84> <01:28.02>got<01:28.20> <01:28.38>nothin''<01:28.70> <01:29.01>yet
[01:50.48]<01:50.48>We<01:50.54> <01:50.60>made<01:50.85> <01:51.11>enough<01:51.47> <01:51.83>mistakes
[01:54.36]<01:54.36>But<01:54.49> <01:54.62>you<01:54.68> <01:54.74>know<01:54.94> <01:55.14>we<01:55.33> <01:55.52>got<01:55.66> <01:55.81>what<01:55.90> <01:55.98>it<01:56.05> <01:56.13>takes
[01:56.62]<01:56.62>Oh,<01:56.80> <01:56.98>we<01:57.20> <01:57.43>ain''t<01:57.48> <01:57.52>got<01:57.82> <01:57.91>nothin''<01:58.21> <01:58.51>yet
[01:59.87]<01:59.87>No,<02:00.09> <02:00.31>we<02:00.47> <02:00.63>ain''t<02:00.68> <02:00.72>got<02:00.94> <02:01.16>nothin''<02:01.50> <02:01.84>yet
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (8799, 'lrc', 'line', 'local_lrc', '[00:08.66]My love has no beginning, my love has no end
[00:12.35]No front or back and my love won''t bend
[00:15.14]I''m in the middle, lost in a spin loving you
[00:29.66]
[00:31.70]And you don''t know, you don''t know
[00:32.86]You don''t know, you don''t know how glad I am
[00:40.41]My love has no bottom, my love has no top
[00:43.97]My love won''t rise, and my love won''t drop
[00:46.83]I''m in the middle, and I can''t stop loving you
[01:03.38]And you don''t know, you don''t know
[01:04.32]You don''t know, you don''t know how glad I am
[01:11.04]I wish I were a poet, so I could express
[01:18.32]What I''d, what I''d like to say, yeah
[01:25.51]I wish I were an artist so I could paint a picture
[01:31.96]Of how I feel, of how I feel today
[01:40.46]My love has no walls on either side
[01:43.55]That makes my love wider than wide
[01:45.91]I''m in the middle, and I can''t hide loving you
[02:02.17]And you don''t know, you don''t know
[02:03.22]You don''t know, you don''t know how glad I am
[02:14.19]No you don''t know, you don''t know, you don''t know, you don''t know
[02:17.43]How glad I am
[02:22.82]How glad I am
[02:26.36]♪
[02:29.97]How glad I am
[02:30.99]
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (14661, 'lrc', 'line', 'local_lrc', '[00:06.62]<00:06.62>Tell<00:06.99> <00:06.99>me<00:07.36> <00:07.36>what''s<00:08.28> <00:08.28>in<00:08.46> <00:08.46>a<00:08.52> <00:08.52>kiss
[00:09.68]<00:09.68>If<00:10.06> <00:10.06>your<00:10.38> <00:10.38>heart''s<00:11.20> <00:11.20>not<00:11.99> <00:11.99>in<00:12.12> <00:12.12>it
[00:12.63]<00:12.63>We<00:12.89> <00:12.89>could<00:13.38> <00:13.38>have<00:14.22> <00:14.22>wedded<00:14.66> <00:14.66>bliss
[00:15.62]<00:15.62>If<00:15.98> <00:15.98>we''d<00:16.43> <00:16.44>only<00:17.49> <00:17.49>begin<00:18.12> <00:18.12>it
[00:18.85]<00:18.85>I''m<00:19.18> <00:19.18>feelin''<00:20.41> <00:20.41>low<00:20.67> <00:20.67>down
[00:22.32]<00:22.32>You<00:22.68> <00:22.68>know<00:23.04> <00:23.04>what<00:23.28> <00:23.29>I''m<00:23.74> <00:23.75>talkin''<00:24.67> <00:24.68>about
[00:27.72]<00:27.72>You<00:27.91> <00:27.91>let<00:28.14> <00:28.14>the<00:28.25> <00:28.25>blues<00:28.75> <00:28.75>move<00:28.95> <00:28.95>in<00:29.33>,<00:29.33> <00:29.33>now<00:29.74> <00:29.75>I''m<00:30.70> <00:30.71>movin''<00:31.12> <00:31.13>out
[00:33.49]<00:33.49>This<00:33.74> <00:33.74>old<00:34.00> <00:34.00>house<00:35.05> <00:35.05>ain''t<00:35.41> <00:35.42>a<00:35.54> <00:35.54>home
[00:36.79]<00:36.79>With<00:37.07> <00:37.07>no<00:37.82> <00:37.82>love<00:38.36> <00:38.36>inside<00:38.89> <00:38.89>it
[00:39.24]<00:39.24>We<00:39.58> <00:39.58>set<00:40.22> <00:40.23>out<00:40.87> <00:40.87>pretty<00:41.26> <00:41.26>strong
[00:42.50]<00:42.50>Now<00:42.71> <00:42.71>we<00:42.92> <00:43.03>just<00:43.80> <00:43.80>can''t<00:44.54> <00:44.54>fight<00:44.92> <00:44.92>it
[00:45.54]<00:45.54>I''m<00:46.08> <00:46.09>leavin''<00:47.14> <00:47.14>today
[00:49.02]<00:49.02>I<00:49.40> <00:49.40>don''t<00:49.75> <00:49.75>want<00:50.22> <00:50.22>to<00:50.32> <00:50.48>argue<00:51.28> <00:51.29>or<00:51.47> <00:51.48>shout
[00:54.34]<00:54.34>You<00:54.50> <00:54.50>let<00:54.75> <00:54.75>the<00:54.83> <00:54.84>blues<00:55.28> <00:55.28>move<00:55.53> <00:55.53>in<00:55.92>,<00:55.92> <00:55.92>now<00:56.15> <00:56.16>I''m<00:57.24> <00:57.25>movin''<00:57.73> <00:57.74>out
[01:00.60]<01:00.60>What''s<01:01.17> <01:01.17>the<01:01.35> <01:01.35>use<01:01.97> <01:01.97>in<01:02.26> <01:02.26>buyin''<01:02.72> <01:02.73>a<01:02.77> <01:02.77>car
[01:03.30]<01:03.30>If<01:03.63> <01:03.63>you<01:03.75> <01:03.75>won''t<01:04.05> <01:04.05>buy<01:04.35> <01:04.35>gasoline
[01:06.47]<01:06.47>We<01:06.84> <01:06.84>used<01:07.33> <01:07.33>to<01:07.37> <01:07.37>be<01:07.89> <01:07.89>two<01:08.35> <01:08.35>under<01:08.60> <01:08.60>par
[01:09.00]<01:09.00>Now<01:09.38> <01:09.38>we<01:09.52> <01:09.53>can''t<01:09.92> <01:09.92>get<01:10.35> <01:10.35>on<01:10.76> <01:10.76>the<01:10.82> <01:10.82>green
[01:11.72]<01:11.72>I<01:11.92> <01:11.92>don''t<01:12.31> <01:12.49>know<01:13.21> <01:13.21>where<01:13.51> <01:13.51>it<01:13.83> <01:13.83>went
[01:14.89]<01:14.89>But<01:15.12> <01:15.12>it<01:15.18> <01:15.18>sure<01:16.14> <01:16.14>went<01:16.60> <01:16.60>a<01:16.64>-<01:16.64>flyin''
[01:17.46]<01:17.46>Love''s<01:18.00> <01:18.00>like<01:18.34> <01:18.34>dough<01:19.07> <01:19.07>that''s<01:19.36> <01:19.36>been<01:19.57> <01:19.57>spent
[01:20.69]<01:20.69>Now<01:20.87> <01:20.87>there''s<01:21.12> <01:21.19>no<01:21.98> <01:21.98>use<01:22.46> <01:22.46>a<01:22.50>-<01:22.50>cryin''
[01:23.61]<01:23.61>Tell<01:23.88> <01:23.88>me<01:24.03> <01:24.03>where<01:24.93> <01:24.93>can<01:25.26> <01:25.26>I<01:25.45> <01:25.45>go
[01:27.14]<01:27.14>East<01:27.60> <01:27.60>or<01:27.88> <01:27.88>west<01:28.33> <01:28.33>or<01:28.58> <01:28.58>north<01:28.97> <01:28.98>or<01:29.34> <01:29.35>due<01:29.90> <01:29.90>south
[01:32.49]<01:32.49>You<01:32.65> <01:32.65>let<01:32.87> <01:32.87>the<01:32.97> <01:32.97>blues<01:33.44> <01:33.44>move<01:33.61> <01:33.61>in<01:34.00>,<01:34.00> <01:34.00>now<01:34.49> <01:34.50>I''m<01:35.34> <01:35.35>movin''<01:35.77> <01:35.78>out
[02:04.85]<02:04.85>What''s<02:05.34> <02:05.34>the<02:05.77> <02:05.77>use<02:06.14> <02:06.14>in<02:06.44> <02:06.44>buyin''<02:06.89> <02:06.89>a<02:06.93> <02:06.93>car
[02:07.41]<02:07.41>If<02:07.79> <02:07.79>you<02:07.91> <02:07.91>won''t<02:08.21> <02:08.21>buy<02:08.50> <02:08.50>gasoline
[02:10.63]<02:10.63>We<02:10.99> <02:10.99>used<02:11.46> <02:11.46>to<02:11.51> <02:11.51>be<02:12.17> <02:12.22>two<02:12.51> <02:12.51>under<02:12.75> <02:12.75>par
[02:13.12]<02:13.12>Now<02:13.50> <02:13.51>we<02:13.62> <02:13.63>can''t<02:13.99> <02:13.99>get<02:14.42> <02:14.43>on<02:14.77> <02:14.78>the<02:14.89> <02:14.90>green
[02:15.83]<02:15.83>I<02:16.06> <02:16.06>don''t<02:16.64> <02:16.64>know<02:17.23> <02:17.23>where<02:17.53> <02:17.53>it<02:17.75> <02:17.91>went
[02:18.89]<02:18.89>But<02:19.11> <02:19.11>it<02:19.17> <02:19.17>sure<02:20.11> <02:20.11>went<02:20.57> <02:20.57>a<02:20.61>-<02:20.61>flyin''
[02:21.43]<02:21.43>Love''s<02:21.94> <02:21.94>like<02:22.30> <02:22.30>dough<02:23.03> <02:23.03>that''s<02:23.35> <02:23.35>been<02:23.68> <02:23.68>spent
[02:24.71]<02:24.71>Now<02:24.89> <02:24.89>there''s<02:25.19> <02:25.20>no<02:25.91> <02:25.91>use<02:26.41> <02:26.41>a<02:26.45>-<02:26.45>cryin''
[02:27.63]<02:27.63>Tell<02:27.83> <02:27.83>me<02:27.96> <02:27.96>where<02:28.83> <02:28.83>can<02:29.12> <02:29.12>I<02:29.31> <02:29.31>go
[02:30.98]<02:30.98>East<02:31.49> <02:31.49>or<02:31.78> <02:31.78>west<02:32.21> <02:32.21>or<02:32.49> <02:32.49>north<02:32.95> <02:32.95>or<02:33.21> <02:33.21>due<02:33.84> <02:33.84>south
[02:36.33]<02:36.33>You<02:36.49> <02:36.49>let<02:36.73> <02:36.73>the<02:36.82> <02:36.82>blues<02:37.24> <02:37.24>move<02:37.47> <02:37.47>in<02:37.86>,<02:37.86> <02:37.86>now<02:38.14> <02:38.15>I''m<02:39.25> <02:39.25>movin''<02:39.61> <02:39.62>out
[02:42.13]<02:42.13>You<02:42.29> <02:42.29>let<02:42.52> <02:42.52>the<02:42.62> <02:42.63>blues<02:43.09> <02:43.09>move<02:43.26> <02:43.26>in<02:43.67>,<02:43.67> <02:43.68>now<02:43.98> <02:43.99>I''m<02:45.05> <02:45.05>movin''<02:45.42> <02:45.43>out
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (2870, 'lrc', 'line', 'local_lrc', '[00:03.09]<00:03.09>Crazy
[00:14.87]<00:14.87>Baby
[00:16.73]<00:16.73>I <00:17.41>m <00:17.87>so <00:18.21>into <00:18.63>you
[00:19.64]<00:19.64>You <00:20.30>got <00:20.61>that <00:20.93>something
[00:21.80]<00:21.80>What <00:22.40>can <00:22.72>I <00:23.01>do
[00:24.21]<00:24.21>Baby
[00:26.14]<00:26.14>You <00:26.79>spin <00:27.12>me <00:27.47>around
[00:29.21]<00:29.21>The <00:29.82>Earth <00:30.12>is <00:30.42>moving
[00:31.09]<00:31.09>But <00:31.66>I <00:31.89>can <00:32.09>t <00:32.34>feel <00:32.61>the <00:33.17>ground
[00:33.23]<00:33.23>Every <00:34.06>time <00:34.63>you <00:35.75>look <00:36.75>at <00:37.32>me
[00:38.79]<00:38.79>My <00:39.36>heart <00:39.62>is <00:39.92>jumping
[00:40.52]<00:40.52>It <00:41.13>s <00:41.43>easy <00:41.73>to <00:42.01>see
[00:42.23]<00:42.23>You <00:42.87>drive <00:43.19>me <00:43.51>crazy
[00:45.14]<00:45.14>I <00:45.81>just <00:46.25>can <00:46.72>t <00:47.28>sleep
[00:48.14]<00:48.14>I <00:48.68>m <00:48.92>so <00:49.16>excited
[00:49.87]<00:49.87>I <00:50.42>m <00:50.67>in <00:51.01>too <00:51.39>deep
[00:51.63]<00:51.63>Ohh <00:52.96>crazy
[00:54.34]<00:54.34>But <00:54.98>it <00:55.33>feels <00:55.80>alright
[00:57.41]<00:57.41>Baby <00:58.02>thinking <00:58.33>of <00:58.63>you <00:58.89>keeps <00:59.17>me <00:59.56>up <00:59.84>all <01:00.16>night
[01:02.21]<01:02.21>Tell <01:03.41>me
[01:04.01]<01:04.01>You <01:04.62>re <01:04.99>so <01:05.27>into <01:05.68>me
[01:07.06]<01:07.06>That <01:07.73>I <01:07.99>m <01:08.36>the <01:08.67>only
[01:09.22]<01:09.22>One <01:09.84>you <01:10.15>will <01:10.50>see
[01:11.63]<01:11.63>Tell <01:12.89>me
[01:13.63]<01:13.63>I <01:14.16>m <01:14.43>not <01:14.69>in <01:14.97>the <01:15.28>blue
[01:16.74]<01:16.74>That <01:17.33>I <01:17.63>m <01:17.94>not <01:18.23>wasting
[01:18.54]<01:18.54>My <01:19.11>feelings <01:19.41>on <01:19.82>you
[01:21.13]<01:21.13>Everytime <01:21.93>I <01:22.72>look <01:23.77>at <01:24.30>you
[01:26.24]<01:26.24>My <01:26.83>heart <01:27.14>is <01:27.48>jumping
[01:28.17]<01:28.17>What <01:28.80>can <01:29.15>I <01:29.49>do
[01:29.81]<01:29.81>You <01:30.42>drive <01:30.70>me <01:31.01>crazy
[01:32.49]<01:32.49>I <01:33.30>just <01:33.66>can <01:33.94>t <01:34.24>sleep
[01:35.58]<01:35.58>I <01:36.21>m <01:36.53>so <01:36.82>excited
[01:37.44]<01:37.44>I <01:38.04>m <01:38.35>in <01:38.66>too <01:39.01>deep
[01:39.27]<01:39.27>Ohh <01:40.44>crazy
[01:41.84]<01:41.84>But <01:42.52>it <01:42.82>feels <01:43.56>alright
[01:44.79]<01:44.79>Baby <01:45.46>thinking <01:45.77>of <01:46.10>you <01:46.39>keeps <01:46.69>me <01:47.04>up <01:47.34>all <01:47.98>night
[01:56.66]<01:56.66>You <01:57.20>drive <01:57.44>me <01:57.84>crazy
[02:00.91]<02:00.91>Ohh <02:01.84>crazy
[02:18.16]<02:18.16>You <02:19.50>drive <02:19.93>me <02:20.47>crazy <02:21.50>baby
[02:23.20]<02:23.20>That <02:24.03>s <02:24.37>I <02:24.75>do
[02:24.92]<02:24.92>I <02:25.53>m <02:25.79>in <02:26.07>too <02:26.46>deep
[02:27.65]<02:27.65>Come <02:28.88>body
[02:29.87]<02:29.87>Feels <02:31.10>alright
[02:32.32]<02:32.32>Baby <02:32.98>thinking <02:33.51>of <02:33.89>you <02:34.39>keeps <02:34.78>me <02:35.10>up <02:35.41>all <02:35.77>night
[02:36.32]<02:36.32>You <02:36.91>drive <02:37.17>me <02:37.53>crazy
[02:38.88]<02:38.88>I <02:39.85>just <02:40.31>can <02:40.61>t <02:40.92>sleep
[02:42.26]<02:42.26>I <02:42.83>m <02:43.10>so <02:43.41>excited
[02:43.42]<02:43.42>I''m <02:44.56>in <02:45.04>too <02:45.49>deep
[02:45.83]<02:45.83>Ohh <02:46.87>crazy
[02:48.35]<02:48.35>But <02:49.01>it <02:49.34>feels <02:49.88>alright
[02:51.32]<02:51.32>Baby <02:52.00>thinking <02:52.31>of <02:52.60>you <02:52.91>keeps <02:53.20>me <02:53.58>up <02:54.02>all <02:54.59>night
[02:56.72]<02:56.72>Crazy
[03:00.93]<03:00.93>Crazy
[03:04.98]<03:04.98>You <03:05.54>drive <03:05.79>me <03:06.07>crazy
[03:07.38]<03:07.38>But <03:08.01>it <03:08.35>feels <03:08.80>alright
[03:10.40]<03:10.40>Baby <03:11.04>thinking <03:11.34>of <03:11.64>you <03:11.96>keeps <03:12.25>me <03:12.57>up <03:12.85>all <03:13.92>night
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (914, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>...<00:00.08>Baby <00:00.16>One <00:00.24>More <00:00.33>Time - <00:00.41>Britney <00:00.49>Spears
[00:00.57]<00:00.57>Lyrics <00:00.65>by：<00:00.73>Max <00:00.82>Martin
[00:00.90]<00:00.90>Composed <00:00.98>by：<00:01.06>Max <00:01.14>Martin
[00:01.22]<00:01.22>Produced <00:01.30>by：<00:01.39>Max <00:01.47>Martin/<00:01.55>Rami <00:01.63>Yacoub
[00:01.71]<00:01.71>Oh <00:02.03>baby <00:02.70>baby
[00:06.85]<00:06.85>Oh <00:07.17>baby <00:07.81>baby
[00:12.03]<00:12.03>Oh <00:12.37>baby <00:13.00>baby <00:13.96>how <00:14.28>was <00:14.58>I <00:14.96>supposed <00:16.25>to <00:16.56>know
[00:19.12]<00:19.12>That <00:19.43>something <00:20.07>wasn''t <00:20.73>right <00:21.38>here?
[00:22.32]<00:22.32>Oh <00:22.65>baby <00:23.26>baby <00:24.25>I <00:24.58>shouldn''t <00:25.22>have <00:25.54>let <00:26.55>you <00:26.86>go
[00:29.46]<00:29.46>And <00:29.77>now <00:30.06>you''re <00:30.38>out <00:30.70>of <00:31.02>sight <00:31.70>yeah
[00:32.42]<00:32.42>Show <00:32.96>me <00:33.66>how <00:33.92>you <00:34.26>want <00:34.57>it <00:34.93>to <00:35.62>be
[00:36.86]<00:36.86>Tell <00:37.16>me <00:37.44>baby <00:38.79>''cause <00:39.04>I <00:39.38>need <00:39.72>to <00:40.09>know <00:40.69>now <00:41.38>oh <00:41.66>because
[00:43.02]<00:43.02>My <00:43.30>loneliness <00:44.89>is <00:45.21>killing <00:45.86>me
[00:46.48]<00:46.48>And <00:47.11>I
[00:48.14]<00:48.14>I <00:48.41>must <00:48.80>confess <00:50.02>I <00:50.33>still <00:50.70>believe
[00:51.99]<00:51.99>Still <00:52.28>believe
[00:53.31]<00:53.31>When <00:53.58>I''m <00:53.85>not <00:54.20>with <00:54.52>you <00:54.88>I <00:55.17>lose <00:55.49>my <00:55.81>mind
[00:56.80]<00:56.80>Give <00:57.11>me <00:57.42>a <00:57.74>sign
[01:00.68]<01:00.68>Hit <01:01.01>me <01:01.35>baby <01:01.99>one <01:02.28>more <01:02.62>time
[01:03.65]<01:03.65>Oh <01:03.92>baby <01:04.55>baby <01:05.52>the <01:05.85>reason <01:06.50>I <01:06.82>breathe <01:07.82>is <01:08.11>you
[01:11.08]<01:11.08>Boy <01:11.32>you <01:11.63>got <01:11.94>me <01:12.32>blinded
[01:13.89]<01:13.89>Oh <01:14.21>pretty <01:14.85>baby <01:15.80>there''s <01:15.95>nothing <01:17.17>that <01:17.79>I <01:18.48>wouldn''t <01:19.05>do
[01:21.03]<01:21.03>It''s <01:21.35>not <01:21.60>the <01:21.92>way <01:22.29>I <01:22.63>planned <01:23.24>it
[01:23.92]<01:23.92>Show <01:24.57>me <01:25.24>how <01:25.48>you <01:25.82>want <01:26.14>it <01:26.49>to <01:27.13>be
[01:28.39]<01:28.39>Tell <01:28.73>me <01:29.11>baby <01:30.38>''cause <01:30.66>I <01:30.99>need <01:31.28>to <01:31.66>know <01:32.28>now <01:32.96>oh <01:33.21>because
[01:34.58]<01:34.58>My <01:34.89>loneliness <01:36.47>is <01:36.81>killing <01:37.45>me
[01:38.08]<01:38.08>And <01:38.72>I
[01:39.73]<01:39.73>I <01:39.95>must <01:40.38>confess <01:41.65>I <01:41.95>still <01:42.35>believe
[01:43.58]<01:43.58>Still <01:43.89>believe
[01:44.93]<01:44.93>When <01:45.22>I''m <01:45.48>not <01:45.80>with <01:46.10>you <01:46.49>I <01:46.78>lose <01:47.15>my <01:47.41>mind
[01:48.45]<01:48.45>Give <01:48.69>me <01:49.05>a <01:49.38>sign
[01:52.31]<01:52.31>Hit <01:52.60>me <01:52.98>baby <01:53.62>one <01:53.89>more <01:54.23>time
[01:55.21]<01:55.21>Oh <01:55.51>baby <01:56.18>baby
[02:00.39]<02:00.39>Oh <02:00.70>baby <02:01.32>baby
[02:05.52]<02:05.52>Oh <02:05.86>baby <02:06.49>baby <02:07.46>how <02:07.79>was <02:08.13>I <02:08.44>supposed <02:09.73>to <02:10.08>know?
[02:15.96]<02:15.96>Oh <02:16.21>pretty <02:16.82>baby <02:17.81>I <02:18.14>shouldn''t <02:18.78>have <02:19.08>let <02:19.79>you <02:20.47>go
[02:24.66]<02:24.66>I <02:24.91>must <02:25.24>confess <02:27.03>that <02:27.20>my <02:27.47>loneliness
[02:29.48]<02:29.48>Is <02:29.72>killing <02:30.34>me <02:30.68>now
[02:32.95]<02:32.95>Don''t <02:33.25>you <02:33.59>know <02:34.25>I <02:34.86>still <02:35.55>believe
[02:37.22]<02:37.22>That <02:37.46>you <02:37.79>will <02:38.09>be <02:38.46>here
[02:39.76]<02:39.76>And <02:40.07>give <02:40.35>me <02:40.70>a <02:41.02>sign?
[02:43.96]<02:43.96>Hit <02:44.27>me <02:44.61>baby <02:45.27>one <02:45.54>more <02:45.92>time
[02:46.86]<02:46.86>My <02:47.19>loneliness <02:48.78>is <02:49.10>killing <02:49.76>me
[02:50.37]<02:50.37>And <02:51.00>I
[02:51.96]<02:51.96>I <02:52.25>must <02:52.65>confess <02:53.93>I <02:54.27>still <02:54.82>believe
[02:55.86]<02:55.86>Still <02:56.20>believe
[02:57.16]<02:57.16>When <02:57.46>I''m <02:57.76>not <02:58.14>with <02:58.44>you <02:58.75>I <02:59.08>lose <02:59.42>my <02:59.77>mind
[03:00.75]<03:00.75>Give <03:01.00>me <03:01.30>a <03:01.69>sign
[03:04.56]<03:04.56>Hit <03:04.85>me <03:05.20>baby <03:05.87>one <03:06.09>more <03:06.24>time
[03:06.39]<03:06.39>I <03:06.53>must <03:06.69>confess <03:08.20>that <03:08.46>my <03:08.80>loneliness
[03:09.59]<03:09.59>My <03:09.75>loneliness <03:09.90>is <03:10.03>killing <03:10.44>me
[03:10.82]<03:10.82>Is <03:11.05>killing <03:11.70>me <03:12.06>now
[03:12.88]<03:12.88>I <03:13.06>must <03:13.46>confess <03:14.61>I <03:14.91>still <03:15.22>believe
[03:15.41]<03:15.41>Don''t <03:15.55>you <03:15.70>know <03:15.85>I <03:16.19>still <03:16.90>believe
[03:17.93]<03:17.93>When <03:18.13>I''m <03:18.43>not <03:18.76>with <03:19.07>you <03:19.47>I <03:19.75>lose <03:20.11>my <03:20.39>mind
[03:20.54]<03:20.54>That <03:20.67>you <03:20.83>will <03:20.97>be <03:21.14>here
[03:21.30]<03:21.30>And <03:21.47>give <03:21.63>me <03:21.94>a <03:22.33>sign?
[03:25.21]<03:25.21>Hit <03:25.53>me <03:25.89>baby <03:26.56>one <03:26.84>more <03:27.18>time
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (4670, 'lrc', 'line', 'local_lrc', '[00:01.84]Yeah!
[00:07.69]Kick it!
[00:21.74]You wake up late for school, man, you don''t wanna go
[00:29.49]You ask your mom, "Please?", but she still says, "No!"
[00:36.62]You missed two classes and no homework
[00:43.76]But your teacher preaches class like you''re some kind of jerk
[00:50.25]You gotta fight
[00:52.56]For your right
[00:54.93]To party
[01:02.75]Your pops caught you smoking and he says, "No way!"
[01:08.53]That hypocrite smokes two packs a day
[01:15.83]Man, livin'' at home is such a drag
[01:23.25]Now, your mom threw away your best porno mag (busted)
[01:30.11]You gotta fight
[01:31.91]For your right
[01:33.92]To party
[01:40.93]You gotta fight
[02:02.94]Don''t step out of this house if that''s the clothes you''re gonna wear
[02:09.29]I''ll kick you out of my home if you don''t cut that hair
[02:16.89]Your mom busted in and said, "What''s that noise?"
[02:24.28]Oh mom, you''re just jealous it''s the Beastie Boys
[02:30.76]You gotta fight
[02:33.21]For your right
[02:34.54]To party
[02:42.04]You gotta fight
[02:43.97]For your right
[02:46.20]To party
[03:01.38]Party!
[03:08.00]Party!
[03:15.99]
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (3986, 'lrc', 'line', 'local_lrc', '[00:09.45]I don''t care if your world is ending today
[00:13.58]Because I wasn''t invited to it anyway
[00:18.38]You said I tasted famous so I drew you a heart
[00:22.26]But now I''m not an artist, I''m a fucking work of art
[00:26.67]I got an F and a C
[00:28.86]And I got a K too
[00:31.12]And the only thing that''s missin''
[00:33.31]Is a bitch like U
[00:36.15]You wanted perfect
[00:38.35]You''ve got your perfect now
[00:40.18]I''m too perfect for someone like you
[00:43.85]I was a dandy in your ghetto
[00:46.09]With the snow white smile
[00:48.22]But you''ll never be as perfect whatever you do
[00:53.08]What''s my name? What''s my name?
[00:55.23]Ah ah ah ah ah
[00:57.88]Hold the S because I am an AINT
[01:01.85]What''s my name? What''s my name?
[01:04.03]Ah ah ah ah ah
[01:06.57]Hold the S because I am an AINT
[01:13.47]Ah ah ah ah
[01:17.15]Ah ah ah ah
[01:18.46]I am a born type of death set on a mop-stick
[01:22.86]You infected me, took diamonds, I took all your shit
[01:27.25]Your ''sell by date'' expired so you had to be sold
[01:31.56]I''m a suffer genius and then a sex symbol
[01:36.52]You wanted perfect
[01:38.99]You''ve got your perfect now
[01:40.93]I''m too perfect for someone like you
[01:44.45]I was a dandy in your ghetto
[01:46.74]With the snow white smile
[01:48.78]But you''ll never be as perfect whatever you do
[01:53.53]What''s my name? What''s my name?
[01:55.92]Ah ah ah ah ah
[01:58.64]Hold the S because I am an AINT
[02:02.40]What''s my name? What''s my name?
[02:04.68]Ah ah ah ah ah
[02:07.37]Hold the S because I am an AINT
[02:13.72]Ah ah ah ah
[02:17.93]Ah ah ah ah
[02:19.22]I got an F and a C
[02:21.31]And I got a K too
[02:23.59]And the only thing that''s missin'' is U
[02:27.61]I got an F and a C
[02:29.93]And I got a K too
[02:32.17]And the only thing that''s missin''
[02:34.35]Is a bitch like U
[02:36.42]I got an F and a C
[02:38.52]And I got a K too
[02:40.78]And the only thing that''s missin''
[02:43.09]Is a bitch like U
[02:45.00]I am the dandy in the ghetto
[02:47.43]With a snow white smile
[02:49.86]Super ego bitch I''ve been evil a while
[02:53.66]I am the dandy in the ghetto
[02:56.09]With a snow white smile
[02:58.62]Super ego bitch I''ve been evil a while
[03:02.88]What''s my name? What''s my name?
[03:05.28]Ah ah ah ah ah
[03:07.56]Hold the S because I am an AINT
[03:11.43]What''s my name? What''s my name?
[03:13.73]Ah ah ah ah ah
[03:16.44]Hold the S because I am an AINT
[03:20.37]What''s my name? What''s my name?
[03:22.55]Ah ah ah ah ah
[03:25.09]Hold the S because I am an AINT
[03:28.98]What''s my name? What''s my name?
[03:31.03]Ah ah ah ah ah
[03:33.76]Hold the S because I am an AINT
[03:37.70]
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (19701, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>(<00:00.06>You<00:00.12> <00:00.17>Make<00:00.23> <00:00.29>Me<00:00.35> <00:00.41>Feel<00:00.46> <00:00.52>Like<00:00.58>)<00:00.64> <00:00.70>A<00:00.75> <00:00.81>Natural<00:00.87> <00:00.93>Woman<00:00.99> <00:01.04>-<00:01.10> <00:01.16>Aretha<00:01.22> <00:01.28>Franklin<00:01.33> <00:01.39>(<00:01.45>艾<00:01.51>瑞<00:01.57>莎<00:01.62>·<00:01.68>弗<00:01.74>兰<00:01.80>克<00:01.86>林<00:01.91>)
[00:01.99]<00:01.99>Written<00:02.12> <00:02.26>by<00:02.39>：<00:02.52>Carole<00:02.65> <00:02.79>King<00:02.92>/<00:03.05>Jerry<00:03.19> <00:03.32>Wexler<00:03.45>/<00:03.59>Gerry<00:03.72> <00:03.85>Goffin
[00:04.00]<00:04.00>Looking <00:04.36>out <00:05.42>on <00:05.64>the <00:05.94>morning <00:06.56>rain
[00:09.68]<00:09.68>I <00:10.06>used <00:10.32>to <00:10.58>feel <00:11.74>so <00:12.22>uninspired
[00:16.26]<00:16.26>And <00:16.55>when <00:16.85>I <00:17.08>knew <00:17.85>I <00:18.19>had <00:18.51>to <00:18.74>face <00:19.28>another <00:19.92>day
[00:22.66]<00:22.66>Lord <00:23.30>it <00:23.55>made <00:23.87>me <00:24.12>feel <00:24.85>so <00:25.43>tired
[00:28.88]<00:28.88>Before <00:29.26>the <00:29.48>day <00:29.75>I <00:30.02>met <00:30.42>you
[00:32.12]<00:32.12>Life <00:32.39>was <00:32.64>so <00:33.06>unkind
[00:34.57]<00:34.57>But <00:34.79>you''re <00:35.05>the <00:35.33>key <00:35.77>to <00:36.25>my <00:36.74>peace <00:37.24>of <00:37.93>mind
[00:39.24]<00:39.24>''Cause <00:39.63>you <00:40.07>make <00:40.56>me <00:41.07>feel
[00:42.63]<00:42.63>You <00:43.19>make <00:43.71>me <00:44.31>feel
[00:45.81]<00:45.81>You <00:46.39>make <00:46.89>me <00:47.38>feel <00:48.01>like <00:48.53>a <00:49.08>natural <00:50.84>woman
[00:52.44]<00:52.44>Woman<00:55.06> when <00:55.46>my <00:55.71>soul <00:56.89>was <00:57.17>in <00:57.35>the <00:57.61>lost <00:57.89>and <00:58.23>found
[01:01.37]<01:01.37>You <01:01.68>came <01:01.99>along <01:03.34>to <01:03.79>claim <01:04.39>it
[01:07.83]<01:07.83>I <01:08.26>didn''t <01:08.67>know <01:09.41>just <01:09.75>what <01:10.05>was <01:10.36>wrong <01:10.61>with <01:10.91>me
[01:13.76]<01:13.76>Till <01:14.09>your <01:14.61>kiss <01:15.60>helped <01:16.12>me <01:16.71>name <01:17.31>it
[01:20.34]<01:20.34>Now <01:20.58>I''m <01:20.78>no <01:21.07>longer <01:21.72>doubtful
[01:23.58]<01:23.58>Of <01:23.87>what <01:24.17>I''m <01:24.47>living <01:24.91>for
[01:26.16]<01:26.16>And <01:26.54>if <01:26.79>I <01:27.06>make <01:27.24>you <01:27.55>happy <01:28.01>I <01:28.21>don''t <01:28.61>need <01:28.89>to <01:29.18>do <01:29.66>more
[01:31.10]<01:31.10>''Cause <01:31.43>you <01:31.83>make <01:32.29>me <01:32.86>feel
[01:34.33]<01:34.33>You <01:34.92>make <01:35.47>me <01:35.93>feel
[01:37.62]<01:37.62>You <01:38.17>make <01:38.68>me <01:39.21>feel <01:39.78>like <01:40.41>a <01:40.88>natural <01:42.62>woman
[01:44.05]<01:44.05>Woman<01:46.90> oh <01:48.02>baby <01:48.55>what <01:48.82>you''ve <01:49.13>done <01:49.44>to <01:49.70>me
[01:50.06]<01:50.06>What <01:50.45>you''ve <01:50.70>done <01:50.99>to <01:51.31>me
[01:52.99]<01:52.99>You <01:53.37>make <01:53.87>me <01:54.17>feel <01:54.87>so <01:55.43>good <01:55.94>inside
[01:57.20]<01:57.20>Good <01:57.66>inside
[02:00.09]<02:00.09>And <02:00.66>I <02:01.59>just <02:01.96>want <02:02.27>to <02:02.49>be <02:03.53>want <02:03.77>to <02:04.25>be
[02:05.56]<02:05.56>Close <02:05.91>to <02:06.21>you <02:06.53>you <02:06.75>make <02:07.05>me <02:07.33>feel <02:08.02>so <02:08.51>alive
[02:10.06]<02:10.06>You <02:10.57>make <02:11.00>me <02:11.48>feel
[02:13.20]<02:13.20>You <02:13.60>make <02:14.05>me <02:14.59>feel
[02:16.39]<02:16.39>You <02:16.78>make <02:17.23>me <02:17.73>feel <02:18.36>like <02:18.85>a <02:19.40>natural <02:21.14>woman
[02:22.94]<02:22.94>You <02:23.42>make <02:23.76>me <02:24.32>feel
[02:26.23]<02:26.23>You <02:26.62>make <02:27.01>me <02:27.56>feel
[02:29.25]<02:29.25>You <02:29.67>make <02:30.15>me <02:30.63>feel <02:31.21>like <02:31.78>a <02:32.27>natural <02:34.05>woman
[02:35.58]<02:35.58>Woman<02:36.26> you <02:36.74>make <02:36.98>me <02:37.36>feel
[02:39.27]<02:39.27>You <02:39.65>make <02:40.04>me <02:40.52>feel
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (5983, 'lrc', 'line', 'local_lrc', '[00:17.37]Everyone it seems
[00:23.51]Has somewhere to go
[00:29.08]
[00:32.95]And the faster the world spins
[00:40.28]The shorter the lights will glow
[00:46.18]
[00:50.03]And I''m swimming in the night
[00:57.29]Chasing down the moon
[01:02.71]
[01:06.95]The deeper in the water
[01:14.16]More I long for you
[01:19.26]
[01:24.31]Most of what you see my dear is purely for show
[01:31.98]Because not everything that goes around comes back around you know
[01:41.37]Holding on too long is just fear of letting go
[01:48.79]Because not everything that goes around comes back around you know
[01:58.21]One thing that is clear, it''s all down hill from here
[02:09.99]
[03:38.40]Love line in your hand
[03:44.72]Cleverly disguised
[03:50.86]
[03:54.49]All the promises of stone
[04:01.56]Crumble in the light
[04:07.92]Most of what you see, my dear, is worth letting go
[04:15.33]Because not everything that goes around comes back around you know
[04:24.75]Holding on too long is just fear of wanting to show
[04:32.00]Because not everything that goes around comes back around you know
[04:41.41]Not everything that goes around comes back around you know
[04:49.94]One thing that is clear, it''s all down hill from here
[05:01.21]
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (8820, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>...Ready<00:00.18> <00:00.35>For<00:00.53> <00:00.71>It<00:00.89>?<00:01.06> <00:01.24>-<00:01.42> <00:01.59>Taylor<00:01.77> <00:01.95>Swift<00:02.12> <00:02.30>(<00:02.48>泰<00:02.65>勒<00:02.83>·<00:03.01>斯<00:03.19>威<00:03.36>夫<00:03.54>特<00:03.72>)
[00:03.90]<00:03.90>Written<00:04.13> <00:04.36>by<00:04.59>：<00:04.82>Ali<00:05.05> <00:05.28>Payami<00:05.51>/<00:05.74>Shellback<00:05.97>/<00:06.20>Max<00:06.43> <00:06.66>Martin<00:06.89>/<00:07.12>Taylor<00:07.35> <00:07.58>Swift
[00:07.81]<00:07.81>Producer<00:08.16>：<00:08.52>Max<00:08.88> <00:09.23>Martin<00:09.59>/<00:09.94>Shellback<00:10.29>/<00:10.65>Ali<00:11.01> <00:11.36>Payami
[00:11.72]<00:11.72>Knew <00:11.90>he <00:12.06>was <00:12.21>a <00:12.36>killer <00:13.00>first <00:13.22>time <00:13.42>that <00:13.60>I <00:13.77>saw <00:14.09>him
[00:14.50]<00:14.50>Wondered <00:14.84>how <00:15.06>many <00:15.30>girls <00:15.63>he <00:15.84>had <00:15.97>loved <00:16.19>and <00:16.43>left <00:16.59>haunted
[00:17.67]<00:17.67>But <00:17.86>if <00:18.00>he''s <00:18.18>a <00:18.32>ghost <00:18.52>then <00:19.03>I <00:19.24>can <00:19.42>be <00:19.62>a <00:19.86>phantom
[00:20.71]<00:20.71>Holdin'' <00:20.89>him <00:21.05>for <00:21.16>ransom
[00:22.25]<00:22.25>Some <00:23.05>some <00:23.50>boys <00:23.80>are <00:24.01>tryin'' <00:24.17>too <00:24.47>hard
[00:25.14]<00:25.14>He <00:25.34>don''t <00:25.50>try <00:25.69>at <00:25.93>all <00:26.14>though
[00:26.51]<00:26.51>Younger <00:26.84>than <00:27.05>my <00:27.21>exes <00:27.65>but <00:27.85>he <00:28.02>act <00:28.21>like <00:28.45>such <00:28.64>a <00:28.81>man <00:29.00>so
[00:29.63]<00:29.63>I <00:29.82>see <00:29.98>nothing <00:30.25>better <00:31.03>I <00:31.25>keep <00:31.45>him <00:31.73>forever
[00:32.59]<00:32.59>Like <00:32.79>a <00:32.95>vendetta-ta
[00:35.22]<00:35.22>I-I-I <00:36.20>see <00:36.92>how <00:37.12>this <00:37.28>is <00:37.44>gon'' <00:37.76>go
[00:39.04]<00:39.04>Touch <00:39.38>me <00:39.78>and <00:39.96>you''ll <00:40.12>never <00:40.38>be <00:40.65>alone
[00:41.25]<00:41.25>I-Island <00:42.20>breeze <00:43.03>and <00:43.23>lights <00:43.39>down <00:43.64>low
[00:44.33]<00:44.33>No <00:44.53>one <00:44.84>has <00:45.05>to <00:45.22>know
[00:47.33]<00:47.33>In <00:47.53>the <00:47.66>middle <00:47.86>of <00:48.03>the <00:48.17>night <00:49.28>in <00:49.51>my <00:49.79>dreams
[00:52.75]<00:52.75>You <00:52.96>should <00:53.16>see <00:53.32>the <00:53.48>things <00:53.80>we <00:54.15>do <00:55.30>baby
[00:59.22]<00:59.22>In <00:59.39>the <00:59.57>middle <00:59.85>of <01:00.06>the <01:00.22>night <01:01.23>in <01:01.45>my <01:01.67>dreams
[01:04.68]<01:04.68>I <01:04.89>know <01:05.09>I''m <01:05.26>gonna <01:05.49>be <01:05.82>with <01:06.15>you
[01:06.61]<01:06.61>So <01:07.04>I <01:07.32>take <01:08.01>my <01:08.78>time
[01:11.22]<01:11.22>Are <01:11.37>you <01:11.53>ready <01:11.80>for <01:12.00>it
[01:17.52]<01:17.52>Knew <01:17.79>I <01:17.98>was <01:18.11>a <01:18.26>robber <01:19.06>first <01:19.27>time <01:19.44>that <01:19.62>he <01:19.74>saw <01:19.92>me
[01:20.50]<01:20.50>Stealing <01:20.63>hearts <01:20.85>and <01:21.23>running <01:21.47>off <01:21.97>and <01:22.06>never <01:22.27>saying <01:22.68>sorry
[01:23.62]<01:23.62>But <01:23.82>if <01:24.01>I''m <01:24.21>a <01:24.35>thief <01:24.58>then <01:25.03>he <01:25.23>can <01:25.40>join <01:25.55>the <01:25.77>heist
[01:26.70]<01:26.70>And <01:26.89>we''ll <01:27.05>move <01:27.22>to <01:27.41>an <01:27.57>island and
[01:29.01]<01:29.01>And <01:29.25>he <01:29.48>can <01:29.68>be <01:29.89>my <01:30.02>jailer <01:31.13>Burton <01:31.36>to <01:31.52>this <01:31.64>Taylor
[01:32.70]<01:32.70>Every <01:32.90>love I''ve <01:33.20>known <01:33.41>in <01:33.63>comparison <01:34.23>is <01:34.47>a <01:34.67>failure
[01:35.62]<01:35.62>I <01:35.82>forget <01:36.00>their <01:36.16>names <01:36.47>now <01:37.02>I''m <01:37.23>so <01:37.41>very <01:37.61>tame <01:37.97>now
[01:38.54]<01:38.54>Never <01:38.75>be <01:38.93>the <01:39.16>same <01:39.33>now <01:40.28>now
[01:41.07]<01:41.07>I-I-I <01:42.24>see <01:42.88>how <01:43.06>this <01:43.23>is <01:43.38>gon'' <01:43.70>go
[01:45.05]<01:45.05>Touch <01:45.28>me <01:45.69>and <01:45.89>you''ll <01:46.07>never <01:46.28>be <01:46.52>alone
[01:47.21]<01:47.21>I-Island <01:48.19>breeze <01:48.77>and <01:49.00>lights <01:49.30>down <01:49.69>low
[01:50.41]<01:50.41>No <01:50.67>one <01:50.86>has <01:51.06>to <01:51.26>know
[01:51.64]<01:51.64>No <01:51.84>one <01:52.03>has <01:52.18>to <01:52.33>know
[01:53.23]<01:53.23>In <01:53.42>the <01:53.55>middle <01:53.78>of <01:54.13>the <01:54.22>night <01:55.25>in <01:55.47>my <01:55.65>dreams
[01:58.76]<01:58.76>You <01:58.96>should <01:59.14>see <01:59.32>the <01:59.49>things <01:59.83>we <02:00.14>do <02:01.31>baby
[02:05.20]<02:05.20>In <02:05.39>the <02:05.55>middle <02:05.83>of <02:06.04>the <02:06.22>night <02:07.22>in <02:07.54>my <02:07.75>dreams
[02:10.70]<02:10.70>I <02:10.92>know <02:11.09>I''m <02:11.25>gonna <02:11.47>be <02:11.78>with <02:12.18>you
[02:12.56]<02:12.56>So <02:12.92>I <02:13.14>take <02:14.31>my <02:14.75>time
[02:17.24]<02:17.24>Are <02:17.42>you <02:17.56>ready <02:17.85>for <02:18.06>it
[02:20.67]<02:20.67>Ooh <02:23.22>are <02:23.43>you <02:23.60>ready <02:23.79>for <02:23.96>it
[02:24.57]<02:24.57>Baby <02:24.91>let <02:25.12>the <02:25.28>games <02:25.57>begin
[02:26.43]<02:26.43>Let <02:26.63>the <02:26.79>games <02:27.07>begin
[02:27.88]<02:27.88>Let <02:28.09>the <02:28.24>games <02:28.51>begin
[02:30.45]<02:30.45>Baby <02:30.77>let <02:30.99>the <02:31.17>games <02:31.53>begin
[02:32.45]<02:32.45>Let <02:32.65>the <02:32.81>games <02:33.03>begin
[02:33.90]<02:33.90>Let <02:34.11>the <02:34.32>games <02:34.53>begin
[02:35.29]<02:35.29>I-I-I <02:36.17>see <02:37.01>how <02:37.23>this <02:37.39>is <02:37.53>gon'' <02:37.70>go
[02:38.92]<02:38.92>Touch <02:39.18>me <02:39.55>and <02:39.76>you''ll <02:39.94>never <02:40.17>be <02:40.35>alone
[02:41.03]<02:41.03>I-Island <02:41.97>breeze <02:42.88>and <02:43.08>lights <02:43.26>down <02:43.61>low
[02:44.47]<02:44.47>No <02:44.69>one <02:44.90>has <02:45.10>to <02:45.28>know
[02:48.68]<02:48.68>In <02:48.85>the <02:49.03>middle <02:49.30>of <02:49.51>the <02:49.68>night <02:50.90>in <02:51.15>my <02:51.32>dreams
[02:54.34]<02:54.34>You <02:54.54>should <02:54.72>see <02:54.88>the <02:55.06>things <02:55.34>we <02:55.52>do <02:57.05>baby
[03:00.91]<03:00.91>In <03:01.06>the <03:01.22>middle <03:01.36>of <03:01.51>the <03:01.67>night <03:02.88>in <03:03.09>my <03:03.24>dreams
[03:06.24]<03:06.24>I <03:06.47>know <03:06.64>I''m <03:06.80>gonna <03:07.02>be <03:07.31>with <03:07.64>you
[03:08.02]<03:08.02>So <03:08.35>I <03:08.74>take <03:09.69>my <03:10.32>time
[03:12.82]<03:12.82>In <03:13.02>the <03:13.16>middle <03:13.32>of <03:13.50>the <03:13.56>night
[03:14.15]<03:14.15>Baby <03:14.37>let <03:14.54>the <03:14.74>games <03:14.96>begin
[03:15.96]<03:15.96>Let <03:16.16>the <03:16.33>games <03:16.44>begin
[03:17.43]<03:17.43>Let <03:17.65>the <03:17.81>games <03:18.02>begin
[03:18.83]<03:18.83>Are <03:19.02>you <03:19.18>ready <03:19.35>for <03:19.59>it
[03:19.90]<03:19.90>Baby <03:20.14>let <03:20.34>the <03:20.63>games <03:20.92>begin
[03:21.95]<03:21.95>Let <03:22.19>the <03:22.36>games <03:22.56>begin
[03:23.37]<03:23.37>Let <03:23.56>the <03:23.76>games <03:23.92>begin
[03:24.75]<03:24.75>Are <03:24.94>you <03:25.12>ready <03:25.30>for <03:25.43>it
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (11634, 'lrc', 'line', 'local_lrc', '[00:50.23]<00:50.23>Permite<00:50.52> <00:50.56>que<00:51.17> <00:51.43>te<00:51.54> <00:51.59>invite<00:52.08> <00:52.51>a<00:52.54> <00:52.59>la<00:52.64> <00:52.65>despedida
[00:56.13]<00:56.13>No<00:56.20> <00:56.25>importa<00:56.55> <00:56.59>que<00:56.96> <00:57.18>no<00:57.34> <00:57.42>merezca<00:58.16> <00:58.36>más<00:58.62> <00:58.82>tu<00:58.98> <00:59.05>atención
[01:01.78]<01:01.78>Así<01:01.90> <01:01.95>se<01:02.22> <01:02.27>hacen<01:03.02> <01:03.14>las<01:03.43> <01:03.50>cosas<01:03.84> <01:03.99>en<01:04.19> <01:04.34>mi<01:04.86> <01:04.91>familia
[01:07.95]<01:07.95>Así<01:08.39> <01:08.58>me<01:08.89> <01:09.03>enseñaron<01:09.72> <01:09.98>a<01:10.13> <01:10.21>que<01:10.32> <01:10.39>las<01:10.68> <01:11.35>hiciera<01:12.30> <01:12.67>yo
[01:15.09]<01:15.09>Permite<01:15.50> <01:15.78>que<01:16.12> <01:16.22>te<01:16.43> <01:16.51>dedique<01:17.06> <01:17.17>la<01:17.66> <01:17.72>última<01:18.58> <01:18.63>línea
[01:21.14]<01:21.14>No<01:21.26> <01:21.42>importa<01:21.64> <01:21.66>que<01:22.08> <01:22.22>te<01:22.57> <01:22.67>disguste<01:23.50> <01:23.56>esta<01:24.03> <01:24.46>canción
[01:26.99]<01:26.99>Así<01:27.12> <01:27.18>mi<01:27.31> <01:27.36>conciencia<01:28.65> <01:28.83>quedará<01:29.34> <01:29.40>más<01:29.89> <01:29.97>tranquila
[01:33.17]<01:33.17>Así,<01:33.34> <01:33.39>en<01:33.45> <01:33.50>esta<01:33.72> <01:33.77>banda,<01:34.68> <01:35.01>decimos<01:35.95> <01:36.19>adiós
[01:39.58]<01:39.58>Y<01:39.74> <01:39.90>al<01:40.20> <01:40.26>final
[01:42.68]<01:42.68>Te<01:42.74> <01:42.76>ataré<01:42.98> <01:43.00>con<01:43.11> <01:43.18>todas<01:44.27> <01:44.38>mis<01:45.05> <01:45.11>fuerzas
[01:47.70]<01:47.70>Mis<01:47.83> <01:47.92>brazos<01:48.37> <01:48.57>serán<01:49.18> <01:49.25>cuerdas
[01:50.63]<01:50.63>Al<01:50.66> <01:50.66>bailar<01:50.99> <01:51.10>este<01:51.61> <01:51.66>vals
[01:54.17]<01:54.17>Y<01:54.34> <01:54.50>al<01:54.64> <01:54.73>final
[01:57.06]<01:57.06>Quiero<01:57.15> <01:57.18>verte<01:57.32> <01:57.34>de<01:59.27> <01:59.40>nuevo<01:59.69> <02:00.40>contenta
[02:02.56]<02:02.56>Sigue<02:02.93> <02:03.01>dando<02:03.41> <02:03.52>vueltas
[02:05.21]<02:05.21>Si<02:05.36> <02:05.39>aguantas<02:06.34> <02:06.63>de<02:06.95> <02:07.11>pie
[02:21.14]<02:21.14>Permite<02:22.33> <02:22.66>que<02:22.95> <02:23.01>te<02:23.25> <02:23.30>explique<02:24.00> <02:24.05>que<02:24.40> <02:24.62>no<02:24.74> <02:25.20>tengo<02:25.49> <02:25.50>prisa
[02:27.52]<02:27.52>No<02:27.59> <02:27.62>importa<02:27.95> <02:28.07>que<02:28.52> <02:28.72>tengas<02:29.46> <02:29.51>algo<02:29.96> <02:30.06>mejor<02:30.73> <02:30.92>que<02:31.15> <02:31.20>hacer
[02:33.37]<02:33.37>Así<02:33.55> <02:33.59>nos<02:33.72> <02:33.77>podemos<02:34.44> <02:34.52>pegar<02:34.98> <02:35.63>toda<02:36.77> <02:36.90>la<02:37.75> <02:37.79>vida
[02:39.21]<02:39.21>Así,<02:39.53> <02:39.63>si<02:39.78> <02:39.92>me<02:40.07> <02:40.19>dejas,<02:40.99> <02:41.05>no<02:41.15> <02:41.27>te<02:41.74> <02:41.88>dejaré<02:42.75> <02:42.95>de<02:43.33> <02:43.39>querer
[02:45.95]<02:45.95>Y<02:46.10> <02:46.25>al<02:46.73> <02:46.85>final
[02:48.68]<02:48.68>Te<02:48.76> <02:48.80>ataré<02:49.00> <02:49.05>con<02:49.37> <02:49.44>todas<02:51.40> <02:51.81>mis<02:52.51> <02:52.58>fuerzas
[02:54.03]<02:54.03>Mis<02:54.16> <02:54.21>brazos<02:54.93> <02:54.95>serán<02:55.20> <02:55.54>cuerdas
[02:56.99]<02:56.99>Al<02:57.07> <02:57.15>bailar<02:57.48> <02:57.53>este<02:57.78> <02:57.86>vals
[03:00.65]<03:00.65>Y<03:00.74> <03:00.82>al<03:01.03> <03:01.52>final
[03:03.76]<03:03.76>Quiero<03:04.02> <03:04.14>verte<03:04.48> <03:04.54>de<03:05.24> <03:05.43>nuevo<03:06.47> <03:06.50>contenta
[03:08.71]<03:08.71>Sigue<03:09.10> <03:09.28>dando<03:09.94> <03:10.15>vueltas
[03:11.48]<03:11.48>Si<03:11.60> <03:11.69>aguantas<03:12.21> <03:12.93>de<03:13.00> <03:13.01>pie
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (12867, 'lrc', 'line', 'local_lrc', '[01:59.80]Sifting through weathered photo albums
[02:10.95](Does it make a difference) Looking for
[02:12.82](This is the way it is) gloriously aged polaroids
[02:19.47](You think it really would make a difference) Of
[02:22.91]Places (Would I hang on the)
[02:24.24]You''ve never been (Beach in perfect black and hide)
[02:28.52](I broke through this hollow shell that once)
[02:34.13](Held me so tight I couldn''t breathe)
[02:38.32](Come) A place (With me) to accept
[02:43.05](Jump off the edge) You don''t exist
[02:47.88]Smile for the camera sweetheart.
[02:51.60]I really wanna immortalize the moment
[02:52.28]Just remember the first step in forgetting
[02:54.98]Is destroying all the evidence.
[02:57.96]With friends
[03:00.54]Like you
[03:03.40]Who needs
[03:06.05]Subtext
[03:08.41]Sub. Text.
[03:13.39]Sub. Text.
[03:18.15]This is a .44 caliber love letter straight from my heart.
[03:21.92]With a gun, make your shot.
[03:26.22]Let''s hope for better shit.
[03:30.64](Straight) That (from my) reason (heart) for separation
[03:39.49](Straight) Straight from (from) my (my) my (heart) heart
[03:49.04]Christened by your bullet
[03:56.28]I''m losing patience
[04:02.54]Well I guess
[04:05.24]It''s my own fault
[04:13.09]Don''t re- don''t re-
[04:17.19]Don''t re-mem-ber
[04:20.76]Don''t re-mem-ber
[04:26.84]Don''t
[04:27.71]
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (957, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>作<00:00.12>词<00:00.24> <00:00.36>:<00:00.48> <00:00.60>J<00:00.72>a<00:00.84>c<00:00.96>k<00:01.08> <00:01.20>U<00:01.32>n<00:01.44>d<00:01.56>e<00:01.68>r<00:01.80>k<00:01.92>o<00:02.04>f<00:02.16>l<00:02.28>e<00:02.40>r<00:02.52>/<00:02.64>J<00:02.76>a<00:02.88>c<00:03.00>k<00:03.12> <00:03.24>C<00:03.36>o<00:03.48>l<00:03.60>l<00:03.72>i<00:03.84>n<00:03.96>s<00:04.08>/<00:04.20>N<00:04.32>i<00:04.44>c<00:04.56>h<00:04.68>o<00:04.80>l<00:04.92>a<00:05.04>s<00:05.16> <00:05.28>T<00:05.40>a<00:05.52>y<00:05.64>l<00:05.76>o<00:05.88>r<00:06.00>/<00:06.12>W<00:06.24>i<00:06.36>l<00:06.48>l<00:06.60>a<00:06.72>r<00:06.84>d<00:06.96> <00:07.08>G<00:07.20>o<00:07.32>o<00:07.44>d<00:07.56>r<00:07.68>o<00:07.80>a<00:07.92>d
[00:08.04]<00:08.04>作<00:08.16>曲<00:08.28> <00:08.40>:<00:08.52> <00:08.64>J<00:08.76>a<00:08.88>c<00:09.00>k<00:09.12> <00:09.24>U<00:09.36>n<00:09.48>d<00:09.60>e<00:09.72>r<00:09.84>k<00:09.96>o<00:10.08>f<00:10.20>l<00:10.32>e<00:10.44>r<00:10.56>/<00:10.68>J<00:10.80>a<00:10.92>c<00:11.04>k<00:11.16> <00:11.28>C<00:11.40>o<00:11.52>l<00:11.64>l<00:11.76>i<00:11.88>n<00:12.00>s<00:12.12>/<00:12.24>N<00:12.36>i<00:12.48>c<00:12.60>h<00:12.72>o<00:12.84>l<00:12.96>a<00:13.08>s<00:13.20> <00:13.32>T<00:13.44>a<00:13.56>y<00:13.68>l<00:13.80>o<00:13.92>r<00:14.04>/<00:14.16>W<00:14.28>i<00:14.40>l<00:14.52>l<00:14.64>a<00:14.76>r<00:14.88>d<00:15.00> <00:15.12>G<00:15.24>o<00:15.36>o<00:15.48>d<00:15.60>r<00:15.72>o<00:15.84>a<00:15.96>d
[00:16.08]<00:16.08>Right<00:16.38> <00:16.38>babe<00:16.93> <00:16.93>you<00:17.08> <00:17.08>listen<00:17.98>,<00:17.98> <00:17.98>I''m<00:18.10> <00:18.10>done<00:18.47>,<00:18.47> <00:18.47>yeah<00:18.86>,<00:18.86> <00:18.87>I''m<00:19.09> <00:19.09>leaving
[00:24.03]<00:24.03>I''m<00:24.30> <00:24.30>getting<00:25.01> <00:25.01>weak<00:25.29>,<00:25.29> <00:25.29>feeling<00:26.00> <00:26.00>tweaked<00:26.46> <00:26.46>out<00:26.87> <00:26.87>and<00:27.02> <00:27.02>faded
[00:31.96]<00:31.96>God<00:32.42> <00:32.42>help<00:32.89> <00:32.89>you<00:33.03>,<00:33.03> <00:33.03>darling<00:33.81>,<00:33.81> <00:33.81>''cause<00:34.03> <00:34.03>my<00:34.28> <00:34.28>love<00:34.88> <00:34.88>is<00:35.04> <00:35.04>drying<00:35.66> <00:35.66>up
[00:39.95]<00:39.95>You<00:40.25> <00:40.25>feel<00:40.75> <00:40.75>so<00:41.06> <00:41.06>lovely<00:42.12> <00:42.32>when<00:42.90> <00:42.90>you<00:43.00> <00:43.00>touch<00:43.45> <00:43.45>me
[00:43.86]<00:43.86>But<00:44.12> <00:44.12>I<00:44.33> <00:44.33>can''t<00:44.73> <00:44.73>do<00:45.07> <00:45.07>this<00:45.43> <00:45.43>anymore
[00:47.79]<00:47.79>Your<00:48.03> <00:48.03>nails<00:48.48> <00:48.48>on<00:48.85> <00:48.85>my<00:49.02> <00:49.02>back<00:49.49> <00:49.50>feel<00:49.83> <00:49.83>that<00:50.19> <00:50.19>summer<00:50.93> <00:50.93>sadness
[00:52.00]<00:52.00>Baby<00:52.86> <00:52.86>it''s<00:53.11> <00:53.11>too<00:53.36> <00:53.36>late<00:53.84> <00:53.84>to<00:54.04> <00:54.04>talk
[00:56.02]<00:56.02>I<00:56.25> <00:56.25>feel<00:57.04> <00:57.04>like<00:57.42> <00:57.42>we''re<00:57.80> <00:57.96>drifting<00:59.08> <00:59.08>apart
[01:00.49]<01:00.49>Talk<01:00.94> <01:00.94>shit<01:01.51>,<01:01.51> <01:01.51>babe
[01:01.97]<01:01.97>Say<01:02.43> <01:02.43>it<01:02.52> <01:02.53>like<01:02.89> <01:02.89>you<01:03.09> <01:03.09>wanna<01:03.57> <01:03.57>leave
[01:04.80]<01:04.80>You<01:05.02> <01:05.02>love<01:05.34> <01:05.34>me<01:05.51> <01:05.51>like<01:06.11> <01:06.46>***<01:06.50>,<01:06.50> <01:06.50>yeah
[01:08.59]<01:08.59>Don''t<01:09.03> <01:09.03>lie<01:09.40>,<01:09.40> <01:09.40>get<01:09.83> <01:09.83>it<01:10.03> <01:10.04>right
[01:10.48]<01:10.48>Need<01:10.84> <01:10.84>me<01:11.09> <01:11.09>every<01:11.50> <01:11.50>night
[01:12.78]<01:12.78>You<01:13.08> <01:13.08>love<01:13.37> <01:13.37>me<01:13.53> <01:13.53>like<01:14.15> <01:16.54>***
[01:24.03]<01:24.03>I<01:24.26> <01:24.26>heard<01:24.86> <01:24.86>your<01:24.99> <01:24.99>friends<01:25.33> <01:25.33>say<01:25.87> <01:25.87>that<01:26.07> <01:26.07>you<01:26.34> <01:26.34>think<01:26.94> <01:26.94>you''re<01:27.06> <01:27.06>better<01:27.48> <01:27.49>off
[01:32.05]<01:32.05>Don''t<01:32.37> <01:32.37>pretend<01:32.91> <01:32.91>to<01:33.02> <01:33.02>love<01:33.34> <01:33.34>me<01:33.57> <01:33.57>when<01:33.87> <01:33.87>you''re<01:34.13> <01:34.13>feeling<01:34.98> <01:34.98>alone
[01:39.85]<01:39.85>I<01:39.99> <01:39.99>need<01:40.36> <01:40.36>to<01:40.54> <01:40.54>know<01:40.85> <01:40.85>the<01:41.02> <01:41.02>truth<01:41.54>,<01:41.54> <01:41.54>''cause<01:41.91> <01:41.91>you''re<01:42.01> <01:42.01>talking<01:42.60> <01:42.60>like<01:42.91> <01:42.91>you''re<01:43.01> <01:43.01>fed<01:43.43> <01:43.44>up
[01:51.86]<01:51.86>I<01:52.24> <01:52.24>feel<01:53.04> <01:53.05>like<01:53.46> <01:53.46>we''re<01:54.00> <01:54.02>drifting<01:55.13> <01:55.13>apart
[01:56.56]<01:56.56>Talk<01:56.96> <01:56.96>shit<01:57.49>,<01:57.49> <01:57.50>babe
[01:57.97]<01:57.97>Say<01:58.44> <01:58.44>it<01:58.53> <01:58.53>like<01:58.91> <01:58.91>you<01:59.08> <01:59.08>wanna<01:59.56> <01:59.56>leave
[02:00.78]<02:00.78>You<02:01.06> <02:01.06>love<02:01.36> <02:01.36>me<02:01.56> <02:01.56>like<02:02.11> <02:02.48>***<02:02.49>,<02:02.49> <02:02.49>yeah
[02:04.58]<02:04.58>Don''t<02:05.07> <02:05.07>lie<02:05.42>,<02:05.42> <02:05.42>get<02:05.81> <02:05.81>it<02:05.98> <02:06.04>right
[02:06.46]<02:06.46>Need<02:06.89> <02:06.89>me<02:07.16> <02:07.16>every<02:07.59> <02:07.59>night
[02:08.80]<02:08.80>You<02:09.05> <02:09.05>love<02:09.36> <02:09.36>me<02:09.53> <02:09.53>like<02:10.33>,<02:10.33> <02:10.80>you<02:11.07> <02:11.07>love<02:11.35> <02:11.35>me<02:11.52> <02:11.52>like
[02:16.90]<02:16.90>You<02:17.07> <02:17.07>love<02:17.39> <02:17.39>me<02:17.55> <02:17.55>like<02:18.13> <02:18.75>***<02:19.54>,<02:19.54> <02:19.54>yeah
[02:24.89]<02:24.89>You<02:25.09> <02:25.09>love<02:25.43> <02:25.43>me<02:25.58> <02:25.58>like<02:26.36>,<02:26.36> <02:26.82>you<02:27.06> <02:27.06>love<02:27.35> <02:27.35>me<02:27.51> <02:27.51>like
[02:32.10]<02:32.10>I<02:32.62> <02:32.83>don''t<02:33.63> <02:33.63>want<02:34.26> <02:34.27>you<02:34.92> <02:35.07>darling<02:36.13>,<02:36.13> <02:36.13>I<02:36.54> <02:36.82>don''t<02:37.61> <02:37.61>want<02:38.03> <02:38.33>you<02:38.84> <02:39.06>darling
[02:40.12]<02:40.12>I<02:40.61> <02:40.82>don''t<02:41.61> <02:41.61>want<02:42.24> <02:42.36>you<02:42.80> <02:43.08>darling
[02:44.00]<02:44.00>You<02:44.81> <02:44.81>don''t<02:45.60> <02:45.60>love<02:46.25> <02:46.26>me<02:46.79> <02:47.05>darling<02:48.06>,<02:48.06> <02:48.06>you<02:48.57> <02:48.80>don''t<02:49.57> <02:49.57>love<02:50.34> <02:50.35>me<02:50.71> <02:51.03>darling
[02:52.08]<02:52.08>You<02:52.62> <02:52.79>don''t<02:53.56> <02:53.56>love<02:54.18> <02:54.35>me<02:54.85> <02:55.00>darling
[02:56.10]<02:56.10>I<02:56.89> <02:56.89>don''t<02:57.58> <02:57.58>love<02:58.42> <02:58.42>you<02:59.01> <02:59.01>darling<02:59.95>,<02:59.95> <03:00.09>I<03:00.92> <03:00.92>don''t<03:01.60> <03:01.60>love<03:02.39> <03:02.39>you<03:03.01> <03:03.01>darling
[03:04.06]<03:04.06>I<03:04.91> <03:04.91>don''t<03:05.57> <03:05.57>love<03:06.42> <03:06.42>you<03:07.06> <03:07.06>darling
[03:08.45]<03:08.45>Talk<03:08.90> <03:08.90>shit<03:09.49>,<03:09.49> <03:09.50>***
[03:09.86]<03:09.86>Say<03:10.40> <03:10.40>it<03:10.50> <03:10.50>like<03:10.92> <03:10.92>you<03:11.07> <03:11.07>wanna<03:11.51> <03:11.51>leave
[03:12.84]<03:12.84>You<03:13.02> <03:13.02>love<03:13.36> <03:13.36>me<03:13.56> <03:13.56>like<03:14.13> <03:15.35>***<03:15.57>,<03:15.57> <03:15.57>yeah
[03:16.55]<03:16.55>Don''t<03:17.01> <03:17.01>lie<03:17.45>,<03:17.45> <03:17.45>get<03:17.80> <03:17.80>it<03:18.04> <03:18.05>right
[03:18.46]<03:18.46>Need<03:18.87> <03:18.87>me<03:19.11> <03:19.11>every<03:19.52> <03:19.52>night
[03:20.82]<03:20.82>You<03:21.12> <03:21.12>love<03:21.41> <03:21.41>me<03:21.57> <03:21.57>like<03:22.32>,<03:22.32> <03:22.87>you<03:23.10> <03:23.10>love<03:23.41> <03:23.41>me<03:23.59> <03:23.59>like
[03:28.76]<03:28.76>You<03:29.04> <03:29.04>love<03:29.33> <03:29.33>me<03:29.49> <03:29.49>like<03:30.10> <03:30.73>***<03:31.53>,<03:31.53> <03:31.53>yeah
[03:36.85]<03:36.85>You<03:37.06> <03:37.06>love<03:37.44> <03:37.44>me<03:37.60> <03:37.60>like<03:38.33>,<03:38.33> <03:38.86>I<03:39.09> <03:39.09>love<03:39.43> <03:39.43>you<03:39.62> <03:39.62>like
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (22679, 'lrc', 'line', 'local_lrc', '[00:09.47]O-o-seven, o-o-seven
[00:18.78]At ocean eleven
[00:23.36]And now rudeboys a go wail
[00:28.17]''Cause them out of jail
[00:32.31]Rudeboys cannot fail
[00:37.38]''Cause them must get bail
[00:41.04]Oh-oh-oh, dem a loot, dem a shoot, dem a wail
[00:46.01]A shanty town
[00:48.87]Dem a loot, dem a shoot, dem a wail
[00:50.96]A shanty town
[00:53.93]Dem rudeboys get a probation
[00:55.90]A shanty town
[00:58.75]And rudeboy a bomb up the town
[01:00.52]A shanty town
[01:20.47]O-o-seven, o-o-seven
[01:29.71]At ocean eleven
[01:34.16]And the rudeboys a go wail
[01:38.78]''Cause them out of jail
[01:43.59]Rudeboys cannot fail
[01:47.88]''Cause them must get bail
[01:51.81]Oh-oh-oh, dem a loot, dem a shoot, dem a wail
[01:57.13]A shanty town
[01:59.41]Dem a loot, dem a shoot, dem a wail
[02:01.22]A shanty town
[02:04.18]Dem rudeboys get a probation
[02:06.11]A shanty town
[02:08.86]And rudeboy a bomb up the town
[02:10.75]A shanty town
[02:13.46]Police get taller
[02:15.76]A shanty town
[02:18.49]Soldier get longer
[02:20.53]A shanty town
[02:22.73]Rudeboy a weep and a wail
[02:25.03]A shanty town
[02:27.59]Rudeboy a weep and a wail
[02:30.03]A shanty town
[02:31.91]
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (11005, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>1<00:00.09> <00:00.18>of<00:00.27> <00:00.36>1<00:00.45> <00:00.54>-<00:00.63> <00:00.72>SHINee<00:00.81> <00:00.90>(<00:00.99>샤<00:01.08>이<00:01.17>니<00:01.26>)
[00:01.35]<00:01.35>词<00:01.41>：<00:01.47>JQ<00:01.52>/<00:01.58>조<00:01.64>미<00:01.70>양<00:01.76>/<00:01.81>배<00:01.87>성<00:01.93>현<00:01.99>/<00:02.05>이<00:02.10>스<00:02.16>란<00:02.22>/<00:02.28>김<00:02.34>인<00:02.39>형<00:02.45>/<00:02.51>박<00:02.57>성<00:02.63>희
[00:02.70]<00:02.70>曲<00:02.77>：<00:02.84>Mike<00:02.91> <00:02.98>Daley<00:03.06>/<00:03.13>Mitchell<00:03.20> <00:03.27>Owens<00:03.34>/<00:03.41>Michael<00:03.48> <00:03.55>Jiminez<00:03.62>/<00:03.69>Tay<00:03.77> <00:03.84>Jasper<00:03.91>/<00:03.98>MZMC
[00:04.05]<00:04.05>编<00:04.19>曲<00:04.32>：<00:04.46>Mike<00:04.59> <00:04.73>Daley<00:04.87>/<00:05.00>Mitchell<00:05.14> <00:05.27>Owens
[00:05.42]<00:05.42>하루 <00:05.80>중에 <00:07.16>1분 <00:07.56>1초 <00:08.11>다르듯
[00:09.26]<00:09.26>날마다 <00:09.78>넌 <00:10.41>새로워져
[00:14.34]<00:14.34>하나 <00:14.80>중에 <00:15.84>그중에 <00:16.43>제일 <00:17.04>첫 <00:17.24>번째
[00:18.11]<00:18.11>유일하단 <00:19.29>뜻인 <00:19.81>거야 <00:21.48>너
[00:22.07]<00:22.07>나를 <00:22.57>부르는 <00:24.55>네 <00:24.80>목소리
[00:26.76]<00:26.76>사뿐하게 <00:27.82>다가와서
[00:28.91]<00:28.91>내 <00:29.17>귓가에 <00:29.93>미끄러져
[00:30.79]<00:30.79>손에 <00:31.50>감겨오는 <00:33.52>네 <00:33.76>손길이
[00:35.64]<00:35.64>처음 <00:35.93>만나는 <00:36.62>저 <00:36.95>눈부신
[00:38.03]<00:38.03>세상으로 <00:38.93>나를 <00:39.18>이끌어
[00:40.81]<00:40.81>넌 <00:41.01>1 <00:41.42>of <00:41.98>1 <00:42.46>girl
[00:45.30]<00:45.30>오직 <00:45.87>하나
[00:47.05]<00:47.05>틀림없이 <00:48.03>나의 <00:48.55>답인 <00:49.11>너
[00:49.76]<00:49.76>넌 <00:49.94>1 <00:50.34>of <00:50.86>1 <00:51.34>girl
[00:53.14]<00:53.14>완벽해
[00:53.74]<00:53.74>비교할 <00:54.35>수 <00:54.70>없는 <00:55.22>넌 <00:55.58>이미
[00:56.00]<00:56.00>내 <00:56.28>세상의 <00:56.86>유일한 <00:57.58>의미
[00:58.63]<00:58.63>하나의 <00:59.21>이름 <01:00.34>너라는 <01:00.88>사람에
[01:02.87]<01:02.87>꼭 <01:03.16>들어맞는 <01:04.72>컬러를 <01:05.34>입힌 <01:06.14>듯 <01:06.84>yeah
[01:07.49]<01:07.49>귓가에 <01:08.16>스친 <01:09.14>달콤한 <01:10.10>노래처럼
[01:11.82]<01:11.82>완벽하게 <01:12.79>어울려
[01:13.71]<01:13.71>자꾸 <01:14.19>너를 <01:14.77>부르게 <01:15.49>돼
[01:15.88]<01:15.88>처음의 <01:16.11>그 <01:16.31>느낌처럼
[01:16.90]<01:16.90>언제나 <01:17.42>replay <01:18.05>replay
[01:18.45]<01:18.45>네 <01:18.82>사랑은 <01:19.18>새롭게 <01:19.71>빛나
[01:20.15]<01:20.15>난 <01:20.51>또다시 <01:20.85>fallin <01:21.40>fallin
[01:21.62]<01:21.62>For <01:21.81>you <01:21.98>come <01:22.14>here
[01:22.33]<01:22.33>사랑한다는 <01:23.04>말도
[01:23.36]<01:23.36>네겐 <01:23.77>지겹지 <01:24.15>않아
[01:24.83]<01:24.83>달콤한 <01:25.37>입맞춤
[01:26.15]<01:26.15>서로에게만 <01:26.94>맞춘 <01:27.46>발걸음
[01:28.45]<01:28.45>아무 <01:28.71>예고 <01:29.03>없이 <01:29.32>두눈이 <01:29.68>마주친
[01:30.41]<01:30.41>너는 <01:30.90>왜 <01:31.16>이리 <01:31.50>아름다운지
[01:33.01]<01:33.01>You''re <01:33.17>my <01:33.31>baby
[01:33.85]<01:33.85>넌 <01:34.13>1 <01:34.62>of <01:35.20>1 <01:35.61>girl
[01:38.55]<01:38.55>오직 <01:39.19>하나
[01:40.36]<01:40.36>틀림없이 <01:41.44>나의 <01:41.94>답인 <01:42.48>너
[01:43.01]<01:43.01>넌 <01:43.23>1 <01:43.72>of <01:44.21>1 <01:44.67>girl
[01:46.35]<01:46.35>완벽해
[01:47.03]<01:47.03>비교할 <01:47.68>수 <01:48.01>없는 <01:48.52>넌 <01:48.84>이미
[01:49.17]<01:49.17>내 <01:49.49>세상의 <01:50.22>유일한 <01:50.82>의미
[01:52.07]<01:52.07>1 <01:52.55>of <01:53.04>1 <01:53.56>girl
[01:56.41]<01:56.41>오직 <01:57.00>하나
[01:58.08]<01:58.08>빈틈없이 <01:59.13>나를 <01:59.66>채우지
[02:00.80]<02:00.80>넌 <02:00.97>1 <02:01.41>of <02:01.93>1 <02:02.40>girl
[02:04.44]<02:04.44>완벽해
[02:04.82]<02:04.82>대신할 <02:05.50>수 <02:05.78>없는 <02:06.37>넌 <02:06.59>오직
[02:07.08]<02:07.08>내 <02:07.26>세상의 <02:07.97>유일한 <02:08.62>의미
[02:09.23]<02:09.23>이토록 <02:10.06>깊이 <02:10.68>너에게
[02:11.74]<02:11.74>스며 <02:12.92>변하고 <02:13.98>있어
[02:18.24]<02:18.24>널 <02:18.60>닮은 <02:18.95>빛으로 <02:19.83>밝힌
[02:20.57]<02:20.57>맘이 <02:21.80>가득 <02:22.36>차오를 <02:23.32>때
[02:24.93]<02:24.93>내가 <02:25.35>너를 <02:25.89>비춰 <02:26.43>줄게
[02:27.18]<02:27.18>넌 <02:27.56>1 <02:28.06>of <02:28.62>1 <02:29.05>girl
[02:31.91]<02:31.91>오직 <02:32.51>하나
[02:33.72]<02:33.72>틀림없이 <02:34.67>나의 <02:35.20>답인 <02:35.68>너
[02:36.36]<02:36.36>넌 <02:36.52>1 <02:36.99>of <02:37.51>1 <02:38.04>girl
[02:39.64]<02:39.64>완벽해
[02:40.36]<02:40.36>비교할 <02:41.07>수 <02:41.33>없는 <02:41.79>넌 <02:41.97>이미
[02:42.53]<02:42.53>내 <02:42.80>세상의 <02:43.56>유일한 <02:44.13>의미
[02:45.43]<02:45.43>1 <02:45.92>of <02:46.38>1 <02:46.85>girl
[02:49.71]<02:49.71>오직 <02:50.26>하나
[02:51.48]<02:51.48>빈틈없이 <02:52.43>나를 <02:52.98>채우지
[02:54.06]<02:54.06>넌 <02:54.24>1 <02:54.74>of <02:55.26>1 <02:55.71>girl
[02:57.43]<02:57.43>완벽해
[02:58.02]<02:58.02>대신할 <02:58.77>수 <02:59.13>없는 <02:59.57>넌 <02:59.74>오직
[03:00.36]<03:00.36>내 <03:00.65>세상의 <03:01.35>유일한 <03:01.97>의미
[03:02.70]<03:02.70>넌 <03:03.02>1 <03:03.55>of <03:04.06>1 <03:04.48>girl
[03:07.48]<03:07.48>넌 <03:07.66>1 <03:08.07>of <03:08.56>1 <03:09.01>girl
[03:09.62]<03:09.62>틀림없이 <03:10.24>나의 <03:10.80>답인 <03:11.28>너
[03:11.74]<03:11.74>넌 <03:12.03>1 <03:12.51>of <03:13.02>1 <03:13.46>girl
[03:13.81]<03:13.81>1 <03:14.11>of <03:14.32>1 <03:14.58>girl <03:14.86>1 <03:15.05>of <03:15.35>1
[03:15.88]<03:15.88>비교할 <03:16.55>수 <03:16.88>없어 <03:17.52>누구도
[03:18.19]<03:18.19>One <03:18.38>and <03:18.59>only <03:19.11>너만을 <03:19.78>원해
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (7, 'lrc', 'line', 'local_lrc', '[00:16.38]Con un beso llego la calma
[00:19.95]Con un beso se fue el dolor
[00:23.09]De esos besos que ganan guerras a tu favor
[00:29.32]Unos besos salen del alma
[00:32.58]Y otros besos del corazón
[00:35.83]Y la magia es que yo en tu boca encontré los dos
[00:42.18]Pero un beso que no esperaba
[00:45.36]El que todo lo terminó
[00:48.53]Hizo mal cuando te callaba
[00:51.57]Y me robó una explicación
[00:56.90]
[00:59.37]Y hoy me hace falta tu voz
[01:03.15]Cuando llega el frío
[01:06.26]Vivo en el vacío que tú dejaste al decir adiós
[01:12.26]Y hoy me hace falta tu voz
[01:15.75]Me sobra tu olvido
[01:19.06]Y sentirme vivo
[01:21.02]Con otros besos le pido a Dios
[01:24.69]Unos besos salen del alma
[01:27.73]Y otros besos del corazón
[01:31.05]Y la magia es que yo en tu boca
[01:34.05]Con mi boca, vimos dos
[01:40.15]
[01:43.28]Con un beso llegó la calma
[01:47.13]Con un beso dijiste adiós
[01:52.53]
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (14863, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>100<00:00.07>%<00:00.13> <00:00.20>Endurance<00:00.26> <00:00.33>(<00:00.39>Explicit<00:00.46>)<00:00.52> <00:00.58>-<00:00.65> <00:00.71>Yard<00:00.78> <00:00.84>Act
[00:00.92]<00:00.92>Lyrics<00:00.98> <00:01.04>by<00:01.10>：<00:01.16>James<00:01.23> <00:01.29>Smith<00:01.35>/<00:01.41>Ryan<00:01.47> <00:01.53>Needham<00:01.59>/<00:01.65>Sam<00:01.71> <00:01.77>Shjipstone
[00:01.84]<00:01.84>Composed<00:01.90> <00:01.96>by<00:02.02>：<00:02.08>James<00:02.15> <00:02.21>Smith<00:02.27>/<00:02.33>Ryan<00:02.39> <00:02.45>Needham<00:02.51>/<00:02.57>Sam<00:02.63> <00:02.69>Shjipstone
[00:02.76]<00:02.76>I <00:02.94>was <00:03.18>woken <00:03.81>by <00:04.17>a <00:04.23>bang
[00:06.69]<00:06.69>And <00:06.81>I <00:06.84>could <00:07.08>already <00:07.59>taste <00:08.07>the <00:08.16>pain
[00:08.82]<00:08.82>The <00:08.88>sudden <00:09.30>fear <00:09.81>that <00:10.02>grips <00:10.44>and <00:10.56>shapes <00:11.04>you
[00:11.25]<00:11.25>When <00:11.37>you <00:11.49>face <00:11.91>the <00:12.03>truth
[00:12.96]<00:12.96>Whose <00:13.26>sofa <00:13.80>was <00:14.16>this
[00:14.94]<00:14.94>Where <00:15.33>were <00:15.45>my <00:15.66>shoes
[00:16.38]<00:16.38>What <00:16.65>did <00:16.86>we <00:17.19>do <00:17.58>last <00:17.97>night
[00:18.33]<00:18.33>I <00:18.51>don''t <00:18.78>remember <00:19.47>leaving <00:19.98>Nathan''s <00:20.76>house
[00:21.90]<00:21.90>Ah <00:22.14>yeah <00:22.53>how <00:22.80>could <00:23.01>I <00:23.19>forget
[00:24.00]<00:24.00>Why <00:24.30>my <00:24.45>pants <00:24.93>were <00:25.05>soaking <00:25.74>wet
[00:26.67]<00:26.67>When <00:26.85>we''d <00:27.09>been <00:27.30>pissing <00:27.75>ourselves <00:28.35>laughing <00:28.83>at <00:29.01>the <00:29.13>news
[00:30.78]<00:30.78>Did <00:30.87>you <00:30.93>see <00:31.23>it <00:31.32>too
[00:32.19]<00:32.19>It <00:32.34>was <00:32.46>incredible <00:33.27>they <00:33.42>played <00:33.81>it <00:33.90>on <00:34.05>a <00:34.14>loop <00:34.92>we <00:35.10>couldn''t <00:35.46>believe <00:36.00>it
[00:36.24]<00:36.24>Basically <00:36.81>they''d <00:37.08>discovered <00:37.71>that <00:37.86>there <00:38.13>were <00:38.31>others <00:38.73>just <00:39.06>like <00:39.45>us
[00:39.66]<00:39.66>Other <00:39.84>beings <00:40.35>other <00:40.53>creatures <00:41.58>other <00:41.82>planets <00:42.36>with <00:42.45>other <00:42.63>species
[00:43.50]<00:43.50>Who <00:43.56>had <00:43.68>other <00:43.92>gods <00:44.31>that <00:44.49>they <00:44.73>believed <00:45.33>in
[00:45.48]<00:45.48>And <00:45.60>they <00:46.11>interviewed <00:46.83>all <00:47.16>of <00:47.25>them <00:47.70>and <00:48.00>everyone <00:48.72>of <00:48.87>them
[00:49.17]<00:49.17>Not <00:49.35>one <00:49.71>could <00:49.95>give <00:50.22>any <00:50.49>hint <00:50.91>of <00:51.00>a <00:51.09>clue <00:51.60>what <00:51.84>they <00:52.08>were <00:52.26>doing <00:52.77>here <00:53.16>either
[00:55.74]<00:55.74>It''s <00:56.10>all <00:56.28>so <00:56.70>pointless
[00:58.02]<00:58.02>It <00:58.11>is <00:58.77>and <00:58.95>that''s <00:59.19>beautiful <00:59.94>l <01:00.06>find <01:00.51>it <01:00.66>humbling <01:01.32>sincerely
[01:02.73]<01:02.73>And <01:04.29>when <01:04.62>you''re <01:04.77>gone
[01:06.18]<01:06.18>It <01:06.45>brings <01:06.78>me <01:06.93>peace <01:07.35>of <01:07.44>mind <01:07.80>to <01:07.98>know <01:08.31>that <01:08.52>this <01:08.79>will <01:08.94>all <01:09.24>just <01:09.51>carry <01:09.96>on
[01:10.23]<01:10.23>With <01:10.41>someone <01:11.04>else
[01:11.70]<01:11.70>Someone <01:12.36>else
[01:12.51]<01:12.51>With <01:12.60>something <01:13.17>new
[01:13.65]<01:13.65>Something <01:14.61>new
[01:15.66]<01:15.66>No <01:16.05>need <01:16.56>to <01:16.83>be <01:17.10>blue
[01:21.63]<01:21.63>Everything <01:21.99>has <01:22.17>already <01:22.59>happened <01:23.19>time <01:23.46>is <01:23.73>an <01:23.97>illusion
[01:30.18]<01:30.18>It''s <01:30.54>hippy <01:31.08>bulls**t <01:31.68>but <01:32.01>it''s <01:32.28>true
[01:34.41]<01:34.41>Come <01:34.53>on <01:34.68>come <01:34.92>on <01:35.10>come <01:35.34>on <01:35.49>come <01:35.70>on <01:35.94>yeah
[01:36.36]<01:36.36>Now <01:36.60>we''re <01:36.78>off <01:36.96>to <01:37.14>meet <01:37.44>them <01:37.74>so <01:38.10>pack <01:38.49>your <01:38.64>weapons
[01:39.51]<01:39.51>Don''t <01:39.75>want <01:40.05>them <01:40.32>thinking <01:40.86>they <01:41.10>can <01:41.34>pull <01:41.61>a <01:41.70>fast <01:42.06>one <01:42.27>on <01:42.42>us
[01:42.66]<01:42.66>Now <01:42.84>do <01:43.17>we <01:43.29>Graeme
[01:43.77]<01:43.77>It''s <01:43.86>alright <01:44.37>I''ve <01:44.52>fought <01:44.88>more <01:45.12>wars <01:45.48>than <01:45.60>I''ve <01:45.75>had <01:45.90>hot <01:46.08>dinners
[01:46.50]<01:46.50>Sure <01:46.86>you <01:47.01>have <01:47.22>but <01:47.40>the <01:47.49>key <01:47.73>to <01:47.91>peace <01:48.24>lies <01:48.51>within <01:48.87>us
[01:49.17]<01:49.17>And <01:49.26>we''d <01:49.50>already <01:49.92>have <01:50.10>achieved <01:50.58>it
[01:50.76]<01:50.76>If <01:50.88>everyone <01:51.36>was <01:51.57>as <01:51.72>enlightened <01:52.47>as <01:52.65>me
[01:55.32]<01:55.32>It''s <01:55.53>hippy <01:56.01>bulls**t <01:56.76>but <01:57.09>it''s <01:57.33>true
[02:00.24]<02:00.24>Watch <02:00.57>me <02:00.81>explode
[02:09.96]<02:09.96>It''s <02:10.17>all <02:10.35>so <02:10.59>pointless <02:11.55>ah <02:12.03>but <02:12.18>it''s <02:12.36>not <02:12.60>though <02:12.93>is <02:13.11>it
[02:14.13]<02:14.13>It''s <02:14.28>really <02:14.76>real <02:15.15>and <02:15.33>when <02:15.60>you <02:15.75>feel <02:16.23>it <02:16.41>you <02:16.65>can <02:16.92>really <02:17.34>feel <02:17.85>it
[02:18.27]<02:18.27>Grab <02:18.54>somebody <02:19.14>that <02:19.32>you <02:19.53>love
[02:19.89]<02:19.89>Grab <02:20.10>anyone <02:20.79>who <02:20.94>needs <02:21.30>to <02:21.54>hear <02:21.99>it
[02:22.32]<02:22.32>And <02:22.44>shake <02:23.10>''em <02:23.19>by <02:23.43>the <02:23.58>shoulders <02:24.33>scream <02:25.17>in <02:25.29>their <02:25.47>face
[02:26.76]<02:26.76>Death <02:27.09>is <02:27.27>coming <02:27.75>for <02:27.93>us <02:28.11>all <02:28.50>but <02:28.71>not <02:28.98>today
[02:30.81]<02:30.81>Today <02:31.20>you''re <02:31.41>living <02:31.86>it <02:32.10>hey <02:32.70>you''re <02:32.91>really <02:33.30>feeling <02:33.93>it
[02:34.32]<02:34.32>Give <02:34.50>it <02:34.62>everything <02:35.37>you''ve <02:35.58>got <02:35.91>knowing <02:36.33>that <02:36.51>you <02:36.75>can''t <02:37.08>take <02:37.50>it <02:37.62>with <02:37.98>you
[02:38.25]<02:38.25>And <02:38.37>all <02:38.61>you <02:38.85>ever <02:39.12>needed <02:39.57>to <02:39.87>exist <02:40.53>has <02:40.77>always <02:41.25>been <02:41.49>within <02:41.91>you
[02:44.28]<02:44.28>Gimme <02:44.49>some <02:44.76>of <02:44.82>that <02:45.06>good <02:45.33>stuff <02:45.87>that <02:46.08>human <02:46.56>spirit
[02:47.85]<02:47.85>Cut <02:48.15>it <02:48.33>with <02:48.54>a <02:48.60>hundred <02:49.08>percent <02:49.68>endurance
[03:11.37]<03:11.37>It''s <03:11.76>all <03:11.97>so <03:12.39>pointless <03:14.16>sure <03:15.15>is
[03:19.74]<03:19.74>And <03:19.95>when <03:20.25>you''re <03:20.43>gone
[03:22.20]<03:22.20>It <03:22.26>makes <03:22.41>me <03:22.50>stronger <03:23.16>knowing
[03:24.03]<03:24.03>That <03:24.15>this <03:24.42>will <03:24.51>all <03:24.84>just <03:25.11>carry <03:25.59>on
[03:25.89]<03:25.89>With <03:26.10>someone <03:26.70>else
[03:27.36]<03:27.36>Someone <03:28.14>else
[03:29.40]<03:29.40>Something <03:30.03>new
[03:30.18]<03:30.18>Something <03:30.93>new
[03:31.14]<03:31.14>It''s <03:32.52>not <03:32.85>like <03:33.03>there''s <03:33.21>going <03:33.39>to <03:33.45>be <03:33.60>nothing <03:34.11>is <03:34.29>it
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (11014, 'lrc', 'line', 'local_lrc', '[00:04.11]You turn over the hour glass
[00:06.24]The sand is falling down
[00:08.01]Oh, it''s too fast for you
[00:11.74]For you
[00:12.82]Don''t waste your love, just let it last
[00:14.97]''Cause once it''s gone it''s never coming back
[00:19.31]It''s true
[00:20.13]Could you love me the same?
[00:24.42]Tell me what makes you stay
[00:30.03]There''s a hundred ways to leave a lover
[00:32.71]Leave a lover, leave a lover
[00:34.94]Hundred ways to leave a lover
[00:37.09]Leave a lover, leave a
[00:38.60]There''s a hundred ways to leave a lover
[00:41.48]I won''t wait a minute longer
[00:43.68]Hundred ways to leave
[00:46.05]But I''m the only one that you need
[00:49.78]
[00:56.65]It''s the final curtain call
[00:58.79]But if you''re ready I will give my all
[01:02.84]For you, for you
[01:05.41]Let them say it how they want
[01:07.30]If I can love you good, it''s no one''s fault
[01:11.55]Ooh
[01:12.52]Could you love me the same?
[01:16.73]Tell me what makes you stay
[01:22.45]There''s a hundred ways to leave a lover
[01:25.09]Leave a lover, leave a lover
[01:27.14]Hundred ways to leave a lover
[01:29.28]Leave a lover, leave a
[01:31.25]There''s a hundred ways to leave a lover
[01:33.84]I won''t wait a minute longer
[01:35.89]Hundred ways to leave
[01:38.49]But I''m the only one that you need
[01:42.38]
[01:49.66]I''m the only one that you need
[01:53.00]
[01:58.25]I''m the only one that you need
[02:02.51]I''m the only one that you need
[02:07.94]There''s a hundred ways to leave a lover
[02:10.77]Leave a lover, leave a lover
[02:13.05]Hundred ways to leave a lover
[02:14.95]Leave a lover, leave a
[02:16.70]There''s a hundred ways to leave a lover
[02:19.67]I won''t wait a minute longer
[02:21.70]Hundred ways to leave
[02:24.29]But I''m the only one that you need
[02:28.27]
[02:35.16]I''m the only one that you need
[02:39.13]
[02:44.09]I''m the only one that you need
[02:46.12]
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (18162, 'lrc', 'line', 'local_lrc', '[00:41.15]<00:41.15>I <00:41.41>just <00:41.59>nod  <00:42.72>I&apos;ve <00:42.95>never <00:43.59>been <00:44.09>so <00:44.34>good <00:44.70>at <00:44.95>shaking <00:45.15>hands
[00:48.20]<00:48.20>I <00:48.45>live <00:48.83>on <00:50.60>the <00:50.85>frozen <00:51.10>surface <00:51.54>of <00:51.97>a <00:52.23>fireball
[00:55.41]<00:55.41>Where <00:55.59>cities <00:55.85>come <00:56.16>together  <00:58.15>to <00:58.34>hate <00:58.59>each <00:58.84>other <00:59.15>in <00:59.40>the <00:59.65>name <01:00.03>of <01:00.28>sport
[01:02.89]<01:02.89>America  <01:06.08>nothing <01:06.27>is <01:06.52>ever <01:06.71>just <01:07.15>how <01:07.52>you <01:07.77>plan
[01:10.77]<01:10.77>I <01:11.01>looked <01:11.45>up <01:11.89>to <01:12.14>you <01:13.29>but <01:13.48>you <01:13.66>thought <01:13.97>I <01:14.41>would <01:14.60>look <01:14.98>the <01:15.16>other <01:15.41>way
[01:18.65]<01:18.65>And <01:19.03>you <01:19.15>hear  <01:20.46>what <01:20.65>you <01:20.84>want <01:22.02>to <01:22.83>hear
[01:26.26]<01:26.26>And <01:26.40>they <01:26.58>take <01:28.01>what <01:28.20>they <01:28.51>want <01:30.49>to <01:30.80>take
[01:33.80]<01:33.80>Don&apos;t <01:34.04>be <01:34.30>sad  <01:36.29>won&apos;t <01:36.54>ever <01:36.73>happen <01:37.29>like <01:37.54>this <01:37.85>anymore
[01:40.97]<01:40.97>So <01:41.22>when&apos;s <01:41.53>it <01:41.78>coming
[01:43.60]<01:43.60>This <01:43.91>life&apos;s <01:44.34>new <01:44.53>great <01:44.84>movement <01:45.59>that <01:45.78>I <01:45.96>can <01:46.40>join
[01:48.77]<01:48.77>The <01:49.08>warning <01:49.64>here
[01:51.52]<01:51.52>Your <01:51.71>faith <01:51.90>has <01:52.21>got <01:52.45>to <01:52.64>be <01:52.83>greater <01:53.20>than <01:53.39>your <01:53.64>fear
[01:57.44]<01:57.44>Forgive <01:58.38>them <01:59.44>even <01:59.88>if <02:00.21>they <02:00.53>are <02:00.90>not <02:01.28>sorry
[02:04.77]<02:04.77>All <02:05.02>the <02:05.33>vultures  <02:07.39>bootleggers <02:07.83>at <02:08.02>the <02:08.33>door <02:09.07>waiting
[02:12.51]<02:12.51>You <02:12.70>are <02:13.13>looking <02:15.19>for <02:15.44>your <02:15.63>own <02:16.00>voice  <02:16.38>but <02:16.75>you&apos;re <02:16.94>nervous
[02:20.37]<02:20.37>While <02:20.57>it <02:21.00>leaves <02:21.56>you <02:22.68>trapped <02:23.06>in <02:23.43>another <02:24.31>dimension
[02:27.81]<02:27.81>Drop <02:28.12>your <02:28.62>guard  <02:29.99>you <02:30.43>don&apos;t <02:30.80>have <02:31.06>to <02:31.24>be <02:31.43>smart <02:31.86>all <02:32.06>of <02:32.30>the <02:32.49>time
[02:35.23]<02:35.23>I <02:35.48>got <02:35.74>a <02:35.92>mind <02:36.11>full <02:36.36>of <02:36.60>blanks
[02:38.61]<02:38.61>I <02:38.79>need <02:38.97>to <02:39.17>go <02:39.41>somewhere <02:39.97>new <02:40.16>fast
[02:42.78]<02:42.78>And <02:43.04>don&apos;t <02:43.47>be <02:43.85>shy  <02:45.28>oh <02:45.66>no  <02:46.34>at <02:46.43>least <02:46.74>deliberately
[02:47.62]<02:47.62>No <02:47.93>one <02:48.18>really <02:48.68>cares <02:48.93>or <02:49.24>wanders <02:49.68>why <02:50.05>anymore
[02:51.05]<02:51.05>Oh <02:51.18>I <02:51.43>got <02:51.61>music  <02:52.49>coming <02:53.23>outta <02:53.42>my <02:53.61>hands <02:53.86>and <02:54.11>feet <02:54.30>and <02:54.54>kisses
[03:20.70]<03:20.70>That <03:22.20>is <03:22.51>how <03:24.14>it <03:24.38>once <03:26.01>was <03:26.32>done
[03:28.19]<03:28.19>All <03:29.44>the <03:30.06>dreamers <03:32.31>on <03:33.68>the <03:33.99>run
[03:35.68]<03:35.68>Forgive <03:36.55>them <03:37.17>even <03:38.05>if <03:38.42>they <03:38.67>are <03:39.17>not <03:39.61>sorry
[03:43.04]<03:43.04>All <03:43.29>the <03:43.60>vultures  <03:45.41>bootleggers <03:46.35>at <03:46.66>the <03:47.03>door <03:47.47>waiting
[03:50.72]<03:50.72>We&apos;re <03:51.18>so <03:51.56>quick <03:52.62>to <03:52.93>point <03:53.49>out <03:53.87>our <03:54.05>own <03:54.49>flaws <03:54.81>in <03:55.18>others
[03:58.49]<03:58.49>Complicated  <04:00.86>man <04:01.17>was <04:01.48>on <04:01.67>the <04:02.11>wings <04:02.48>of <04:02.73>robots
[04:13.63]<04:13.63>If <04:13.82>you <04:14.01>believe <04:14.26>in <04:15.63>this <04:16.01>world <04:17.19>your <04:17.44>not <04:17.63>inviting <04:18.69>me
[04:21.03]<04:21.03>But <04:21.40>don&apos;t <04:21.59>think <04:21.84>that <04:21.96>yet  <04:23.15>to <04:23.59>the <04:23.90>top  <04:24.96>now <04:25.05>know <04:25.24>what <04:25.55>to <04:25.80>do
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (12191, 'lrc', 'line', 'local_lrc', '[00:00.00]<00:00.00>1234<00:02.48> <00:04.97>1234<00:07.46> <00:09.94>-<00:12.43> <00:14.91>Catch<00:17.39> <00:19.88>22
[00:22.38]<00:22.38>You <00:23.38>you <00:23.55>try
[00:24.58]<00:24.58>You <00:24.74>try <00:24.95>to <00:25.12>get <00:25.53>by
[00:27.07]<00:27.07>You''re <00:27.22>never <00:27.66>going <00:28.27>to <00:28.32>pull <00:28.53>it <00:28.87>off
[00:29.23]<00:29.23>You <00:29.48>shouldn''t <00:30.16>even <00:30.51>try
[00:31.70]<00:31.70>You''re <00:31.87>a <00:32.04>wet <00:32.39>cigarette
[00:34.20]<00:34.20>You''re <00:34.52>always <00:34.91>second <00:35.40>best
[00:36.53]<00:36.53>But <00:36.83>they''re <00:36.94>never <00:37.46>going <00:37.67>to <00:38.08>give <00:38.30>a <00:38.65>s**t
[00:39.26]<00:39.26>About <00:40.02>anybody <00:40.92>but <00:41.16>themselves
[00:42.65]<00:42.65>So <00:42.84>you <00:43.05>fight <00:43.96>for <00:44.29>them <00:44.41>to <00:44.66>realize
[00:46.68]<00:46.68>There''s <00:46.85>more <00:46.90>to <00:47.20>life
[00:47.47]<00:47.47>There''s <00:47.80>more <00:48.36>to <00:48.56>you
[00:48.73]<00:48.73>There''s <00:49.01>more <00:49.33>than <00:49.63>meets <00:49.92>the <00:50.23>eye
[00:51.32]<00:51.32>And <00:51.73>when <00:52.49>you''re <00:52.73>done
[00:53.78]<00:53.78>The <00:54.00>battle''s <00:54.37>been <00:54.98>won
[00:56.42]<00:56.42>You <00:56.58>sit <00:56.80>back <00:57.62>you <00:57.84>smile
[00:58.72]<00:58.72>And <00:59.03>this <00:59.21>is <00:59.47>what <00:59.73>you <01:00.07>hum <01:00.27>you <01:00.59>hum
[01:21.45]<01:21.45>1234 <01:21.93>1234
[01:22.68]<01:22.68>You <01:22.92>you <01:23.17>try
[01:23.71]<01:23.71>You <01:23.77>try <01:23.91>to <01:24.04>get <01:24.26>by
[01:24.89]<01:24.89>You''re <01:25.04>never <01:25.21>going <01:25.22>to <01:25.37>pull <01:25.54>it <01:25.75>off
[01:25.78]<01:25.78>You <01:25.99>shouldn''t <01:26.15>even <01:26.41>try
[01:27.01]<01:27.01>You''re <01:27.19>a <01:27.37>wet <01:27.42>cigarette
[01:28.05]<01:28.05>You''re <01:28.21>always <01:28.48>second <01:28.70>best
[01:29.07]<01:29.07>But <01:29.28>they''re <01:29.45>never <01:29.64>going <01:29.66>to <01:29.82>give <01:29.97>a <01:30.15>s**t
[01:30.25]<01:30.25>About <01:30.95>anybody <01:31.26>but <01:31.45>themselves
[01:31.98]<01:31.98>You <01:32.14>fight
[01:32.55]<01:32.55>For <01:32.71>them <01:32.87>to <01:33.10>realize
[01:33.64]<01:33.64>There''s <01:33.80>more <01:34.08>to <01:34.14>life
[01:34.31]<01:34.31>There''s <01:34.48>more <01:34.63>to <01:34.78>you
[01:34.82]<01:34.82>There''s <01:34.98>more <01:35.15>than <01:35.30>meets <01:35.47>the <01:35.51>eye
[01:36.00]<01:36.00>And <01:36.17>when <01:36.36>you''re <01:36.59>done
[01:37.12]<01:37.12>Your <01:37.27>battle''s <01:37.49>been <01:37.73>won
[01:38.26]<01:38.26>You <01:38.34>sit <01:38.50>back
[01:38.81]<01:38.81>You <01:38.97>smile
[01:39.41]<01:39.41>And <01:39.57>this <01:39.60>is <01:39.78>what <01:39.95>you <01:40.09>hum <01:40.27>you <01:40.33>hum
[01:58.75]<01:58.75>1234 <01:59.16>1234
[01:59.88]<01:59.88>Years <02:00.08>go <02:00.35>by
[02:00.93]<02:00.93>The <02:01.05>time <02:01.22>it <02:01.30>does <02:01.54>fly
[02:02.18]<02:02.18>Every <02:02.42>single <02:02.66>second
[02:02.94]<02:02.94>Is <02:03.10>a <02:03.26>moment <02:03.54>in <02:03.76>time
[02:04.25]<02:04.25>That <02:04.47>passes <02:04.70>oh <02:04.90>so <02:04.92>quick
[02:05.43]<02:05.43>And <02:05.59>it <02:05.75>seems <02:05.94>like <02:06.12>nothing
[02:06.67]<02:06.67>But <02:06.83>when <02:06.85>you''re <02:07.01>looking <02:07.18>back
[02:07.27]<02:07.27>Well <02:07.48>it <02:07.59>amounts <02:08.03>to <02:08.23>everything
[02:08.97]<02:08.97>I''ve <02:09.15>got <02:09.41>myself
[02:10.04]<02:10.04>I''ve <02:10.16>got <02:10.32>my <02:10.52>friends
[02:11.16]<02:11.16>I''ve <02:11.30>got <02:11.46>my <02:11.55>little <02:11.81>family
[02:12.12]<02:12.12>But <02:12.28>that''s <02:12.48>not <02:12.65>where <02:12.84>it <02:13.04>ends
[02:13.52]<02:13.52>This <02:13.52>one <02:13.67>goes <02:13.90>out <02:14.07>to <02:14.24>you
[02:14.48]<02:14.48>It <02:14.66>goes <02:14.81>out <02:14.96>to <02:15.03>everyone
[02:15.73]<02:15.73>It''s <02:15.90>in <02:16.06>the <02:16.16>name <02:16.36>of <02:16.49>honesty
[02:16.71]<02:16.71>Because <02:16.90>life <02:17.07>has <02:17.25>just <02:17.48>begun
[02:46.10]<02:46.10>Look <02:46.31>around <02:46.62>little <02:46.86>brother
[02:47.12]<02:47.12>Can <02:47.29>you <02:47.48>tell <02:47.51>me <02:47.70>what <02:47.85>you <02:48.03>see
[02:48.43]<02:48.43>You''re <02:48.59>a <02:48.60>big <02:48.82>boy <02:49.09>now
[02:49.11]<02:49.11>So <02:49.35>take <02:49.53>responsibility
[02:50.67]<02:50.67>You <02:50.81>never <02:50.98>had <02:51.04>it <02:51.14>hard
[02:51.79]<02:51.79>But <02:51.93>now <02:52.09>it''s <02:52.22>getting <02:52.41>tough
[02:52.64]<02:52.64>So <02:52.78>you <02:52.96>whine <02:53.26>whine <02:53.53>whine
[02:53.93]<02:53.93>And <02:54.09>you <02:54.13>say <02:54.29>you''ve <02:54.43>had <02:54.66>enough
[02:55.22]<02:55.22>You <02:55.38>say <02:55.38>I''m <02:55.56>full <02:55.72>of <02:55.86>s**t
[02:56.01]<02:56.01>That <02:56.19>I''m <02:56.35>a <02:56.60>hypocrite
[02:57.32]<02:57.32>I <02:57.47>shouldn''t <02:57.70>talk
[02:57.76]<02:57.76>When <02:57.98>I <02:58.14>can''t <02:58.31>take <02:58.46>the <02:58.58>advice <02:58.85>that <02:59.01>I <02:59.17>give
[02:59.57]<02:59.57>Well <02:59.75>maybe <02:59.99>you''re <03:00.24>right
[03:00.73]<03:00.73>But <03:00.88>open <03:01.07>your <03:01.32>eyes
[03:01.78]<03:01.78>The <03:01.96>main <03:02.17>difference <03:02.43>here
[03:02.46]<03:02.46>Is <03:02.70>that <03:02.86>I <03:03.06>try <03:03.36>try <03:03.66>try
[03:35.74]<03:35.74>1234 <03:36.20>1234
[03:55.03]<03:55.03>1234 <03:55.42>1
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES (12293, 'lrc', 'line', 'local_lrc', '[00:42.00]I''ve been busy counting
[00:47.00]Every drop of rain
[00:52.00]And I ain''t scared of drowning
[00:57.00]Cause I told you once, I told you twice
[01:00.00]I swear I must have said a thousand times
[01:02.00]If I am the water
[01:07.00]Then I am the rain
[01:12.00]If I am the fire
[01:17.00]Then I am the flame
[01:27.00]Eleven eleven
[01:32.00]Eleven eleven
[01:37.00]Eleven eleven
[01:43.00]I''ve been busy burning
[01:48.00]Matches with the flame(matches with the flame)
[01:53.00]But I ain''t scared of dying
[01:58.00]Cause I told you once, I told you twice
[02:00.00]I swear I must have said a thousand times
[02:03.00]If I am the water
[02:08.00]Then I am the rain
[02:13.00]If I am the fire
[02:18.00]Then I am the flame
[02:23.00]Eleven eleven
[02:28.00]Eleven eleven
[02:33.00]Eleven eleven
[02:38.00]Eleven eleven
[02:44.00]They''re trying to get out
[02:54.00]They''re never gonna get out
[03:08.00]Eleven eleven
[03:13.00]Eleven eleven
[03:22.00]Eleven eleven
[03:27.00]Eleven eleven
[03:32.00]Eleven eleven
[03:37.00]Eleven eleven
', 0, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;

COMMIT;
