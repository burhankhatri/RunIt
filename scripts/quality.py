#!/usr/bin/env python3
"""Refresh data/quality.json from LMArena's leaderboard dataset (CC BY 4.0)."""
import json, pathlib, time, urllib.parse, urllib.request

API = "https://datasets-server.huggingface.co/filter?"
DATASET = "lmarena-ai/leaderboard-dataset"


def get(url):
    for wait in (2, 5, 10, 20, 40, 0):
        try:
            return json.load(urllib.request.urlopen(url, timeout=90))
        except Exception as e:  # the dataset server times out while it warms up
            err = e
            time.sleep(wait)
    raise SystemExit(f"{url}: {err}")


def rows(category):
    out, total = [], 1
    while len(out) < total:
        q = dict(dataset=DATASET, config="text", split="latest", where=f"\"category\"='{category}'", offset=len(out), length=100)
        page = get(API + urllib.parse.urlencode(q))
        total = page["num_rows_total"]
        out += [r["row"] for r in page["rows"]]
    return out


overall, coding = rows("overall"), {r["model_name"]: r for r in rows("coding")}
models = [
    {
        "name": r["model_name"],
        "rating": round(r["rating"]),
        "votes": int(r["vote_count"]),
        "coding": round(coding[r["model_name"]]["rating"]) if r["model_name"] in coding else None,
    }
    for r in overall
    if "roprietary" not in (r["license"] or "Proprietary")
]
out = pathlib.Path(__file__).parent.parent / "data/quality.json"
out.write_text(json.dumps({
    "source": "LMArena text leaderboard, lmarena-ai/leaderboard-dataset on Hugging Face, CC BY 4.0",
    "date": overall[0]["leaderboard_publish_date"],
    "models": models,
}, indent=1) + "\n")
print(len(models), "open models")
