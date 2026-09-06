-- =====================================================================
-- Syncify Playlist Deduplication SQL Script (Task F2.4 / Mitigates A3)
-- Generated: 2026-09-04 01:15:57 UTC
-- Database: syncify_backup_pre_repair.db
-- Duplicate groups analyzed: 58
-- =====================================================================

PRAGMA foreign_keys = ON;
BEGIN TRANSACTION;

-- ---------------------------------------------------------------------
-- Group 1/58: "miguel+j.luis" (Account 4, 2 playlists)
-- Primary Winner: ID 139 ('Miguel+J.Luis'), initial tracks: 50
-- ---------------------------------------------------------------------
-- Merging Loser ID 138 ('Miguel+J.Luis'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 138 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 139);
UPDATE playlist_sources SET playlist_id = 139 WHERE playlist_id = 138;
DELETE FROM playlist_tracks WHERE playlist_id = 138;
DELETE FROM playlists WHERE id = 138;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 139), updated_at = CURRENT_TIMESTAMP WHERE id = 139;

-- ---------------------------------------------------------------------
-- Group 2/58: "10albums2023" (Account 9, 2 playlists)
-- Primary Winner: ID 265 ('10albums2023'), initial tracks: 107
-- ---------------------------------------------------------------------
-- Merging Loser ID 72 ('10albums2023'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 72 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 265);
UPDATE playlist_sources SET playlist_id = 265 WHERE playlist_id = 72;
DELETE FROM playlist_tracks WHERE playlist_id = 72;
DELETE FROM playlists WHERE id = 72;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 265), updated_at = CURRENT_TIMESTAMP WHERE id = 265;

-- ---------------------------------------------------------------------
-- Group 3/58: "arroba" (Account 9, 2 playlists)
-- Primary Winner: ID 32 ('Arroba'), initial tracks: 64
-- ---------------------------------------------------------------------
-- Merging Loser ID 366 ('Arroba'): Jaccard=0.938, Containment=0.984, Extra Tracks=1
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (32, 23210, 65, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 366 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 32);
UPDATE playlist_sources SET playlist_id = 32 WHERE playlist_id = 366;
DELETE FROM playlist_tracks WHERE playlist_id = 366;
DELETE FROM playlists WHERE id = 366;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 32), updated_at = CURRENT_TIMESTAMP WHERE id = 32;

-- ---------------------------------------------------------------------
-- Group 4/58: "blue stage" (Account 9, 2 playlists)
-- Primary Winner: ID 206 ('Blue Stage'), initial tracks: 35
-- ---------------------------------------------------------------------
-- Merging Loser ID 40 ('Blue Stage'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 40 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 206);
UPDATE playlist_sources SET playlist_id = 206 WHERE playlist_id = 40;
DELETE FROM playlist_tracks WHERE playlist_id = 40;
DELETE FROM playlists WHERE id = 40;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 206), updated_at = CURRENT_TIMESTAMP WHERE id = 206;

-- ---------------------------------------------------------------------
-- Group 5/58: "canción del día" (Account 9, 2 playlists)
-- Primary Winner: ID 264 ('Canción del día'), initial tracks: 12
-- ---------------------------------------------------------------------
-- Merging Loser ID 63 ('Canción del día'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 63 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 264);
UPDATE playlist_sources SET playlist_id = 264 WHERE playlist_id = 63;
DELETE FROM playlist_tracks WHERE playlist_id = 63;
DELETE FROM playlists WHERE id = 63;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 264), updated_at = CURRENT_TIMESTAMP WHERE id = 264;

-- ---------------------------------------------------------------------
-- Group 6/58: "d1" (Account 9, 3 playlists)
-- Primary Winner: ID 220 ('D1'), initial tracks: 442
-- ---------------------------------------------------------------------
-- Merging Loser ID 189 ('D1'): Jaccard=0.982, Containment=0.991, Extra Tracks=4
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 1276, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 8168, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 5797, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 9486, 492, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 189 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 220);
UPDATE playlist_sources SET playlist_id = 220 WHERE playlist_id = 189;
DELETE FROM playlist_tracks WHERE playlist_id = 189;
DELETE FROM playlists WHERE id = 189;
-- Merging Loser ID 416 ('D1'): Jaccard=0.919, Containment=0.977, Extra Tracks=10
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 6936, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 3813, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 4461, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 8667, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 9315, 497, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 10365, 498, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 382, 499, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 7528, 500, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 6678, 501, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (220, 10232, 502, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 416 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 220);
UPDATE playlist_sources SET playlist_id = 220 WHERE playlist_id = 416;
DELETE FROM playlist_tracks WHERE playlist_id = 416;
DELETE FROM playlists WHERE id = 416;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 220), updated_at = CURRENT_TIMESTAMP WHERE id = 220;

