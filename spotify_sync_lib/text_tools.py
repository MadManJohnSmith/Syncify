import unicodedata
import re
from .config import APP_CONFIG # Use relative import for APP_CONFIG

def normalize_text_advanced(text, is_artist=False):
    if not text: return ""
    normalized_text = str(text)
    
    for pattern in APP_CONFIG["normalization_patterns_to_remove_regex"]:
        normalized_text = pattern.sub("", normalized_text)
    
    normalized_text = unicodedata.normalize('NFKD', normalized_text).encode('ascii', 'ignore').decode('utf-8')
    
    if is_artist:
        normalized_text = re.sub(r'feat\..*', '', normalized_text, flags=re.IGNORECASE)
        normalized_text = re.sub(r'ft\..*', '', normalized_text, flags=re.IGNORECASE) # Also ft.
        normalized_text = re.sub(r'[,&].*', '', normalized_text) # Takes only first artist
        # Attempt to remove ' a.k.a. ...'
        normalized_text = re.sub(r'\s+a\.?k\.?a\.?.*', '', normalized_text, flags=re.IGNORECASE)
    else: # Is title
        normalized_text = re.sub(r'[\(\[\{].*?[\)\]\}]', '', normalized_text) 
        normalized_text = re.sub(r'[:-].*$', '', normalized_text) 
    
    normalized_text = re.sub(r"[^\w\s]", "", normalized_text) 
    normalized_text = normalized_text.lower().strip()
    normalized_text = re.sub(r'\s+', ' ', normalized_text)
    return normalized_text

def extract_version_keywords(title):
    if not title: return set()
    title_lower = title.lower()
    found_keywords = set()
    
    for kw in APP_CONFIG["version_keywords"]:
        if re.search(r'\b' + re.escape(kw) + r'\b', title_lower):
            found_keywords.add(kw)
            
    bracket_content = re.findall(r'[\(\[\{](.*?)[\)\]\}]', title_lower)
    for content_part in bracket_content:
        for kw in APP_CONFIG["version_keywords"]:
            if re.search(r'\b' + re.escape(kw) + r'\b', content_part):
                found_keywords.add(kw)
    return found_keywords

def generate_block_key(norm_artist, norm_title):
    key_parts = []
    if norm_artist and len(norm_artist) > 0:
        key_parts.append(norm_artist[0])
    # More robust: use first few letters if available
    # if norm_artist:
    #     key_parts.append(norm_artist[:min(len(norm_artist), 2)])


    if norm_title:
        title_words = norm_title.split()
        if title_words:
            # Use first letter of the first significant word (not common short words like 'a', 'an', 'the')
            # This part can be enhanced. For now, just first letter of first word.
            key_parts.append(title_words[0][0])
    
    return "".join(key_parts) if key_parts else "default_block"