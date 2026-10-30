"""Look up album metadata for an Album Review: lookup.py release | tracks | cover

python -I tools/album-review/lookup.py release "ARTIST" "TITLE" [--year YEAR]
python -I tools/album-review/lookup.py tracks MBID
python -I tools/album-review/lookup.py cover "ARTIST" "TITLE" [--page "ARTICLE"]

`release` searches MusicBrainz for the release and prints the candidates as JSON, an official
release first. `tracks` prints the track list of one release (every medium, numbered from 1),
with each duration as M:SS, the runtime, and the release date. A missing duration or incomplete
runtime reads ?:??. `cover` searches Wikipedia for the
album's article and prints the cover image URL and the article URL, for the review's `coverUrl`
and `coverSource`. It accepts only an article whose summary names the artist and calls the work an
album, a soundtrack, or a score, and prints empty strings when none has an image; `--page` reads
one article that you name. Check the image before you use it: a search can still pick the wrong
record.

The tools read metadata only. They never download a cover: the application does that when the
user asks for it. MusicBrainz allows one request a second, so `release` and `tracks` wait between
requests. Both services require a descriptive user agent; this tool sends one.
"""
import argparse
import json
import sys
import time
import unicodedata
import urllib.error
import urllib.parse
import urllib.request

USER_AGENT = "MixJamTechnoAlbumReview/1.0 (https://github.com/satyrlord/mixjam-techno)"
MUSICBRAINZ = "https://musicbrainz.org/ws/2"
WIKI_API = "https://en.wikipedia.org/w/api.php"
WIKI_SUMMARY = "https://en.wikipedia.org/api/rest_v1/page/summary/"
TIMEOUT = 20
_last_musicbrainz = 0.0


def get_json(url):
    request = urllib.request.Request(
        url, headers={"User-Agent": USER_AGENT, "Accept": "application/json"}
    )
    with urllib.request.urlopen(request, timeout=TIMEOUT) as response:
        return json.loads(response.read().decode("utf-8"))


def musicbrainz(path, **params):
    global _last_musicbrainz
    wait = 1.1 - (time.monotonic() - _last_musicbrainz)
    if wait > 0:
        time.sleep(wait)
    _last_musicbrainz = time.monotonic()
    params["fmt"] = "json"
    return get_json(f"{MUSICBRAINZ}/{path}?{urllib.parse.urlencode(params)}")


def phrase(value):
    """A Lucene phrase: quote the value and escape quotes and backslashes in it."""
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def clock(seconds):
    return f"{seconds // 60}:{seconds % 60:02d}"


def runtime(seconds):
    if seconds < 3600:
        return clock(seconds)
    return f"{seconds // 3600}:{clock(seconds % 3600).zfill(5)}"


def releases(artist, title, year):
    query = f"release:{phrase(title)} AND artist:{phrase(artist)}"
    if year:
        query += f" AND date:{year}"
    found = musicbrainz("release", query=query, limit="8").get("releases", [])
    found.sort(key=lambda release: (release.get("status", "").lower() != "official"))
    return [
        {
            "mbid": release["id"],
            "title": release.get("title", ""),
            "artist": "".join(
                credit.get("name", "") + credit.get("joinphrase", "")
                for credit in release.get("artist-credit", [])
            ),
            "date": release.get("date", ""),
            "status": release.get("status", ""),
            "country": release.get("country", ""),
            "tracks": release.get("track-count", 0),
        }
        for release in found
    ]


def tracks(mbid):
    data = musicbrainz(f"release/{mbid}", inc="recordings")
    listed = []
    total = 0
    for medium in data.get("media", []):
        for track in medium.get("tracks", []):
            milliseconds = track.get("length")
            seconds = round(milliseconds / 1000) if milliseconds else None
            total += seconds or 0
            listed.append(
                {
                    "num": len(listed) + 1,
                    "title": track.get("title", "Unknown"),
                    "duration": clock(seconds) if seconds is not None else "?:??",
                }
            )
    return {
        "mbid": mbid,
        "title": data.get("title", ""),
        "date": data.get("date", ""),
        "runtime": runtime(total) if listed and all(
            track["duration"] != "?:??" for track in listed
        ) else "?:??",
        "tracks": listed,
    }


def fold(value):
    """Lowercase without accents, for comparing titles."""
    decomposed = unicodedata.normalize("NFD", value)
    return "".join(c for c in decomposed if not unicodedata.combining(c)).lower()