-- ---------------------------------------------------------------------
-- Group 7/58: "d10" (Account 9, 3 playlists)
-- Primary Winner: ID 230 ('D10'), initial tracks: 427
-- ---------------------------------------------------------------------
-- Merging Loser ID 161 ('D10'): Jaccard=0.981, Containment=0.991, Extra Tracks=4
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (230, 10127, 481, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (230, 7733, 482, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (230, 8228, 483, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (230, 428, 484, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 161 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 230);
UPDATE playlist_sources SET playlist_id = 230 WHERE playlist_id = 161;
DELETE FROM playlist_tracks WHERE playlist_id = 161;
DELETE FROM playlists WHERE id = 161;
-- Merging Loser ID 402 ('D10'): Jaccard=0.925, Containment=0.981, Extra Tracks=8
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (230, 11161, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (230, 4605, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (230, 8898, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (230, 11187, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (230, 1885, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (230, 10405, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (230, 10842, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (230, 7170, 492, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 402 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 230);
UPDATE playlist_sources SET playlist_id = 230 WHERE playlist_id = 402;
DELETE FROM playlist_tracks WHERE playlist_id = 402;
DELETE FROM playlists WHERE id = 402;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 230), updated_at = CURRENT_TIMESTAMP WHERE id = 230;

-- ---------------------------------------------------------------------
-- Group 8/58: "d11" (Account 9, 3 playlists)
-- Primary Winner: ID 126 ('D11'), initial tracks: 427
-- ---------------------------------------------------------------------
-- Merging Loser ID 231 ('D11'): Jaccard=0.943, Containment=0.972, Extra Tracks=12
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 9711, 474, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 2428, 475, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 3190, 476, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 3191, 477, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 3189, 478, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 3192, 479, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 3333, 480, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 10018, 481, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 4427, 482, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 3655, 483, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 4870, 484, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 4834, 485, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 231 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 126);
UPDATE playlist_sources SET playlist_id = 126 WHERE playlist_id = 231;
DELETE FROM playlist_tracks WHERE playlist_id = 231;
DELETE FROM playlists WHERE id = 231;
-- Merging Loser ID 403 ('D11'): Jaccard=0.937, Containment=0.988, Extra Tracks=5
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 8617, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 10002, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 8268, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 4213, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (126, 7758, 490, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 403 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 126);
UPDATE playlist_sources SET playlist_id = 126 WHERE playlist_id = 403;
DELETE FROM playlist_tracks WHERE playlist_id = 403;
DELETE FROM playlists WHERE id = 403;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 126), updated_at = CURRENT_TIMESTAMP WHERE id = 126;

-- ---------------------------------------------------------------------
-- Group 9/58: "d12" (Account 9, 3 playlists)
-- Primary Winner: ID 181 ('D12'), initial tracks: 404
-- ---------------------------------------------------------------------
-- Merging Loser ID 232 ('D12'): Jaccard=0.935, Containment=0.968, Extra Tracks=13
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 865, 461, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 10020, 462, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 11079, 463, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 8194, 464, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 19251, 465, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 11349, 466, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 1135, 467, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 3196, 468, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 3399, 469, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 7309, 470, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 12195, 471, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 12455, 472, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 19252, 473, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 232 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 181);
UPDATE playlist_sources SET playlist_id = 181 WHERE playlist_id = 232;
DELETE FROM playlist_tracks WHERE playlist_id = 232;
DELETE FROM playlists WHERE id = 232;
-- Merging Loser ID 404 ('D12'): Jaccard=0.927, Containment=0.980, Extra Tracks=8
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 11336, 474, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 6730, 475, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 9003, 476, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 5891, 477, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 10404, 478, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 10362, 479, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 23287, 480, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (181, 9823, 481, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 404 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 181);
UPDATE playlist_sources SET playlist_id = 181 WHERE playlist_id = 404;
DELETE FROM playlist_tracks WHERE playlist_id = 404;
DELETE FROM playlists WHERE id = 404;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 181), updated_at = CURRENT_TIMESTAMP WHERE id = 181;

-- ---------------------------------------------------------------------
-- Group 10/58: "d13" (Account 9, 3 playlists)
-- Primary Winner: ID 233 ('D13'), initial tracks: 443
-- ---------------------------------------------------------------------
-- Merging Loser ID 175 ('D13'): Jaccard=0.903, Containment=0.954, Extra Tracks=20
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 932, 482, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11384, 483, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 9373, 484, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 3532, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11407, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11408, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11410, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11418, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11419, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11420, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11423, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11426, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 5550, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 5087, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 2251, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 14688, 497, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11532, 498, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 726, 499, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11297, 500, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 1274, 501, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 175 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 233);
UPDATE playlist_sources SET playlist_id = 233 WHERE playlist_id = 175;
DELETE FROM playlist_tracks WHERE playlist_id = 175;
DELETE FROM playlists WHERE id = 175;
-- Merging Loser ID 405 ('D13'): Jaccard=0.844, Containment=0.960, Extra Tracks=17
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 6672, 502, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 6671, 503, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 4023, 504, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 1822, 505, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 3810, 506, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11409, 507, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 1255, 508, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11411, 509, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 4886, 510, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11413, 511, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 11206, 512, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 5583, 513, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 7720, 514, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 23288, 515, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 12196, 516, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 12413, 517, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (233, 12434, 518, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 405 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 233);
UPDATE playlist_sources SET playlist_id = 233 WHERE playlist_id = 405;
DELETE FROM playlist_tracks WHERE playlist_id = 405;
DELETE FROM playlists WHERE id = 405;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 233), updated_at = CURRENT_TIMESTAMP WHERE id = 233;

-- ---------------------------------------------------------------------
-- Group 11/58: "d14" (Account 9, 3 playlists)
-- Primary Winner: ID 406 ('D14'), initial tracks: 435
-- ---------------------------------------------------------------------
-- Merging Loser ID 234 ('D14'): Jaccard=0.888, Containment=0.951, Extra Tracks=21
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 3852, 481, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 2212, 482, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 3654, 483, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 2215, 484, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 2216, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 3563, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 4652, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 3240, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 3991, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 3801, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 3570, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 2280, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 3683, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 11689, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 3682, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 2294, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 2685, 497, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 2292, 498, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 4858, 499, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 2413, 500, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 2456, 501, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 234 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 406);
UPDATE playlist_sources SET playlist_id = 406 WHERE playlist_id = 234;
DELETE FROM playlist_tracks WHERE playlist_id = 234;
DELETE FROM playlists WHERE id = 234;
-- Merging Loser ID 150 ('D14'): Jaccard=0.905, Containment=0.988, Extra Tracks=5
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 1373, 502, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 748, 503, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 830, 504, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 943, 505, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (406, 1273, 506, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 150 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 406);
UPDATE playlist_sources SET playlist_id = 406 WHERE playlist_id = 150;
DELETE FROM playlist_tracks WHERE playlist_id = 150;
DELETE FROM playlists WHERE id = 150;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 406), updated_at = CURRENT_TIMESTAMP WHERE id = 406;

-- ---------------------------------------------------------------------
-- Group 12/58: "d15" (Account 9, 3 playlists)
-- Primary Winner: ID 113 ('D15'), initial tracks: 430
-- ---------------------------------------------------------------------
-- Merging Loser ID 235 ('D15'): Jaccard=0.961, Containment=0.981, Extra Tracks=8
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 1382, 482, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 4558, 483, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 4724, 484, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 4005, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 19266, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 19267, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 5866, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 4751, 489, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 235 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 113);
UPDATE playlist_sources SET playlist_id = 113 WHERE playlist_id = 235;
DELETE FROM playlist_tracks WHERE playlist_id = 235;
DELETE FROM playlists WHERE id = 235;
-- Merging Loser ID 407 ('D15'): Jaccard=0.922, Containment=0.979, Extra Tracks=9
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 8848, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 8678, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 5246, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 8806, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 8675, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 10233, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 7564, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 4093, 497, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (113, 1497, 498, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 407 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 113);
UPDATE playlist_sources SET playlist_id = 113 WHERE playlist_id = 407;
DELETE FROM playlist_tracks WHERE playlist_id = 407;
DELETE FROM playlists WHERE id = 407;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 113), updated_at = CURRENT_TIMESTAMP WHERE id = 113;

-- ---------------------------------------------------------------------
-- Group 13/58: "d16" (Account 9, 3 playlists)
-- Primary Winner: ID 73 ('D16'), initial tracks: 403
-- ---------------------------------------------------------------------
-- Merging Loser ID 236 ('D16'): Jaccard=0.949, Containment=0.980, Extra Tracks=8
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 4025, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 2274, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 19268, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 18632, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 10299, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 4839, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 3798, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 9405, 496, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 236 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 73);
UPDATE playlist_sources SET playlist_id = 73 WHERE playlist_id = 236;
DELETE FROM playlist_tracks WHERE playlist_id = 236;
DELETE FROM playlists WHERE id = 236;
-- Merging Loser ID 408 ('D16'): Jaccard=0.919, Containment=0.982, Extra Tracks=7
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 1333, 497, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 11425, 498, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 6741, 499, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 11300, 500, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 23289, 501, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 23290, 502, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (73, 60, 503, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 408 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 73);
UPDATE playlist_sources SET playlist_id = 73 WHERE playlist_id = 408;
DELETE FROM playlist_tracks WHERE playlist_id = 408;
DELETE FROM playlists WHERE id = 408;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 73), updated_at = CURRENT_TIMESTAMP WHERE id = 73;

-- ---------------------------------------------------------------------
-- Group 14/58: "d17" (Account 9, 3 playlists)
-- Primary Winner: ID 409 ('D17'), initial tracks: 439
-- ---------------------------------------------------------------------
-- Merging Loser ID 117 ('D17'): Jaccard=0.875, Containment=0.942, Extra Tracks=25
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 3299, 484, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 743, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 3187, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 3186, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 2369, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 5535, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 3201, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 955, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 3724, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 2539, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 909, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 4280, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 5104, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 3947, 497, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 4871, 498, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 12048, 499, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 2798, 500, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 9346, 501, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 1069, 502, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 962, 503, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 14677, 504, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 1052, 505, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 880, 506, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 5534, 507, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 14826, 508, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 117 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 409);
UPDATE playlist_sources SET playlist_id = 409 WHERE playlist_id = 117;
DELETE FROM playlist_tracks WHERE playlist_id = 117;
DELETE FROM playlists WHERE id = 117;
-- Merging Loser ID 237 ('D17'): Jaccard=0.906, Containment=0.991, Extra Tracks=4
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 4747, 509, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 4759, 510, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 4130, 511, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (409, 3199, 512, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 237 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 409);
UPDATE playlist_sources SET playlist_id = 409 WHERE playlist_id = 237;
DELETE FROM playlist_tracks WHERE playlist_id = 237;
DELETE FROM playlists WHERE id = 237;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 409), updated_at = CURRENT_TIMESTAMP WHERE id = 409;

