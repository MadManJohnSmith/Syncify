import requests

BASE_URL = "https://lrclib.net/api"

def fetch_lyrics(track_name, artist_name, album_name):
    """
    Fetches lyrics for a given track.
    """
    search_params = {
        "track_name": track_name,
        "artist_name": artist_name,
        "album_name": album_name,
    }
    response = requests.get(f"{BASE_URL}/search", params=search_params)
    response.raise_for_status()
    search_results = response.json()

    if not search_results:
        return None

    # Assume the first result is the correct one
    track_id = search_results[0]["id"]
    response = requests.get(f"{BASE_URL}/get/{track_id}")
    response.raise_for_status()
    lyrics_data = response.json()

    return lyrics_data.get("syncedLyrics")

if __name__ == "__main__":
    # Example usage
    lyrics = fetch_lyrics("Bohemian Rhapsody", "Queen", "A Night At The Opera")
    if lyrics:
        print(lyrics)
    else:
        print("Lyrics not found.")
