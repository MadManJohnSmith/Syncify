#!/usr/bin/env python3
import urllib.request, urllib.parse, json, time, os

names = ["Alan Mearns", "Anika Noni Rose", "Ann Shirley", "April Bender", "BRANKO", "Band Of Horses", "Between the Crosses", "Bill Wurtz", "Black Door Parole", "Bobby Krlic", "Boris Khaikin", "Brooklyn Bounce", "Buckcherry", "Charlie Wilson", "DJ Koze", "DOGSTAR", "Dads", "Dan The Automator", "Dead Meadow", "Diego & Victor Hugo", "Dirty Projectors", "EVIL EXCESS", "Freddie Hubbard", "Föllakzoid", "GURLS", "Giorgia Angiuli", "Godsmack", "Greg Holden", "Hermann", "Hermann Weindorf", "Inky Johnson", "Jaade Mx", "Jacob Collier", "Jeffrey Lamar Williams", "Jesse & Joy", "Jupiter Grains", "Karaoke - Tommy James & The Shondells", "Kathia", "Kay The Yacht", "Keith Jarrett", "Lamb of God", "Laura Carbone", "Lin Cortes", "Little Wings", "Living in Fiction", "Logo", "London After Midnight", "LuciFer", "Mike Krol", "Mild Orange", "Morgan", "Morgan Jones", "Municipal Waste", "NO LOGO", "Nick Murphy", "Paulo Miranda Silveira", "Rebel and a Basketcase", "Shirley Ann Lee", "Soft Jazz Playlist", "Steve Coleman", "Steve Coleman & Five Elements", "Summer Salt", "The Grays", "The Haunted", "The Haxan Cloak", "The Shoes", "The Soronprfbs", "The Time", "Theory Of A Deadman", "Tom & Collins", "Tom Collins", "Vanilla Ice", "Wayland", "Wayland Holyfield", "Wayne Shorter", "World's Fair", "X3SR", "Yak Gotti", "jade mx"]

cache_file = "scripts/mb_artist_cache.json"
cache = {}
if os.path.exists(cache_file):
    try:
        with open(cache_file, "r") as f:
            cache = json.load(f)
    except Exception:
        cache = {}

for name in names:
    if name in cache and cache[name].get("mbid"):
        continue
    clean = name.strip()
    url = f"https://musicbrainz.org/ws/2/artist?query=artist:%22{urllib.parse.quote(clean)}%22&fmt=json&limit=1"
    req = urllib.request.Request(url, headers={"User-Agent": "Syncify/1.0.0 (https://github.com/syncify/syncify)"})
    success = False
    for attempt in range(4):
        try:
            with urllib.request.urlopen(req, timeout=10) as resp:
                data = json.loads(resp.read().decode("utf-8"))
                artists = data.get("artists", [])
                if artists:
                    art = artists[0]
                    cache[name] = {"mbid": art.get("id"), "name": art.get("name"), "score": art.get("score")}
                else:
                    cache[name] = {"mbid": "NOT_FOUND", "name": name, "score": 0}
                success = True
                break
        except Exception as e:
            time.sleep(2.0 * (attempt + 1))
    if not success:
        cache[name] = {"mbid": "NOT_FOUND", "name": name, "score": 0}
    with open(cache_file, "w") as f:
        json.dump(cache, f, indent=2)
    time.sleep(1.0)

print(f"Done. Total cached: {len(cache)}")