-- ---------------------------------------------------------------------
-- Group 15/58: "d18" (Account 9, 3 playlists)
-- Primary Winner: ID 56 ('D18'), initial tracks: 408
-- ---------------------------------------------------------------------
-- Merging Loser ID 410 ('D18'): Jaccard=0.922, Containment=0.963, Extra Tracks=15
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 7466, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 11421, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 9002, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 10497, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 10950, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 10461, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 10446, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 11776, 497, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 12260, 498, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 3536, 499, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 9403, 500, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 6164, 501, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 10490, 502, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 12423, 503, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 8063, 504, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 410 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 56);
UPDATE playlist_sources SET playlist_id = 56 WHERE playlist_id = 410;
DELETE FROM playlist_tracks WHERE playlist_id = 410;
DELETE FROM playlists WHERE id = 410;
-- Merging Loser ID 238 ('D18'): Jaccard=0.923, Containment=0.983, Extra Tracks=7
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 4506, 505, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 4505, 506, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 3059, 507, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 3541, 508, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 3686, 509, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 4864, 510, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (56, 4430, 511, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 238 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 56);
UPDATE playlist_sources SET playlist_id = 56 WHERE playlist_id = 238;
DELETE FROM playlist_tracks WHERE playlist_id = 238;
DELETE FROM playlists WHERE id = 238;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 56), updated_at = CURRENT_TIMESTAMP WHERE id = 56;

-- ---------------------------------------------------------------------
-- Group 16/58: "d19" (Account 9, 3 playlists)
-- Primary Winner: ID 411 ('D19'), initial tracks: 441
-- ---------------------------------------------------------------------
-- Merging Loser ID 59 ('D19'): Jaccard=0.916, Containment=0.968, Extra Tracks=14
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 4835, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 14162, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 4851, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 5532, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 5522, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 3063, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 1035, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 1174, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 4861, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 3321, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 847, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 3323, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 12940, 497, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 3301, 498, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 59 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 411);
UPDATE playlist_sources SET playlist_id = 411 WHERE playlist_id = 59;
DELETE FROM playlist_tracks WHERE playlist_id = 59;
DELETE FROM playlists WHERE id = 59;
-- Merging Loser ID 239 ('D19'): Jaccard=0.897, Containment=0.974, Extra Tracks=11
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 4726, 499, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 4743, 500, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 4717, 501, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 5527, 502, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 5528, 503, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 2199, 504, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 3401, 505, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 4748, 506, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 2429, 507, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 19269, 508, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (411, 4966, 509, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 239 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 411);
UPDATE playlist_sources SET playlist_id = 411 WHERE playlist_id = 239;
DELETE FROM playlist_tracks WHERE playlist_id = 239;
DELETE FROM playlists WHERE id = 239;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 411), updated_at = CURRENT_TIMESTAMP WHERE id = 411;

-- ---------------------------------------------------------------------
-- Group 17/58: "d2" (Account 9, 3 playlists)
-- Primary Winner: ID 221 ('D2'), initial tracks: 408
-- ---------------------------------------------------------------------
-- Merging Loser ID 125 ('D2'): Jaccard=0.985, Containment=0.995, Extra Tracks=2
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (221, 9658, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (221, 2406, 490, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 125 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 221);
UPDATE playlist_sources SET playlist_id = 221 WHERE playlist_id = 125;
DELETE FROM playlist_tracks WHERE playlist_id = 125;
DELETE FROM playlists WHERE id = 125;
-- Merging Loser ID 393 ('D2'): Jaccard=0.954, Containment=0.983, Extra Tracks=7
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (221, 10056, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (221, 11971, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (221, 5800, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (221, 9989, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (221, 9317, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (221, 11072, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (221, 7340, 497, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 393 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 221);
UPDATE playlist_sources SET playlist_id = 221 WHERE playlist_id = 393;
DELETE FROM playlist_tracks WHERE playlist_id = 393;
DELETE FROM playlists WHERE id = 393;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 221), updated_at = CURRENT_TIMESTAMP WHERE id = 221;

-- ---------------------------------------------------------------------
-- Group 18/58: "d20" (Account 9, 3 playlists)
-- Primary Winner: ID 111 ('D20'), initial tracks: 436
-- ---------------------------------------------------------------------
-- Merging Loser ID 241 ('D20'): Jaccard=0.957, Containment=0.979, Extra Tracks=9
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 19270, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 1459, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 18, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 7184, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 4288, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 1439, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 12404, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 2873, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 9219, 497, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 241 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 111);
UPDATE playlist_sources SET playlist_id = 111 WHERE playlist_id = 241;
DELETE FROM playlist_tracks WHERE playlist_id = 241;
DELETE FROM playlists WHERE id = 241;
-- Merging Loser ID 413 ('D20'): Jaccard=0.921, Containment=0.974, Extra Tracks=11
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 11074, 498, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 11226, 499, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 12219, 500, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 5148, 501, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 6654, 502, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 12003, 503, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 10958, 504, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 12323, 505, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 3519, 506, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 2940, 507, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (111, 9090, 508, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 413 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 111);
UPDATE playlist_sources SET playlist_id = 111 WHERE playlist_id = 413;
DELETE FROM playlist_tracks WHERE playlist_id = 413;
DELETE FROM playlists WHERE id = 413;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 111), updated_at = CURRENT_TIMESTAMP WHERE id = 111;

-- ---------------------------------------------------------------------
-- Group 19/58: "d21" (Account 9, 3 playlists)
-- Primary Winner: ID 68 ('D21'), initial tracks: 451
-- ---------------------------------------------------------------------
-- Merging Loser ID 240 ('D21'): Jaccard=0.969, Containment=0.989, Extra Tracks=5
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 3065, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 2981, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 4841, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 2826, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 12247, 497, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 240 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 68);
UPDATE playlist_sources SET playlist_id = 68 WHERE playlist_id = 240;
DELETE FROM playlist_tracks WHERE playlist_id = 240;
DELETE FROM playlists WHERE id = 240;
-- Merging Loser ID 412 ('D21'): Jaccard=0.917, Containment=0.969, Extra Tracks=14
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 9927, 498, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 8791, 499, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 9577, 500, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 2839, 501, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 9634, 502, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 11697, 503, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 8931, 504, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 7085, 505, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 6562, 506, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 7059, 507, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 6285, 508, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 4134, 509, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 5602, 510, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (68, 8472, 511, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 412 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 68);
UPDATE playlist_sources SET playlist_id = 68 WHERE playlist_id = 412;
DELETE FROM playlist_tracks WHERE playlist_id = 412;
DELETE FROM playlists WHERE id = 412;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 68), updated_at = CURRENT_TIMESTAMP WHERE id = 68;

-- ---------------------------------------------------------------------
-- Group 20/58: "d22" (Account 9, 3 playlists)
-- Primary Winner: ID 67 ('D22'), initial tracks: 403
-- ---------------------------------------------------------------------
-- Merging Loser ID 243 ('D22'): Jaccard=0.949, Containment=0.978, Extra Tracks=9
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 3747, 471, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 4852, 472, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 19271, 473, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 11874, 474, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 8545, 475, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 2566, 476, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 3685, 477, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 5408, 478, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 3067, 479, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 243 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 67);
UPDATE playlist_sources SET playlist_id = 67 WHERE playlist_id = 243;
DELETE FROM playlist_tracks WHERE playlist_id = 243;
DELETE FROM playlists WHERE id = 243;
-- Merging Loser ID 415 ('D22'): Jaccard=0.908, Containment=0.972, Extra Tracks=11
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 12476, 480, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 1535, 481, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 6853, 482, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 7452, 483, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 1495, 484, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 10556, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 10496, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 8172, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 135, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 10492, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (67, 7171, 490, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 415 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 67);
UPDATE playlist_sources SET playlist_id = 67 WHERE playlist_id = 415;
DELETE FROM playlist_tracks WHERE playlist_id = 415;
DELETE FROM playlists WHERE id = 415;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 67), updated_at = CURRENT_TIMESTAMP WHERE id = 67;

