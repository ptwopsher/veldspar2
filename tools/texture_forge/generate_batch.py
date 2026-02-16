#!/usr/bin/env python3
"""
Batch texture generator for Veldspar.
Reads prompts from a TOML file and generates all textures.
Usage: python3 generate_batch.py prompts.toml output_dir/
"""

import requests
import base64
import json
import sys
import time
from pathlib import Path

try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib
    except ImportError:
        print("Need Python 3.11+ (tomllib) or 'pip install tomli'")
        sys.exit(1)

API_URL = "http://localhost:8080/v1/messages"
MODEL = "gemini-3-pro-image"


def generate_image(prompt: str) -> bytes | None:
    payload = {
        "model": MODEL,
        "max_tokens": 8096,
        "stream": False,
        "messages": [{"role": "user", "content": prompt}]
    }
    headers = {
        "Content-Type": "application/json",
        "x-api-key": "test"
    }

    try:
        response = requests.post(API_URL, json=payload, headers=headers, timeout=120)
        response.raise_for_status()
    except requests.exceptions.ConnectionError:
        print("  ❌ Cannot connect to proxy at localhost:8080")
        return None
    except requests.exceptions.Timeout:
        print("  ❌ Request timed out")
        return None
    except requests.exceptions.HTTPError as e:
        print(f"  ❌ API Error: {e}")
        return None

    data = response.json()
    for block in data.get("content", []):
        if block.get("type") == "image":
            source = block.get("source", {})
            if source.get("type") == "base64":
                return base64.b64decode(source["data"])

    for block in data.get("content", []):
        if block.get("type") == "text":
            print(f"  📝 Text response: {block.get('text', '')[:100]}")

    print("  ❌ No image in response")
    return None


def main():
    if len(sys.argv) < 3:
        print("Usage: python3 generate_batch.py prompts.toml output_dir/")
        sys.exit(1)

    prompts_file = Path(sys.argv[1])
    output_dir = Path(sys.argv[2])
    output_dir.mkdir(parents=True, exist_ok=True)

    with open(prompts_file, "rb") as f:
        config = tomllib.load(f)

    textures = config.get("textures", [])
    print(f"🎨 Veldspar Texture Forge — {len(textures)} textures to generate\n")

    success = 0
    failed = 0

    for i, tex in enumerate(textures, 1):
        name = tex["name"]
        prompt = tex["prompt"]
        out_path = output_dir / f"{name}.png"

        if out_path.exists():
            print(f"[{i}/{len(textures)}] ⏭  {name}.png already exists, skipping")
            success += 1
            continue

        print(f"[{i}/{len(textures)}] ⏳ Generating {name}.png ...")
        image_data = generate_image(prompt)

        if image_data:
            out_path.write_bytes(image_data)
            print(f"  ✅ Saved ({len(image_data) / 1024:.1f} KB)")
            success += 1
        else:
            print(f"  ❌ Failed")
            failed += 1

        # Small delay to not hammer the API
        if i < len(textures):
            time.sleep(1)

    print(f"\n{'='*40}")
    print(f"Done: {success} success, {failed} failed out of {len(textures)}")


if __name__ == "__main__":
    main()