def page_score(page, title, artist):
    page, album, artist = fold(page), fold(title), fold(artist)
    score = 0
    score += 10 if page == album else 0
    score += 6 if album in page else 0
    score += 4 if artist in page else 0
    score += 3 if "album" in page else 0
    score -= 6 if "song" in page else 0
    score -= 8 if "disambiguation" in page else 0
    return score


def wiki_search(query):
    params = urllib.parse.urlencode(
        {"action": "query", "format": "json", "list": "search", "srsearch": query,
         "srlimit": "5", "srnamespace": "0"}
    )
    try:
        data = get_json(f"{WIKI_API}?{params}")
    except (urllib.error.URLError, TimeoutError, ValueError):
        return []
    return [result["title"] for result in data.get("query", {}).get("search", [])]


def wiki_summary(page):
    try:
        return get_json(WIKI_SUMMARY + urllib.parse.quote(page.replace(" ", "_"), safe=""))
    except (urllib.error.URLError, TimeoutError, ValueError):
        return None


def clean_url(url):
    """The address without the tracking parameters that the summary API adds."""
    parts = urllib.parse.urlsplit(url)
    query = [(key, value) for key, value in urllib.parse.parse_qsl(parts.query)
             if not key.startswith("utm_")]
    return urllib.parse.urlunsplit(parts._replace(query=urllib.parse.urlencode(query)))


def describes_album(summary, artist):
    """Whether a page summary is about a record by the artist."""
    text = fold(" ".join(str(summary.get(key, "")) for key in ("description", "extract")))
    names = [fold(part).strip() for part in artist.replace("&", "/").replace(" and ", "/").split("/")]
    record = any(word in text for word in ("album", "soundtrack", "score", "record", "suite"))
    return record and any(name and name in text for name in names)


def page_cover(page):
    """The cover image and address of one Wikipedia page, or None."""
    summary = wiki_summary(page)
    if not summary or summary.get("type") == "disambiguation":
        return None
    image = (summary.get("thumbnail") or {}).get("source") or (
        summary.get("originalimage") or {}
    ).get("source")
    if not image:
        return None
    source = (
        summary.get("content_urls", {}).get("desktop", {}).get("page")
        or "https://en.wikipedia.org/wiki/" + urllib.parse.quote(page.replace(" ", "_"))
    )
    return summary, {"coverUrl": clean_url(image), "coverSource": source,
                     "page": summary.get("title", page)}


def cover(artist, title, page=None):
    """The cover image of the album's Wikipedia article, and the article URL.

    With `page`, read that article. Otherwise search, and accept only an article whose summary
    names the artist and calls the work an album, a soundtrack, or a score.
    """
    empty = {"coverUrl": "", "coverSource": "", "page": ""}
    if page:
        found = page_cover(page)
        return found[1] if found else empty
    candidates = {}

    def add(page, bonus=0):
        score = page_score(page, title, artist) + bonus
        candidates[page] = max(score, candidates.get(page, score))

    for page in (title, f"{title} (album)", f"{title} ({artist} album)",
                 f"{title} (soundtrack)"):
        add(page, 2)
    for query in (f"{artist} {title} album", f'"{artist}" "{title}" album',
                  f'"{title}" album', f"{title} {artist}"):
        for page in wiki_search(query):
            add(page)
    ordered = sorted(candidates, key=lambda page: -candidates[page])[:8]
    for page in ordered:
        found = page_cover(page)
        if found and describes_album(found[0], artist):
            return found[1]
    return empty


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    release = commands.add_parser("release", help="search MusicBrainz for a release")
    release.add_argument("artist")
    release.add_argument("title")
    release.add_argument("--year", type=int)
    listing = commands.add_parser("tracks", help="the track list of a MusicBrainz release")
    listing.add_argument("mbid")
    art = commands.add_parser("cover", help="the Wikipedia cover URL of an album")
    art.add_argument("artist")
    art.add_argument("title")
    art.add_argument("--page", help="read this Wikipedia article instead of searching")
    arguments = parser.parse_args(argv)
    try:
        if arguments.command == "release":
            result = releases(arguments.artist, arguments.title, arguments.year)
        elif arguments.command == "tracks":
            result = tracks(arguments.mbid)
        else:
            result = cover(arguments.artist, arguments.title, arguments.page)
    except (urllib.error.URLError, TimeoutError, ValueError) as error:
        print(f"Lookup failed: {error}", file=sys.stderr)
        return 1
    json.dump(result, sys.stdout, ensure_ascii=False, indent=2)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