-- ---------------------------------------------------------------------
-- Group 21/58: "d23" (Account 9, 3 playlists)
-- Primary Winner: ID 115 ('D23'), initial tracks: 195
-- ---------------------------------------------------------------------
-- Merging Loser ID 242 ('D23'): Jaccard=0.935, Containment=0.974, Extra Tracks=5
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (115, 18767, 210, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (115, 12254, 211, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (115, 4142, 212, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (115, 10451, 213, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (115, 3403, 214, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 242 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 115);
UPDATE playlist_sources SET playlist_id = 115 WHERE playlist_id = 242;
DELETE FROM playlist_tracks WHERE playlist_id = 242;
DELETE FROM playlists WHERE id = 242;
-- Merging Loser ID 414 ('D23'): Jaccard=0.896, Containment=0.994, Extra Tracks=1
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (115, 10111, 215, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 414 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 115);
UPDATE playlist_sources SET playlist_id = 115 WHERE playlist_id = 414;
DELETE FROM playlist_tracks WHERE playlist_id = 414;
DELETE FROM playlists WHERE id = 414;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 115), updated_at = CURRENT_TIMESTAMP WHERE id = 115;

-- ---------------------------------------------------------------------
-- Group 22/58: "d3" (Account 9, 3 playlists)
-- Primary Winner: ID 222 ('D3'), initial tracks: 415
-- ---------------------------------------------------------------------
-- Merging Loser ID 176 ('D3'): Jaccard=0.921, Containment=0.961, Extra Tracks=16
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 826, 480, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 5778, 481, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 1149, 482, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 9870, 483, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 1054, 484, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 1152, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 735, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 11717, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 2248, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 4154, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 6028, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 6900, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 9641, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 2486, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 10208, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 910, 495, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 176 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 222);
UPDATE playlist_sources SET playlist_id = 222 WHERE playlist_id = 176;
DELETE FROM playlist_tracks WHERE playlist_id = 176;
DELETE FROM playlists WHERE id = 176;
-- Merging Loser ID 394 ('D3'): Jaccard=0.878, Containment=0.975, Extra Tracks=10
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 10124, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 8314, 497, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 9096, 498, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 11283, 499, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 11225, 500, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 7914, 501, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 8046, 502, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 10826, 503, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 11155, 504, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (222, 8092, 505, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 394 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 222);
UPDATE playlist_sources SET playlist_id = 222 WHERE playlist_id = 394;
DELETE FROM playlist_tracks WHERE playlist_id = 394;
DELETE FROM playlists WHERE id = 394;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 222), updated_at = CURRENT_TIMESTAMP WHERE id = 222;

-- ---------------------------------------------------------------------
-- Group 23/58: "d4" (Account 9, 3 playlists)
-- Primary Winner: ID 187 ('D4'), initial tracks: 439
-- ---------------------------------------------------------------------
-- Merging Loser ID 223 ('D4'): Jaccard=0.964, Containment=0.988, Extra Tracks=5
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 5304, 484, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 10216, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 2504, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 2419, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 170, 488, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 223 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 187);
UPDATE playlist_sources SET playlist_id = 187 WHERE playlist_id = 223;
DELETE FROM playlist_tracks WHERE playlist_id = 223;
DELETE FROM playlists WHERE id = 223;
-- Merging Loser ID 395 ('D4'): Jaccard=0.904, Containment=0.965, Extra Tracks=15
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 11471, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 11216, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 12307, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 1293, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 5263, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 11771, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 9001, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 11753, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 7591, 497, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 6867, 498, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 6229, 499, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 7272, 500, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 8286, 501, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 9495, 502, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (187, 7655, 503, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 395 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 187);
UPDATE playlist_sources SET playlist_id = 187 WHERE playlist_id = 395;
DELETE FROM playlist_tracks WHERE playlist_id = 395;
DELETE FROM playlists WHERE id = 395;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 187), updated_at = CURRENT_TIMESTAMP WHERE id = 187;

-- ---------------------------------------------------------------------
-- Group 24/58: "d5" (Account 9, 3 playlists)
-- Primary Winner: ID 224 ('D5'), initial tracks: 440
-- ---------------------------------------------------------------------
-- Merging Loser ID 134 ('D5'): Jaccard=0.958, Containment=0.979, Extra Tracks=9
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 8387, 481, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 1181, 482, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 5865, 483, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 953, 484, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 949, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 1047, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 734, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 812, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 838, 489, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 134 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 224);
UPDATE playlist_sources SET playlist_id = 224 WHERE playlist_id = 134;
DELETE FROM playlist_tracks WHERE playlist_id = 134;
DELETE FROM playlists WHERE id = 134;
-- Merging Loser ID 396 ('D5'): Jaccard=0.898, Containment=0.972, Extra Tracks=12
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 8269, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 10260, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 7175, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 10501, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 8552, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 6539, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 11111, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 7017, 497, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 12267, 498, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 6582, 499, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 6769, 500, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (224, 11629, 501, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 396 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 224);
UPDATE playlist_sources SET playlist_id = 224 WHERE playlist_id = 396;
DELETE FROM playlist_tracks WHERE playlist_id = 396;
DELETE FROM playlists WHERE id = 396;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 224), updated_at = CURRENT_TIMESTAMP WHERE id = 224;

-- ---------------------------------------------------------------------
-- Group 25/58: "d6" (Account 9, 3 playlists)
-- Primary Winner: ID 122 ('D6'), initial tracks: 430
-- ---------------------------------------------------------------------
-- Merging Loser ID 226 ('D6'): Jaccard=0.957, Containment=0.981, Extra Tracks=8
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 5196, 474, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 11382, 475, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 4642, 476, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 2776, 477, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 5357, 478, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 4641, 479, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 2775, 480, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 4986, 481, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 226 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 122);
UPDATE playlist_sources SET playlist_id = 122 WHERE playlist_id = 226;
DELETE FROM playlist_tracks WHERE playlist_id = 226;
DELETE FROM playlists WHERE id = 226;
-- Merging Loser ID 398 ('D6'): Jaccard=0.893, Containment=0.971, Extra Tracks=12
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 12483, 482, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 8501, 483, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 12252, 484, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 9043, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 5590, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 10998, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 10999, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 10049, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 23285, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 8206, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 8566, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (122, 12221, 493, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 398 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 122);
UPDATE playlist_sources SET playlist_id = 122 WHERE playlist_id = 398;
DELETE FROM playlist_tracks WHERE playlist_id = 398;
DELETE FROM playlists WHERE id = 398;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 122), updated_at = CURRENT_TIMESTAMP WHERE id = 122;

-- ---------------------------------------------------------------------
-- Group 26/58: "d7" (Account 9, 3 playlists)
-- Primary Winner: ID 158 ('D7'), initial tracks: 414
-- ---------------------------------------------------------------------
-- Merging Loser ID 399 ('D7'): Jaccard=0.923, Containment=0.966, Extra Tracks=14
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 9512, 481, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 7715, 482, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 10495, 483, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 12146, 484, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 12148, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 12147, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 5268, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 7378, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 6768, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 11997, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 11708, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 4775, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 9327, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 1697, 494, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 399 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 158);
UPDATE playlist_sources SET playlist_id = 158 WHERE playlist_id = 399;
DELETE FROM playlist_tracks WHERE playlist_id = 399;
DELETE FROM playlists WHERE id = 399;
-- Merging Loser ID 227 ('D7'): Jaccard=0.935, Containment=0.993, Extra Tracks=3
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 2290, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 2541, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (158, 3657, 497, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 227 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 158);
UPDATE playlist_sources SET playlist_id = 158 WHERE playlist_id = 227;
DELETE FROM playlist_tracks WHERE playlist_id = 227;
DELETE FROM playlists WHERE id = 227;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 158), updated_at = CURRENT_TIMESTAMP WHERE id = 158;

