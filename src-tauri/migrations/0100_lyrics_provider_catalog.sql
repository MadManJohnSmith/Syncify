-- Stable source variants. Preserve user choices on existing rows; retire Genius (no fetcher).
UPDATE lyrics_provider_settings SET provider_id = 'lrclib_synced', provider_name = 'LRCLIB (synced)', sync_level = 'line' WHERE provider_id = 'lrclib';
UPDATE lyrics_provider_settings SET provider_id = 'netease_synced', provider_name = 'NetEase (synced)', sync_level = 'line' WHERE provider_id = 'netease';
UPDATE lyrics_provider_settings SET provider_id = 'apple_music_ttml', provider_name = 'Apple Music TTML', sync_level = 'syllable' WHERE provider_id = 'apple_music';
DELETE FROM lyrics_provider_settings WHERE provider_id = 'genius';
INSERT OR IGNORE INTO lyrics_provider_settings (provider_id, provider_name, priority, sync_level) VALUES
 ('spotify_color', 'Spotify Color Lyrics', 5, 'word'),
 ('musixmatch_richsync', 'Musixmatch Richsync', 6, 'word'),
 ('ultrastar', 'UltraStar Karaoke', 7, 'syllable'),
 ('kugou', 'Kugou Karaoke', 8, 'word'),
 ('qqmusic', 'QQ Music', 9, 'line'),
 ('lyricsplus', 'LyricsPlus', 10, 'line'),
 ('musixmatch_plain', 'Musixmatch (plain)', 11, 'none'),
 ('lrclib_plain', 'LRCLIB (plain)', 12, 'none'),
 ('tekstowo_plain', 'Tekstowo (plain)', 13, 'none');
