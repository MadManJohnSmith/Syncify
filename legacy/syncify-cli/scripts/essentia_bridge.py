#!/usr/bin/env python3
"""
Essentia + TensorFlow Audio Feature & Multi-Mood Bridge for Syncify
Extracts BPM, Short Key (e.g. Am, C), Energy, Danceability, Loudness,
Top Mood, and Top-3 Discogs-400 styles.
"""

import sys
import os
import json

KEY_MAP = {
    "A minor": "Am", "A major": "A",
    "A# minor": "A#m", "A# major": "A#",
    "B minor": "Bm", "B major": "B",
    "C minor": "Cm", "C major": "C",
    "C# minor": "C#m", "C# major": "C#",
    "D minor": "Dm", "D major": "D",
    "D# minor": "D#m", "D# major": "D#",
    "E minor": "Em", "E major": "E",
    "F minor": "Fm", "F major": "F",
    "F# minor": "F#m", "F# major": "F#",
    "G minor": "Gm", "G major": "G",
    "G# minor": "G#m", "G# major": "G#"
}

def extract_features(audio_path):
    if not os.path.exists(audio_path):
        return {"success": False, "error": f"Audio file not found: {audio_path}"}

    try:
        import essentia
        import essentia.standard as es
    except ImportError:
        return {
            "success": False,
            "error": "Essentia library not installed in Python environment"
        }

    try:
        # Extract audio features using Essentia MusicExtractor
        features, _ = es.MusicExtractor(
            lowlevelStats=['mean'],
            rhythmStats=['mean'],
            tonalStats=['mean']
        )(audio_path)

        bpm = round(float(features['rhythm.bpm']), 1)
        raw_key = f"{features['tonal.key_edma.key']} {features['tonal.key_edma.scale']}"
        short_key = KEY_MAP.get(raw_key, raw_key)

        energy = round(float(features['lowlevel.average_loudness']), 3)
        danceability = round(float(features['rhythm.danceability']), 3)
        loudness = round(float(features['lowlevel.loudness_ebu128.integrated']), 1)

        # Multi-Mood Classifiers (happy, sad, relaxed, aggressive, party, acoustic, electronic)
        moods = {}
        top_mood = None
        highest_mood_score = 0.0

        for mood_name in ["happy", "sad", "relaxed", "aggressive", "party", "acoustic", "electronic"]:
            stat_key = f"highlevel.mood_{mood_name}.all.probability"
            try:
                score = round(float(features[stat_key]), 3)
                moods[mood_name] = score
                if score > highest_mood_score and score >= 0.4:
                    highest_mood_score = score
                    top_mood = mood_name
            except Exception:
                pass

        # TensorFlow Discogs400 Style Classification
        styles = []
        try:
            import tensorflow as tf
            model_dir = os.path.join(os.path.dirname(__file__), "models")
            model_path = os.path.join(model_dir, "genre_discogs400-discogs-effnet-1.pb")
            if os.path.exists(model_path):
                audio = es.MonoLoader(filename=audio_path, sampleRate=16000)()
                model = es.TensorflowPredictEffnetDiscogs(graphFilename=model_path)
                predictions = model(audio)
                top_indices = predictions.mean(axis=0).argsort()[-3:][::-1]
                for idx in top_indices:
                    styles.append({
                        "style": f"Style-{idx}",
                        "probability": round(float(predictions.mean(axis=0)[idx]), 3)
                    })
        except Exception:
            pass # Non-fatal if TensorFlow models are offline

        return {
            "success": True,
            "bpm": bpm,
            "key": short_key,
            "energy": energy,
            "danceability": danceability,
            "loudness": loudness,
            "mood": top_mood,
            "moods": moods,
            "styles": styles
        }
    except Exception as e:
        return {"success": False, "error": str(e)}

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(json.dumps({"success": False, "error": "Usage: essentia_bridge.py <path_to_flac>"}))
        sys.exit(1)

    audio_file = sys.argv[1]
    result = extract_features(audio_file)
    print(json.dumps(result))