-- ---------------------------------------------------------------------
-- Group 27/58: "d8" (Account 9, 3 playlists)
-- Primary Winner: ID 172 ('D8'), initial tracks: 435
-- ---------------------------------------------------------------------
-- Merging Loser ID 228 ('D8'): Jaccard=0.901, Containment=0.958, Extra Tracks=18
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 4736, 475, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 11785, 476, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 5177, 477, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 11078, 478, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 9105, 479, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 4489, 480, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 2540, 481, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 4832, 482, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 3689, 483, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 8117, 484, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 4417, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 19247, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 5081, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 19248, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 17950, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 5077, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 19249, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 2287, 492, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 228 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 172);
UPDATE playlist_sources SET playlist_id = 172 WHERE playlist_id = 228;
DELETE FROM playlist_tracks WHERE playlist_id = 228;
DELETE FROM playlists WHERE id = 228;
-- Merging Loser ID 400 ('D8'): Jaccard=0.888, Containment=0.976, Extra Tracks=10
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 12439, 493, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 5067, 494, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 11083, 495, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 9430, 496, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 10103, 497, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 7834, 498, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 7368, 499, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 23286, 500, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 9322, 501, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (172, 8332, 502, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 400 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 172);
UPDATE playlist_sources SET playlist_id = 172 WHERE playlist_id = 400;
DELETE FROM playlist_tracks WHERE playlist_id = 400;
DELETE FROM playlists WHERE id = 400;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 172), updated_at = CURRENT_TIMESTAMP WHERE id = 172;

-- ---------------------------------------------------------------------
-- Group 28/58: "d9" (Account 9, 3 playlists)
-- Primary Winner: ID 196 ('D9'), initial tracks: 437
-- ---------------------------------------------------------------------
-- Merging Loser ID 229 ('D9'): Jaccard=0.955, Containment=0.979, Extra Tracks=9
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (196, 4733, 485, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (196, 2715, 486, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (196, 3205, 487, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (196, 11742, 488, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (196, 5632, 489, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (196, 1225, 490, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (196, 2097, 491, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (196, 4868, 492, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (196, 2421, 493, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 229 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 196);
UPDATE playlist_sources SET playlist_id = 196 WHERE playlist_id = 229;
DELETE FROM playlist_tracks WHERE playlist_id = 229;
DELETE FROM playlists WHERE id = 229;
-- Merging Loser ID 401 ('D9'): Jaccard=0.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 401 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 196);
UPDATE playlist_sources SET playlist_id = 196 WHERE playlist_id = 401;
DELETE FROM playlist_tracks WHERE playlist_id = 401;
DELETE FROM playlists WHERE id = 401;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 196), updated_at = CURRENT_TIMESTAMP WHERE id = 196;

-- ---------------------------------------------------------------------
-- Group 29/58: "dcc where the f*&$ did that come from?" (Account 9, 2 playlists)
-- Primary Winner: ID 281 ('DCC Where the f*&$ did that come from?'), initial tracks: 19
-- ---------------------------------------------------------------------
-- Merging Loser ID 121 ('DCC Where the f*&$ did that come from?'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 121 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 281);
UPDATE playlist_sources SET playlist_id = 281 WHERE playlist_id = 121;
DELETE FROM playlist_tracks WHERE playlist_id = 121;
DELETE FROM playlists WHERE id = 121;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 281), updated_at = CURRENT_TIMESTAMP WHERE id = 281;

-- ---------------------------------------------------------------------
-- Group 30/58: "delusions of greatness and feeling like the protagonist" (Account 9, 2 playlists)
-- Primary Winner: ID 282 ('Delusions Of Greatness And Feeling Like The Protagonist'), initial tracks: 7
-- ---------------------------------------------------------------------
-- Merging Loser ID 119 ('Delusions Of Greatness And Feeling Like The Protagonist'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 119 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 282);
UPDATE playlist_sources SET playlist_id = 282 WHERE playlist_id = 119;
DELETE FROM playlist_tracks WHERE playlist_id = 119;
DELETE FROM playlists WHERE id = 119;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 282), updated_at = CURRENT_TIMESTAMP WHERE id = 282;

-- ---------------------------------------------------------------------
-- Group 31/58: "did i heard that before?" (Account 9, 2 playlists)
-- Primary Winner: ID 363 ('Did I Heard That Before?'), initial tracks: 32
-- ---------------------------------------------------------------------
-- Merging Loser ID 49 ('Did I Heard That Before?'): Jaccard=0.939, Containment=0.969, Extra Tracks=1
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (363, 4723, 33, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 49 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 363);
UPDATE playlist_sources SET playlist_id = 363 WHERE playlist_id = 49;
DELETE FROM playlist_tracks WHERE playlist_id = 49;
DELETE FROM playlists WHERE id = 49;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 363), updated_at = CURRENT_TIMESTAMP WHERE id = 363;

-- ---------------------------------------------------------------------
-- Group 32/58: "goth" (Account 9, 2 playlists)
-- Primary Winner: ID 248 ('Goth'), initial tracks: 368
-- ---------------------------------------------------------------------
-- Merging Loser ID 75 ('Goth'): Jaccard=0.992, Containment=0.997, Extra Tracks=1
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (248, 14395, 369, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 75 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 248);
UPDATE playlist_sources SET playlist_id = 248 WHERE playlist_id = 75;
DELETE FROM playlist_tracks WHERE playlist_id = 75;
DELETE FROM playlists WHERE id = 75;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 248), updated_at = CURRENT_TIMESTAMP WHERE id = 248;

-- ---------------------------------------------------------------------
-- Group 33/58: "green stage" (Account 9, 2 playlists)
-- Primary Winner: ID 208 ('Green Stage'), initial tracks: 34
-- ---------------------------------------------------------------------
-- Merging Loser ID 35 ('Green Stage'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 35 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 208);
UPDATE playlist_sources SET playlist_id = 208 WHERE playlist_id = 35;
DELETE FROM playlist_tracks WHERE playlist_id = 35;
DELETE FROM playlists WHERE id = 35;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 208), updated_at = CURRENT_TIMESTAMP WHERE id = 208;

-- ---------------------------------------------------------------------
-- Group 34/58: "i want to meet mikromusic" (Account 9, 2 playlists)
-- Primary Winner: ID 350 ('I want to meet Mikromusic'), initial tracks: 78
-- ---------------------------------------------------------------------
-- Merging Loser ID 47 ('I want to meet Mikromusic'): Jaccard=0.975, Containment=0.987, Extra Tracks=1
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (350, 960, 105, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 47 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 350);
UPDATE playlist_sources SET playlist_id = 350 WHERE playlist_id = 47;
DELETE FROM playlist_tracks WHERE playlist_id = 47;
DELETE FROM playlists WHERE id = 47;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 350), updated_at = CURRENT_TIMESTAMP WHERE id = 350;

