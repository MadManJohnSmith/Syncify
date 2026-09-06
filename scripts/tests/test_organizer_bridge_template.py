#!/usr/bin/env python3
"""
Regression tests for safe template formatting in scripts/organizer_bridge.py (TASK-48).

Verifies that:
1. Missing keys in templates do not raise KeyError.
2. Unbalanced or malformed braces do not raise ValueError.
3. Tags present in the dictionary are formatted accurately.
4. Illegal filesystem characters are replaced in path components.
5. format_path integrates safely without crashing library reorganization.
"""

import sys
import unittest
from pathlib import Path

# Add scripts directory to module search path
SCRIPTS_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS_DIR))

from organizer_bridge import (
    SafeTemplateFormatter,
    safe_format_template,
    format_path,
    sanitize_filename,
)


class TestOrganizerBridgeTemplate(unittest.TestCase):
    """Test suite for safe template formatting in organizer_bridge.py."""

    def setUp(self):
        self.full_tags = {
            "artist": "Pink Floyd",
            "album_artist": "Pink Floyd",
            "album": "The Wall",
            "title": "Comfortably Numb",
            "genre": "Progressive Rock",
            "year": 1979,
            "track": 6,
            "disc": 2,
        }

    def test_correct_formatting_present_tags(self):
        """Verify that standard templates format accurately when all tags are present."""
        template = "{artist}/{album}/{track:02d} - {title}"
        result = safe_format_template(template, self.full_tags)
        self.assertEqual(result, "Pink Floyd/The Wall/06 - Comfortably Numb")

        # Test disc and track format
        disc_template = "{album_artist}/{album}/{disc:02d}-{track:02d} {title}"
        disc_result = safe_format_template(disc_template, self.full_tags)
        self.assertEqual(disc_result, "Pink Floyd/The Wall/02-06 Comfortably Numb")

        # Test genre pattern
        genre_template = "{genre}/{artist}/{album}/{title}"
        genre_result = safe_format_template(genre_template, self.full_tags)
        self.assertEqual(genre_result, "Progressive Rock/Pink Floyd/The Wall/Comfortably Numb")

    def test_missing_keys_no_keyerror(self):
        """Verify that missing keys (like bitrate, disc, comment) do not raise KeyError."""
        partial_tags = {
            "artist": "Led Zeppelin",
            "title": "Kashmir",
        }

        # Template contains fields not in tags
        template = "{artist}/{album}/{track:02d} - {title} [{bitrate}]"
        try:
            result = safe_format_template(template, partial_tags)
        except KeyError as exc:
            self.fail(f"safe_format_template raised KeyError unexpectedly: {exc}")

        # Missing keys should be replaced safely without crashing
        self.assertIn("Led Zeppelin", result)
        self.assertIn("Kashmir", result)
        self.assertNotIn("KeyError", result)

    def test_unbalanced_braces_no_valueerror(self):
        """Verify that syntax errors with unbalanced braces do not raise ValueError."""
        test_patterns = [
            "{artist - {title}",         # Stray opening brace
            "{artist}/{album/{title}",   # Nested / unclosed brace
            "{artist}/{title}}",         # Stray closing brace
            "{artist",                   # Unclosed brace at end
            "artist}",                   # Unopened brace at end
            "{{{{bad_template",          # Pathological braces
            "}{}{",                      # Inverse braces
        ]

        tags = {"artist": "Queen", "album": "Sheer Heart Attack", "title": "Killer Queen"}

        for pattern in test_patterns:
            try:
                result = safe_format_template(pattern, tags, fallback_filename="fallback_track")
            except ValueError as exc:
                self.fail(f"Pattern '{pattern}' raised ValueError unexpectedly: {exc}")
            except Exception as exc:
                self.fail(f"Pattern '{pattern}' raised {type(exc).__name__} unexpectedly: {exc}")

            self.assertIsInstance(result, str)
            self.assertTrue(len(result) > 0)

    def test_filesystem_illegal_characters_replaced(self):
        """Verify that illegal filesystem characters (<>:"/\\|?*) in components are sanitized."""
        dirty_tags = {
            "artist": "AC/DC",
            "album": 'What: Ever? *Special* <Deluxe> "Edition"',
            "title": "Track | 01",
            "track": 1,
        }

        template = "{artist}/{album}/{track:02d} - {title}"
        result = safe_format_template(template, dirty_tags)

        # Result components must not contain any illegal characters
        components = result.split("/")
        self.assertEqual(components[0], "AC_DC")
        self.assertNotIn(":", components[1])
        self.assertNotIn("?", components[1])
        self.assertNotIn("*", components[1])
        self.assertNotIn("<", components[1])
        self.assertNotIn(">", components[1])
        self.assertNotIn('"', components[1])
        self.assertNotIn("|", components[2])

    def test_illegal_characters_in_template_literals(self):
        """Verify that illegal characters in static parts of template are also sanitized."""
        template = "Genre: Rock*/{artist}/{title}"
        result = safe_format_template(template, {"artist": "Queen", "title": "Radio Ga Ga"})
        self.assertNotIn(":", result)
        self.assertNotIn("*", result)
        self.assertTrue(result.startswith("Genre_ Rock_/Queen/Radio Ga Ga"))

    def test_safe_template_formatter_standalone(self):
        """Verify SafeTemplateFormatter methods directly."""
        formatter = SafeTemplateFormatter(default="")

        # Missing kwargs return default without KeyError
        rendered = formatter.format("Hello {missing}", known="world")
        self.assertEqual(rendered, "Hello ")

        # Bad format specifier on string doesn't raise ValueError
        bad_spec = formatter.format("{track:02d}", track="")
        self.assertEqual(bad_spec, "")

        # Numeric string converted when possible
        num_spec = formatter.format("{track:02d}", track="7")
        self.assertEqual(num_spec, "07")

        # None value handled cleanly
        none_spec = formatter.format("{artist}", artist=None)
        self.assertEqual(none_spec, "")

    def test_format_path_integration(self):
        """Verify format_path integration with file_path and file extension."""
        audio_file = Path("/tmp/incoming/song.flac")

        # Test missing tags in format_path
        pattern = "{artist}/{album}/{track:02d} - {title} [{bitrate}]"
        tags = {"artist": "Nirvana", "title": "Smells Like Teen Spirit"}
        res = format_path(pattern, tags, audio_file)

        self.assertTrue(res.endswith(".flac"))
        self.assertIn("Nirvana", res)
        self.assertIn("Smells Like Teen Spirit", res)

        # Test broken pattern in format_path falls back safely
        broken_pattern = "{artist/{album/broken"
        fallback_res = format_path(broken_pattern, tags, audio_file)
        self.assertTrue(fallback_res.endswith(".flac"))


if __name__ == "__main__":
    unittest.main()
