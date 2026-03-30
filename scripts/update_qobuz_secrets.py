
import re
import requests
import os
from pathlib import Path

def update_qobuz_secrets():
    print("Fetching Qobuz login page...")
    try:
        # Get login page to find bundle URL
        resp = requests.get("https://play.qobuz.com/login", headers={"User-Agent": "Mozilla/5.0"})
        resp.raise_for_status()
        html = resp.text
        
        # Find bundle.js
        # Pattern: <script src="/resources/20.0.0-b000/bundle.js"></script>
        # or similar
        bundle_match = re.search(r'<script src="([^"]+/bundle\.js)"', html)
        if not bundle_match:
            print("Could not find bundle.js in login page")
            return False
            
        src = bundle_match.group(1)
        if src.startswith("http"):
             bundle_url = src
        else:
             bundle_url = "https://play.qobuz.com" + src
        print(f"Found bundle: {bundle_url}")
        
        # Fetch bundle
        resp = requests.get(bundle_url, headers={"User-Agent": "Mozilla/5.0"})
        resp.raise_for_status()
        js = resp.text
        
        # Extract App ID and Secret
        # Pattern from streamrip: production:{api:{appId:"(?P<app_id>\d{9})",appSecret:"(\w{32})
        # Note: formatting might vary, so be flexible with whitespace
        # production:{api:{appId:"123456789",appSecret:"..."
        
        match = re.search(r'production:\s*{\s*api:\s*{\s*appId:\s*"(?P<app_id>\d+)",\s*appSecret:\s*"(?P<secret>\w+)"', js)
        
        if not match:
             # Try simpler pattern just for appId/secret near "production"
             match = re.search(r'appId:"(\d+)",appSecret:"(\w+)"', js)
        
        if not match:
            print("Could not find App ID/Secret in bundle.js")
            return False
            
        app_id = match.group(1) if match.lastindex >= 1 else match.group("app_id")
        secret = match.group(2) if match.lastindex >= 2 else match.group("secret")
        
        print(f"Found App ID: {app_id}")
        print(f"Found Secret: {secret[:5]}...")
        
        # Update .env
        env_path = Path(".env")
        if not env_path.exists():
            print(".env not found")
            return False
            
        lines = env_path.read_text(encoding='utf-8').splitlines()
        new_lines = []
        updated_id = False
        updated_secret = False
        
        for line in lines:
            if line.startswith("QOBUZ_APP_ID="):
                new_lines.append(f"QOBUZ_APP_ID={app_id}")
                updated_id = True
            elif line.startswith("QOBUZ_APP_SECRET="):
                new_lines.append(f"QOBUZ_APP_SECRET={secret}")
                updated_secret = True
            else:
                new_lines.append(line)
        
        if not updated_id:
            new_lines.append(f"QOBUZ_APP_ID={app_id}")
        if not updated_secret:
            new_lines.append(f"QOBUZ_APP_SECRET={secret}")
            
        env_path.write_text("\n".join(new_lines), encoding='utf-8')
        print("Updated .env successfully")
        return True
        
    except Exception as e:
        print(f"Error: {e}")
        return False

if __name__ == "__main__":
    update_qobuz_secrets()