-- ---------------------------------------------------------------------
-- Group 35/58: "if you're going to suicide listen to these songs." (Account 9, 3 playlists)
-- Primary Winner: ID 397 ('If you're going to suicide listen to these songs.'), initial tracks: 5
-- ---------------------------------------------------------------------
-- Merging Loser ID 386 ('If you're going to suicide listen to these songs.'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 386 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 397);
UPDATE playlist_sources SET playlist_id = 397 WHERE playlist_id = 386;
DELETE FROM playlist_tracks WHERE playlist_id = 386;
DELETE FROM playlists WHERE id = 386;
-- Merging Loser ID 51 ('If you're going to suicide listen to these songs.'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 51 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 397);
UPDATE playlist_sources SET playlist_id = 397 WHERE playlist_id = 51;
DELETE FROM playlist_tracks WHERE playlist_id = 51;
DELETE FROM playlists WHERE id = 51;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 397), updated_at = CURRENT_TIMESTAMP WHERE id = 397;

-- ---------------------------------------------------------------------
-- Group 36/58: "liked songs pt. 1" (Account 9, 2 playlists)
-- Primary Winner: ID 339 ('Liked Songs Pt. 1'), initial tracks: 1988
-- ---------------------------------------------------------------------
-- Merging Loser ID 106 ('Liked Songs Pt. 1'): Jaccard=0.959, Containment=0.980, Extra Tracks=39
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 5797, 2000, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 9486, 2001, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 9658, 2002, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 2406, 2003, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 826, 2004, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 4761, 2005, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 5778, 2006, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 1149, 2007, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 9870, 2008, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 1054, 2009, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 1152, 2010, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 735, 2011, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 2248, 2012, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 4154, 2013, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 6900, 2014, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 9641, 2015, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 2486, 2016, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 10208, 2017, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 910, 2018, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 733, 2019, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 731, 2020, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 14198, 2021, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 9271, 2022, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 14199, 2023, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 5745, 2024, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 928, 2025, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 10132, 2026, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 7353, 2027, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 169, 2028, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 8387, 2029, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 1181, 2030, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 5865, 2031, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 953, 2032, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 949, 2033, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 1047, 2034, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 734, 2035, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 812, 2036, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 838, 2037, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (339, 12475, 2038, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 106 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 339);
UPDATE playlist_sources SET playlist_id = 339 WHERE playlist_id = 106;
DELETE FROM playlist_tracks WHERE playlist_id = 106;
DELETE FROM playlists WHERE id = 106;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 339), updated_at = CURRENT_TIMESTAMP WHERE id = 339;

-- ---------------------------------------------------------------------
-- Group 37/58: "liked songs pt. 1_part2" (Account 9, 2 playlists)
-- Primary Winner: ID 338 ('Liked Songs Pt. 1_part2'), initial tracks: 1983
-- ---------------------------------------------------------------------
-- Merging Loser ID 102 ('Liked Songs Pt. 1_part2'): Jaccard=0.936, Containment=0.967, Extra Tracks=66
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 14689, 2000, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 7433, 2001, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 7567, 2002, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 11569, 2003, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 8469, 2004, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 110, 2005, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 11768, 2006, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 8322, 2007, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 741, 2008, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 12263, 2009, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 739, 2010, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 665, 2011, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 2343, 2012, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 6155, 2013, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 903, 2014, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 7225, 2015, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 7860, 2016, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 6745, 2017, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 9226, 2018, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1944, 2019, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 5646, 2020, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 781, 2021, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 680, 2022, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 7156, 2023, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1264, 2024, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 5644, 2025, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1738, 2026, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 951, 2027, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1182, 2028, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 5643, 2029, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 2688, 2030, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 7244, 2031, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 14691, 2032, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 8573, 2033, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 4079, 2034, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1337, 2035, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1735, 2036, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1184, 2037, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 6079, 2038, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 7157, 2039, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 14592, 2040, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1176, 2041, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1059, 2042, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 9142, 2043, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 11500, 2044, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 11729, 2045, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 11731, 2046, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 9007, 2047, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 12315, 2048, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 4723, 2049, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1266, 2050, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1155, 2051, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1053, 2052, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 13638, 2053, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 12096, 2054, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1057, 2055, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1877, 2056, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 1381, 2057, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 7330, 2058, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 6226, 2059, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 795, 2060, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 10127, 2061, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 7733, 2062, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 8228, 2063, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 8033, 2064, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (338, 14694, 2065, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 102 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 338);
UPDATE playlist_sources SET playlist_id = 338 WHERE playlist_id = 102;
DELETE FROM playlist_tracks WHERE playlist_id = 102;
DELETE FROM playlists WHERE id = 102;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 338), updated_at = CURRENT_TIMESTAMP WHERE id = 338;

-- ---------------------------------------------------------------------
-- Group 38/58: "liked songs pt. 1_part3" (Account 9, 2 playlists)
-- Primary Winner: ID 337 ('Liked Songs Pt. 1_part3'), initial tracks: 1994
-- ---------------------------------------------------------------------
-- Merging Loser ID 98 ('Liked Songs Pt. 1_part3'): Jaccard=0.919, Containment=0.958, Extra Tracks=84
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 3944, 2000, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 529, 2001, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 5972, 2002, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 5998, 2003, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 2449, 2004, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 4480, 2005, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 1212, 2006, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 8141, 2007, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 2228, 2008, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 2229, 2009, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 4799, 2010, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11227, 2011, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 4796, 2012, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 8080, 2013, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11228, 2014, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 4794, 2015, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 4788, 2016, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 1049, 2017, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 8000, 2018, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 7997, 2019, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 6840, 2020, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 8131, 2021, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 14680, 2022, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 6504, 2023, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 8992, 2024, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 3814, 2025, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 8419, 2026, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 14681, 2027, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 14682, 2028, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 6922, 2029, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 1157, 2030, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 2010, 2031, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 14683, 2032, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 9318, 2033, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 1046, 2034, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 5033, 2035, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 1066, 2036, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 1759, 2037, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 1154, 2038, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 114, 2039, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 5432, 2040, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 6363, 2041, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11384, 2042, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 9373, 2043, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 3532, 2044, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11407, 2045, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11408, 2046, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11410, 2047, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11412, 2048, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 8391, 2049, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 3369, 2050, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11418, 2051, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11419, 2052, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11420, 2053, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11423, 2054, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11426, 2055, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 5550, 2056, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 5087, 2057, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 2251, 2058, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 14688, 2059, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11532, 2060, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 726, 2061, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11297, 2062, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11575, 2063, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 8991, 2064, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 852, 2065, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 748, 2066, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 830, 2067, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 943, 2068, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 1273, 2069, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 4805, 2070, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 2103, 2071, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 840, 2072, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11856, 2073, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11894, 2074, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11921, 2075, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 7454, 2076, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 2649, 2077, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 11988, 2078, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 4589, 2079, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 10361, 2080, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 1224, 2081, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 6536, 2082, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (337, 1325, 2083, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 98 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 337);
UPDATE playlist_sources SET playlist_id = 337 WHERE playlist_id = 98;
DELETE FROM playlist_tracks WHERE playlist_id = 98;
DELETE FROM playlists WHERE id = 98;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 337), updated_at = CURRENT_TIMESTAMP WHERE id = 337;

-- ---------------------------------------------------------------------
-- Group 39/58: "liked songs pt. 1_part4" (Account 9, 2 playlists)
-- Primary Winner: ID 323 ('Liked Songs Pt. 1_part4'), initial tracks: 1987
-- ---------------------------------------------------------------------
-- Merging Loser ID 90 ('Liked Songs Pt. 1_part4'): Jaccard=0.924, Containment=0.961, Extra Tracks=78
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 5805, 2000, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 5363, 2001, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 377, 2002, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 5360, 2003, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 3330, 2004, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 8413, 2005, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 8406, 2006, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 6959, 2007, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 6704, 2008, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 196, 2009, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 7218, 2010, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 4968, 2011, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 832, 2012, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1081, 2013, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1079, 2014, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 6787, 2015, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 3298, 2016, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 3310, 2017, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1074, 2018, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 12186, 2019, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 917, 2020, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 818, 2021, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 939, 2022, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 797, 2023, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 937, 2024, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 743, 2025, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 955, 2026, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 909, 2027, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 6514, 2028, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 8388, 2029, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 9524, 2030, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 9346, 2031, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1069, 2032, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 962, 2033, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 14677, 2034, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1052, 2035, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 880, 2036, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 3378, 2037, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 9697, 2038, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 859, 2039, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1064, 2040, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 504, 2041, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 12256, 2042, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 960, 2043, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 6707, 2044, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 930, 2045, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 14161, 2046, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 12486, 2047, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 822, 2048, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 793, 2049, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 3781, 2050, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 12567, 2051, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1048, 2052, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 418, 2053, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1865, 2054, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 7254, 2055, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1050, 2056, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 8510, 2057, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1038, 2058, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 673, 2059, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 6700, 2060, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1035, 2061, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1174, 2062, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 6898, 2063, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 847, 2064, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 11644, 2065, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 11703, 2066, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1267, 2067, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 7746, 2068, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1269, 2069, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 11673, 2070, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 905, 2071, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 907, 2072, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1062, 2073, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1063, 2074, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 7499, 2075, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1132, 2076, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (323, 1060, 2077, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 90 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 323);
UPDATE playlist_sources SET playlist_id = 323 WHERE playlist_id = 90;
DELETE FROM playlist_tracks WHERE playlist_id = 90;
DELETE FROM playlists WHERE id = 90;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 323), updated_at = CURRENT_TIMESTAMP WHERE id = 323;

