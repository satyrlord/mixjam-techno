"""Offline checks for the Album Review lookup tool. They make no network request."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("lookup", ROOT / "tools/album-review/lookup.py")
lookup = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(lookup)


class LookupTests(unittest.TestCase):
    def test_missing_track_lengths_do_not_produce_a_partial_album_runtime(self):
        release = {"media": [{"tracks": [
            {"title": "Known", "length": 60000},
            {"title": "Unknown"},
        ]}]}
        with patch.object(lookup, "musicbrainz", return_value=release):
            result = lookup.tracks("fixture")
        self.assertEqual(result["tracks"][1]["duration"], "?:??")
        self.assertEqual(result["runtime"], "?:??")

    def test_complete_track_lengths_produce_the_album_runtime(self):
        release = {"media": [{"tracks": [
            {"title": "First", "length": 60000},
        ]}, {"tracks": [{"title": "Second", "length": 120000}]}]}
        with patch.object(lookup, "musicbrainz", return_value=release):
            result = lookup.tracks("fixture")
        self.assertEqual(result["runtime"], "3:00")
        self.assertEqual([track["num"] for track in result["tracks"]], [1, 2])

    def test_durations_use_the_review_formats(self):
        self.assertEqual(lookup.clock(0), "0:00")
        self.assertEqual(lookup.clock(754), "12:34")
        self.assertEqual(lookup.runtime(3599), "59:59")
        self.assertEqual(lookup.runtime(3933), "1:05:33")

    def test_lucene_phrases_escape_quotes_and_backslashes(self):
        self.assertEqual(lookup.phrase('Say "Hi" \\ now'), '"Say \\"Hi\\" \\\\ now"')

    def test_cover_links_lose_tracking_parameters_only(self):
        self.assertEqual(
            lookup.clean_url("https://upload.wikimedia.org/a/Cover.jpg?utm_source=x&utm_campaign=y"),
            "https://upload.wikimedia.org/a/Cover.jpg",
        )
        self.assertEqual(
            lookup.clean_url("https://example.com/c.jpg?size=300&utm_content=z"),
            "https://example.com/c.jpg?size=300",
        )

    def test_an_article_counts_only_when_it_names_the_artist_and_a_record(self):
        album = {"description": "1976 studio album by Jean-Michel Jarre"}
        self.assertTrue(lookup.describes_album(album, "Jean-Michel Jarre"))
        self.assertTrue(lookup.describes_album(
            {"extract": "Possible Musics is an album by Jon Hassell and Brian Eno."},
            "Jon Hassell & Brian Eno"))
        self.assertFalse(lookup.describes_album({"description": "French musician"}, "Jean-Michel Jarre"))
        self.assertFalse(lookup.describes_album({"description": "1990 album by Enigma"}, "Vangelis"))

    def test_page_scores_prefer_the_album_article(self):
        exact = lookup.page_score("Oxygène (album)", "Oxygène", "Jean-Michel Jarre")
        song = lookup.page_score("Oxygène (song)", "Oxygène", "Jean-Michel Jarre")
        other = lookup.page_score("Jean-Michel Jarre discography", "Oxygène", "Jean-Michel Jarre")
        self.assertGreater(exact, song)
        self.assertGreater(exact, other)
        self.assertEqual(lookup.fold("Oxygène"), "oxygene")

    def test_the_user_agent_names_the_application(self):
        self.assertIn("MixJamTechno", lookup.USER_AGENT)


if __name__ == "__main__":
    unittest.main()
