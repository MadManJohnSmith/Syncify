import requests
import hashlib

class QobuzMigrator:
    def __init__(self, from_credentials, to_credentials, app_id, app_secret):
        self.app_id = app_id
        self.app_secret = app_secret
        self.from_session, self.from_user_id, self.from_auth_token = self._login(from_credentials)
        self.to_session, self.to_user_id, self.to_auth_token = self._login(to_credentials)

    def _login(self, credentials):
        session = requests.Session()
        password_md5 = hashlib.md5(credentials["password"].encode()).hexdigest()
        login_params = {
            "email": credentials["email"],
            "password": password_md5,
            "app_id": self.app_id,
        }
        response = session.post("https://www.qobuz.com/api.json/0.2/user/login", params=login_params)
        response.raise_for_status()
        login_data = response.json()
        user_id = login_data["user"]["id"]
        auth_token = login_data["user_auth_token"]
        return session, user_id, auth_token

    def _get_favorites(self, user_id, auth_token, session, favorite_type):
        offset = 0
        limit = 100
        favorites = []
        while True:
            params = {
                "user_id": user_id,
                "limit": limit,
                "offset": offset,
                "app_id": self.app_id,
                "user_auth_token": auth_token,
            }
            response = session.get(f"https://www.qobuz.com/api.json/0.2/{favorite_type}/getUserFavorites", params=params)
            response.raise_for_status()
            data = response.json()
            items = data[favorite_type + 's']['items']
            if not items:
                break
            favorites.extend(items)
            offset += limit
        return favorites

    def export_favorites(self):
        print("Exporting favorites...")
        tracks = self._get_favorites(self.from_user_id, self.from_auth_token, self.from_session, "track")
        albums = self._get_favorites(self.from_user_id, self.from_auth_token, self.from_session, "album")
        artists = self._get_favorites(self.from_user_id, self.from_auth_token, self.from_session, "artist")
        return {"tracks": tracks, "albums": albums, "artists": artists}

    def _add_favorite(self, user_id, auth_token, session, favorite_type, item_id):
        params = {
            "user_id": user_id,
            f"{favorite_type}_id": item_id,
            "app_id": self.app_id,
            "user_auth_token": auth_token,
        }
        response = session.post(f"https://www.qobuz.com/api.json/0.2/{favorite_type}/addFavorite", params=params)
        response.raise_for_status()

    def import_favorites(self, favorites):
        print("Importing favorites...")
        for track in favorites["tracks"]:
            self._add_favorite(self.to_user_id, self.to_auth_token, self.to_session, "track", track["id"])
        for album in favorites["albums"]:
            self._add_favorite(self.to_user_id, self.to_auth_token, self.to_session, "album", album["id"])
        for artist in favorites["artists"]:
            self._add_favorite(self.to_user_id, self.to_auth_token, self.to_session, "artist", artist["id"])


    def migrate(self):
        favorites = self.export_favorites()
        self.import_favorites(favorites)
        print("Migration complete.")

if __name__ == "__main__":
    # This is a placeholder and will not work without a valid app_id and app_secret
    # You can obtain these by inspecting the network requests of the Qobuz web player
    APP_ID = "YOUR_APP_ID"
    APP_SECRET = "YOUR_APP_SECRET"

    from_creds = {"email": "from@example.com", "password": "password"}
    to_creds = {"email": "to@example.com", "password": "password"}
    migrator = QobuzMigrator(from_creds, to_creds, APP_ID, APP_SECRET)
    migrator.migrate()