-- ---------------------------------------------------------------------
-- Group 40/58: "liked songs pt. 1_part5" (Account 9, 2 playlists)
-- Primary Winner: ID 79 ('Liked Songs Pt. 1_part5'), initial tracks: 1479
-- ---------------------------------------------------------------------
-- Merging Loser ID 313 ('Liked Songs Pt. 1_part5'): Jaccard=0.946, Containment=0.981, Extra Tracks=27
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 12247, 1522, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 3747, 1523, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 2225, 1524, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 19271, 1525, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 11874, 1526, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 8545, 1527, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 2566, 1528, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 3881, 1529, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 3067, 1530, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 581, 1531, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 5731, 1532, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 4142, 1533, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 10451, 1534, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 3403, 1535, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 1962, 1536, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 1963, 1537, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 1969, 1538, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 2382, 1539, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 6437, 1540, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 5162, 1541, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 2735, 1542, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 6542, 1543, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 2096, 1544, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 6545, 1545, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 2103, 1546, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 2106, 1547, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (79, 3812, 1548, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 313 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 79);
UPDATE playlist_sources SET playlist_id = 79 WHERE playlist_id = 313;
DELETE FROM playlist_tracks WHERE playlist_id = 313;
DELETE FROM playlists WHERE id = 313;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 79), updated_at = CURRENT_TIMESTAMP WHERE id = 79;

