import re

filepath = 'c:/Users/madma/OneDrive/Documents/Syncify/ui/src/views/SettingsView.vue'
with open(filepath, 'r', encoding='utf-8') as f:
    content = f.read()

# 1. Replace folder template HTML
pattern1 = re.compile(r'         <!-- 8\. FOLDER STRUCTURE -->\n         <div v-if="activeCategory === \'folders\'" class="space-y-8">.*?         </div>\n\n         <!-- 9\. SYNC & SCHEDULING -->', re.DOTALL)
content = pattern1.sub('         <!-- 8. FOLDER STRUCTURE -->\n         <SettingsDownloads v-if="activeCategory === \'folders\'" />\n\n         <!-- 9. SYNC & SCHEDULING -->', content)

# 2. Add import
content = content.replace(
    "import SettingsSync from './settings/SettingsSync.vue'",
    "import SettingsSync from './settings/SettingsSync.vue'\nimport SettingsDownloads from './settings/SettingsDownloads.vue'"
)

# 3. Remove folder template script variables and logic
# This starts at `// --- Folder Template Logic ---` up to `// Lyrics settings toggle functions`
pattern3 = re.compile(r'// --- Folder Template Logic ---.*?// Lyrics settings toggle functions', re.DOTALL)
content = pattern3.sub('// Lyrics settings toggle functions', content)

# 4. Remove folder template save functions
# `// Save folder settings to backend` up to `// Duplicate settings from composable (reactive)`
pattern4 = re.compile(r'// Save folder settings to backend.*?// Duplicate settings from composable \(reactive\)', re.DOTALL)
content = pattern4.sub('// Duplicate settings from composable (reactive)', content)

# 5. Remove hydration logic in onMounted
pattern5 = re.compile(r'    // Sync folder template state with backend\n    if \(downloadSettings\.folderSettings\.folder_template\) \{\n      folderTemplate\.value = downloadSettings\.folderSettings\.folder_template\n      fileTemplate\.value = downloadSettings\.folderSettings\.file_template\n    \}\n    ', re.DOTALL)
content = pattern5.sub('', content)

with open(filepath, 'w', encoding='utf-8') as f:
    f.write(content)

print("Replaced successfully!")
