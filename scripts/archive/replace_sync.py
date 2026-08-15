import re

filepath = 'c:/Users/madma/OneDrive/Documents/Syncify/ui/src/views/SettingsView.vue'
with open(filepath, 'r', encoding='utf-8') as f:
    content = f.read()

# 1. Replace sync HTML
pattern1 = re.compile(r'         <!-- 9\. SYNC & SCHEDULING -->\n         <div v-if="activeCategory === \'sync\'" class="space-y-8">.*?         </div>\n\n         <!-- 10\. ADVANCED -->', re.DOTALL)
content = pattern1.sub('         <!-- 9. SYNC & SCHEDULING -->\n         <SettingsSync v-if="activeCategory === \'sync\'" />\n\n         <!-- 10. ADVANCED -->', content)

# 2. Add import
content = content.replace(
    "import SettingsMetadata from './settings/SettingsMetadata.vue'\nimport { getServices",
    "import SettingsMetadata from './settings/SettingsMetadata.vue'\nimport SettingsSync from './settings/SettingsSync.vue'\nimport { getServices"
)

# 3. Remove syncServicesList
pattern3 = re.compile(r'// Service list for sync settings UI\nconst syncServicesList = \[\n.*?\]\n\n// Backend state', re.DOTALL)
content = pattern3.sub('// Backend state', content)

# 4. Remove syncSettings.loadSettings()
content = content.replace(
    "    // Load sync settings from backend (Sprint 1)\n    await syncSettings.loadSettings()\n    \n    // Load download settings from backend (Sprint 2)",
    "    // Load download settings from backend (Sprint 2)"
)

with open(filepath, 'w', encoding='utf-8') as f:
    f.write(content)

print("Replaced successfully!")