-- ---------------------------------------------------------------------
-- Group 41/58: "liked songs pt. 2" (Account 9, 2 playlists)
-- Primary Winner: ID 64 ('Liked Songs Pt. 2'), initial tracks: 504
-- ---------------------------------------------------------------------
-- Merging Loser ID 272 ('Liked Songs Pt. 2'): Jaccard=0.951, Containment=0.978, Extra Tracks=11
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (64, 19271, 505, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (64, 11874, 506, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (64, 8545, 507, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (64, 2566, 508, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (64, 3881, 509, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (64, 3067, 510, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (64, 581, 511, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (64, 5731, 512, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (64, 4142, 513, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (64, 10451, 514, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (64, 3403, 515, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 272 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 64);
UPDATE playlist_sources SET playlist_id = 64 WHERE playlist_id = 272;
DELETE FROM playlist_tracks WHERE playlist_id = 272;
DELETE FROM playlists WHERE id = 272;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 64), updated_at = CURRENT_TIMESTAMP WHERE id = 64;

-- ---------------------------------------------------------------------
-- Group 42/58: "los ladrones de Ámsterdam" (Account 9, 2 playlists)
-- Primary Winner: ID 286 ('Los Ladrones De Ámsterdam'), initial tracks: 85
-- ---------------------------------------------------------------------
-- Merging Loser ID 78 ('Los Ladrones De Ámsterdam'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 78 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 286);
UPDATE playlist_sources SET playlist_id = 286 WHERE playlist_id = 78;
DELETE FROM playlist_tracks WHERE playlist_id = 78;
DELETE FROM playlists WHERE id = 78;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 286), updated_at = CURRENT_TIMESTAMP WHERE id = 286;

-- ---------------------------------------------------------------------
-- Group 43/58: "miguel j.luis" (Account 9, 3 playlists)
-- Primary Winner: ID 215 ('Miguel J.Luis'), initial tracks: 48
-- ---------------------------------------------------------------------
-- Merging Loser ID 133 ('Miguel J.Luis'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 133 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 215);
UPDATE playlist_sources SET playlist_id = 215 WHERE playlist_id = 133;
DELETE FROM playlist_tracks WHERE playlist_id = 133;
DELETE FROM playlists WHERE id = 133;
-- Merging Loser ID 132 ('Miguel J.Luis'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 132 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 215);
UPDATE playlist_sources SET playlist_id = 215 WHERE playlist_id = 132;
DELETE FROM playlist_tracks WHERE playlist_id = 132;
DELETE FROM playlists WHERE id = 132;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 215), updated_at = CURRENT_TIMESTAMP WHERE id = 215;

-- ---------------------------------------------------------------------
-- Group 44/58: "missing from local collection (2025-06-02)" (Account 9, 2 playlists)
-- Primary Winner: ID 193 ('Missing From Local Collection (2025-06-02)'), initial tracks: 318
-- ---------------------------------------------------------------------
-- Merging Loser ID 216 ('Missing From Local Collection (2025-06-02)'): Jaccard=0.859, Containment=0.969, Extra Tracks=9
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (193, 2421, 335, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (193, 3196, 336, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (193, 3563, 337, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (193, 10018, 338, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (193, 11078, 339, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (193, 18631, 340, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (193, 11997, 341, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (193, 18632, 342, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (193, 11874, 343, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 216 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 193);
UPDATE playlist_sources SET playlist_id = 193 WHERE playlist_id = 216;
DELETE FROM playlist_tracks WHERE playlist_id = 216;
DELETE FROM playlists WHERE id = 216;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 193), updated_at = CURRENT_TIMESTAMP WHERE id = 193;

-- ---------------------------------------------------------------------
-- Group 45/58: "my playlist #44" (Account 9, 2 playlists)
-- Primary Winner: ID 254 ('My playlist #44'), initial tracks: 29
-- ---------------------------------------------------------------------
-- Merging Loser ID 66 ('My playlist #44'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 66 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 254);
UPDATE playlist_sources SET playlist_id = 254 WHERE playlist_id = 66;
DELETE FROM playlist_tracks WHERE playlist_id = 66;
DELETE FROM playlists WHERE id = 66;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 254), updated_at = CURRENT_TIMESTAMP WHERE id = 254;

-- ---------------------------------------------------------------------
-- Group 46/58: "new favs" (Account 9, 2 playlists)
-- Primary Winner: ID 367 ('New favs'), initial tracks: 14
-- ---------------------------------------------------------------------
-- Merging Loser ID 52 ('New favs'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 52 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 367);
UPDATE playlist_sources SET playlist_id = 367 WHERE playlist_id = 52;
DELETE FROM playlist_tracks WHERE playlist_id = 52;
DELETE FROM playlists WHERE id = 52;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 367), updated_at = CURRENT_TIMESTAMP WHERE id = 367;

-- ---------------------------------------------------------------------
-- Group 47/58: "niu radio" (Account 9, 2 playlists)
-- Primary Winner: ID 390 ('Niu Radio'), initial tracks: 130
-- ---------------------------------------------------------------------
-- Merging Loser ID 42 ('Niu Radio'): Jaccard=0.940, Containment=0.969, Extra Tracks=4
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (390, 13638, 131, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (390, 7156, 132, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (390, 9996, 133, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (390, 7173, 134, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 42 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 390);
UPDATE playlist_sources SET playlist_id = 390 WHERE playlist_id = 42;
DELETE FROM playlist_tracks WHERE playlist_id = 42;
DELETE FROM playlists WHERE id = 42;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 390), updated_at = CURRENT_TIMESTAMP WHERE id = 390;

-- ---------------------------------------------------------------------
-- Group 48/58: "no alternative - v.a." (Account 9, 2 playlists)
-- Primary Winner: ID 41 ('No Alternative - V.A.'), initial tracks: 15
-- ---------------------------------------------------------------------
-- Merging Loser ID 211 ('No Alternative - V.A.'): Jaccard=0.750, Containment=0.923, Extra Tracks=1
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (41, 12898, 16, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 211 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 41);
UPDATE playlist_sources SET playlist_id = 41 WHERE playlist_id = 211;
DELETE FROM playlist_tracks WHERE playlist_id = 211;
DELETE FROM playlists WHERE id = 211;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 41), updated_at = CURRENT_TIMESTAMP WHERE id = 41;

-- ---------------------------------------------------------------------
-- Group 49/58: "polish←(*꒪ヮ꒪*)" (Account 9, 2 playlists)
-- Primary Winner: ID 368 ('Polish←(*꒪ヮ꒪*)'), initial tracks: 55
-- ---------------------------------------------------------------------
-- Merging Loser ID 55 ('Polish←(*꒪ヮ꒪*)'): Jaccard=0.982, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 55 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 368);
UPDATE playlist_sources SET playlist_id = 368 WHERE playlist_id = 55;
DELETE FROM playlist_tracks WHERE playlist_id = 55;
DELETE FROM playlists WHERE id = 55;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 368), updated_at = CURRENT_TIMESTAMP WHERE id = 368;

-- ---------------------------------------------------------------------
-- Group 50/58: "ready player one" (Account 9, 2 playlists)
-- Primary Winner: ID 39 ('Ready Player One'), initial tracks: 23
-- ---------------------------------------------------------------------
-- Merging Loser ID 388 ('Ready Player One'): Jaccard=0.833, Containment=0.952, Extra Tracks=1
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (39, 18778, 24, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 388 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 39);
UPDATE playlist_sources SET playlist_id = 39 WHERE playlist_id = 388;
DELETE FROM playlist_tracks WHERE playlist_id = 388;
DELETE FROM playlists WHERE id = 388;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 39), updated_at = CURRENT_TIMESTAMP WHERE id = 39;

-- ---------------------------------------------------------------------
-- Group 51/58: "red stage" (Account 9, 2 playlists)
-- Primary Winner: ID 204 ('Red Stage'), initial tracks: 30
-- ---------------------------------------------------------------------
-- Merging Loser ID 36 ('Red Stage'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 36 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 204);
UPDATE playlist_sources SET playlist_id = 204 WHERE playlist_id = 36;
DELETE FROM playlist_tracks WHERE playlist_id = 36;
DELETE FROM playlists WHERE id = 36;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 204), updated_at = CURRENT_TIMESTAMP WHERE id = 204;

-- ---------------------------------------------------------------------
-- Group 52/58: "songs not in my local library" (Account 9, 2 playlists)
-- Primary Winner: ID 142 ('Songs Not In My Local Library'), initial tracks: 712
-- ---------------------------------------------------------------------
-- Merging Loser ID 218 ('Songs Not In My Local Library'): Jaccard=0.848, Containment=0.942, Extra Tracks=39
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 5162, 789, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 2290, 790, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 2541, 791, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 2552, 792, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 2566, 793, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 5866, 794, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 3067, 795, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 3196, 796, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 3200, 797, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 8545, 798, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 3563, 799, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 10018, 800, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 3686, 801, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 9405, 802, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 4142, 803, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 4290, 804, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 4505, 805, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 4656, 806, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 11079, 807, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 4726, 808, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 4733, 809, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 4736, 810, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 10020, 811, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 4748, 812, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 4751, 813, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 10565, 814, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 4832, 815, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 4839, 816, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 4864, 817, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 4868, 818, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 11299, 819, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 11078, 820, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 5196, 821, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 18631, 822, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 12148, 823, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 18767, 824, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 11785, 825, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 11997, 826, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (142, 18632, 827, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 218 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 142);
UPDATE playlist_sources SET playlist_id = 142 WHERE playlist_id = 218;
DELETE FROM playlist_tracks WHERE playlist_id = 218;
DELETE FROM playlists WHERE id = 218;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 142), updated_at = CURRENT_TIMESTAMP WHERE id = 142;

-- ---------------------------------------------------------------------
-- Group 53/58: "songs that i awake to" (Account 9, 2 playlists)
-- Primary Winner: ID 353 ('Songs that I awake to'), initial tracks: 70
-- ---------------------------------------------------------------------
-- Merging Loser ID 34 ('Songs that I awake to'): Jaccard=0.972, Containment=0.986, Extra Tracks=1
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (353, 2251, 71, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 34 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 353);
UPDATE playlist_sources SET playlist_id = 353 WHERE playlist_id = 34;
DELETE FROM playlist_tracks WHERE playlist_id = 34;
DELETE FROM playlists WHERE id = 34;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 353), updated_at = CURRENT_TIMESTAMP WHERE id = 353;

-- ---------------------------------------------------------------------
-- Group 54/58: "sun" (Account 9, 2 playlists)
-- Primary Winner: ID 263 ('SUN'), initial tracks: 36
-- ---------------------------------------------------------------------
-- Merging Loser ID 120 ('SUN'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 120 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 263);
UPDATE playlist_sources SET playlist_id = 263 WHERE playlist_id = 120;
DELETE FROM playlist_tracks WHERE playlist_id = 120;
DELETE FROM playlists WHERE id = 120;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 263), updated_at = CURRENT_TIMESTAMP WHERE id = 263;

-- ---------------------------------------------------------------------
-- Group 55/58: "swipefy" (Account 9, 2 playlists)
-- Primary Winner: ID 219 ('Swipefy'), initial tracks: 40
-- ---------------------------------------------------------------------
-- Merging Loser ID 170 ('Swipefy'): Jaccard=0.975, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 170 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 219);
UPDATE playlist_sources SET playlist_id = 219 WHERE playlist_id = 170;
DELETE FROM playlist_tracks WHERE playlist_id = 170;
DELETE FROM playlists WHERE id = 170;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 219), updated_at = CURRENT_TIMESTAMP WHERE id = 219;

-- ---------------------------------------------------------------------
-- Group 56/58: "tcb" (Account 9, 2 playlists)
-- Primary Winner: ID 252 ('TCB'), initial tracks: 137
-- ---------------------------------------------------------------------
-- Merging Loser ID 116 ('TCB'): Jaccard=1.000, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 116 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 252);
UPDATE playlist_sources SET playlist_id = 252 WHERE playlist_id = 116;
DELETE FROM playlist_tracks WHERE playlist_id = 116;
DELETE FROM playlists WHERE id = 116;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 252), updated_at = CURRENT_TIMESTAMP WHERE id = 252;

-- ---------------------------------------------------------------------
-- Group 57/58: "walking like a badass" (Account 9, 2 playlists)
-- Primary Winner: ID 391 ('Walking Like A Badass'), initial tracks: 59
-- ---------------------------------------------------------------------
-- Merging Loser ID 44 ('Walking Like A Badass'): Jaccard=0.903, Containment=0.949, Extra Tracks=3
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (391, 169, 60, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (391, 6028, 61, NULL);
INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (391, 1083, 62, NULL);
DELETE FROM playlist_sources WHERE playlist_id = 44 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 391);
UPDATE playlist_sources SET playlist_id = 391 WHERE playlist_id = 44;
DELETE FROM playlist_tracks WHERE playlist_id = 44;
DELETE FROM playlists WHERE id = 44;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 391), updated_at = CURRENT_TIMESTAMP WHERE id = 391;

-- ---------------------------------------------------------------------
-- Group 58/58: "white stage" (Account 9, 2 playlists)
-- Primary Winner: ID 38 ('White Stage'), initial tracks: 29
-- ---------------------------------------------------------------------
-- Merging Loser ID 205 ('White Stage'): Jaccard=0.966, Containment=1.000, Extra Tracks=0
DELETE FROM playlist_sources WHERE playlist_id = 205 AND (account_id, service_playlist_id) IN (SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = 38);
UPDATE playlist_sources SET playlist_id = 38 WHERE playlist_id = 205;
DELETE FROM playlist_tracks WHERE playlist_id = 205;
DELETE FROM playlists WHERE id = 205;
UPDATE playlists SET track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = 38), updated_at = CURRENT_TIMESTAMP WHERE id = 38;

COMMIT;

PRAGMA foreign_key_check;
