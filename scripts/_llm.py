"""Optional free-LLM polish for generated text.

Reads secrets/llm.json (gitignored — never committed):

    {"provider": "gemini", "api_key": "...", "model": "gemini-2.0-flash"}
    {"provider": "groq",   "api_key": "...", "model": "llama-3.3-70b-versatile"}

Free keys (no card): aistudio.google.com → Get API key, or console.groq.com.
ask() returns None when no key is configured — callers keep their template text.
"""
import json
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def _config():
    p = ROOT / "secrets" / "llm.json"
    if not p.exists():
        return None
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None


def ask(prompt, system="You write concise, honest release marketing copy. No fluff, no emoji spam."):
    cfg = _config()
    if not cfg or not cfg.get("api_key"):
        return None
    try:
        if cfg["provider"] == "gemini":
            model = cfg.get("model", "gemini-2.0-flash")
            url = f"https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={cfg['api_key']}"
            body = {"system_instruction": {"parts": [{"text": system}]},
                    "contents": [{"parts": [{"text": prompt}]}]}
        elif cfg["provider"] == "groq":
            model = cfg.get("model", "llama-3.3-70b-versatile")
            url = "https://api.groq.com/openai/v1/chat/completions"
            body = {"model": model,
                    "messages": [{"role": "system", "content": system},
                                 {"role": "user", "content": prompt}]}
        else:
            return None
        req = urllib.request.Request(url, json.dumps(body).encode(),
                                     {"Content-Type": "application/json",
                                      **({"Authorization": f"Bearer {cfg['api_key']}"} if cfg["provider"] == "groq" else {})})
        resp = json.loads(urllib.request.urlopen(req, timeout=60).read())
        if cfg["provider"] == "gemini":
            return resp["candidates"][0]["content"]["parts"][0]["text"]
        return resp["choices"][0]["message"]["content"]
    except Exception as e:
        print(f"llm polish skipped: {e}")
        return None
