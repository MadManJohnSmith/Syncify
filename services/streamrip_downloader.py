import asyncio
from streamrip.config import Config
from streamrip.rip.main import Main
import os
import tomlkit

class StreamripDownloader:
    def __init__(self, config_path="config.toml"):
        self.config_path = config_path
        self._prepare_config()
        self.config = Config(self.config_path)
        self.main = Main(self.config)

    def _prepare_config(self):
        # Load the config file
        with open(self.config_path, "r") as f:
            config_data = tomlkit.loads(f.read())

        # Check for environment variables and update the config
        qobuz_email = os.getenv("QOBUZ_EMAIL")
        qobuz_password = os.getenv("QOBUZ_PASSWORD")

        if qobuz_email and qobuz_password:
            config_data["qobuz"]["email_or_userid"] = qobuz_email
            config_data["qobuz"]["password_or_token"] = qobuz_password

        # Write the updated config back to the file
        with open(self.config_path, "w") as f:
            f.write(tomlkit.dumps(config_data))


    async def download(self, urls):
        async with self.main as main:
            await main.add_all(urls)
            await main.resolve()
            await main.rip()

async def download_urls(urls):
    downloader = StreamripDownloader()
    await downloader.download(urls)

if __name__ == "__main__":
    # Example usage:
    # Replace with actual URLs you want to download
    test_urls = [
        "https://www.qobuz.com/us-en/album/rumours-fleetwood-mac/0603497941032"
    ]
    # Check if a config file exists, and create a dummy one if not
    if not os.path.exists("config.toml"):
        with open("config.toml", "w") as f:
            f.write('')
    asyncio.run(download_urls(test_urls))
