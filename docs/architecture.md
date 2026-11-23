# RunIt architecture

Describes the system at HEAD. Update it in the same commit as any change it describes.

## Shape

```
apps/macos (SwiftUI)      apps/windows (WPF, .NET 10)      crates/runit-cli
          \                     |                              /
           C ABI: crates/runit-ffi (open, call, free, close)  /
                          \                                  /
                           crates/runit-core  --- child processes --- llama.cpp b12651
                                   |
                         huggingbay.xyz (HTTPS)
```

JSON goes in and out of `runit_call(method, params)`. Progress arrives on one event callback, from any thread. Both apps hold no logic beyond display.

## runit-core

| Module | Job |
|---|---|
| `hardware` | CPU, memory, GPU (asked of the engine itself), memory capacity rules |
| `conditions` | heat (macOS thermal pressure) and battery power |
| `engine` | locate and run llama.cpp; parse devices and timings; `Guard` ends children even if the app is killed (pipe watcher on Unix, job object on Windows) |
| `gguf` | read whole or cut-off headers; write random-weight test models |
| `bench` | the test: up to three generated models, GPU first while cool, 20 s steady run, then CPU; the CPU is chosen when it writes over 15% faster |
| `predict` | least-squares fit; memory tiers; speed for any model shape, including split loads |
| `catalog` | Hugging Bay snapshot (SHA-256 checked, cached as `catalog-2.json`), search, file plans, ranged headers |
| `quality` | LMArena ratings bundled from `data/quality.json`; exact-name matching; publisher trust |
| `recommend` | picks and ranked lists |
| `download`, `items` | resumable verified downloads; two at once; library list |
| `run` | time a real model; chat through `llama-server` on 127.0.0.1 with a one-time key |
| `app` | named calls for the apps |

## Files on disk

`~/Library/Application Support/RunIt`, `%LOCALAPPDATA%\RunIt`, or `RUNIT_DATA_DIR`: `report.json`, `headers.json`, `catalog-2.json`, `items.json`, `settings.json`, `bench/` (emptied after a test), `models/` (movable in This Mac / This PC).

## Network

Only `https://huggingbay.xyz` and the signed storage links it redirects to. `/api/v1/catalog/snapshot` (at most one 45 MB fetch per published day), `/api/v1/search`, `/api/v1/artifacts/{id}/download-plan`, `/api/downloads/...`. No account, no telemetry. 429 and 503 wait for `Retry-After`, two retries.

## Limits and defaults

Context assumed for memory: 8192. Comfortable: 80% of capacity. Unified memory keeps the larger of 4 GB and 25% for the system. Smooth 10 tok/s, quick 40, patient 6. Headers read for the 28 best-rated candidates plus 8 unrated. Suggested quants: 4 bits or more, under 9. Disk kept free: 1 GiB.

## Environment variables

`RUNIT_DATA_DIR`, `RUNIT_ENGINE_DIR`, and for checking builds `RUNIT_SHOTS`, `RUNIT_LOOK`, `RUNIT_FILM`, `RUNIT_RETEST`, `RUNIT_FETCH`. Installers: `RUNIT_REPO`, `RUNIT_BASE`, `RUNIT_NO_OPEN`.

## Releases

A `v*` tag builds both apps, publishes zips and `SHA256SUMS`, then runs both install commands on clean runners.

## Known gaps

- The apps are ad-hoc signed (macOS) and unsigned (Windows). The install commands are unaffected; a zip downloaded in a browser will be warned about.
- One GPU is used, the best discrete one. Windows uses Vulkan, not CUDA. Windows on ARM runs the x64 build.
- Mixtures are never placed with experts on the CPU, so some that could run split are called too big or slow.
- Windows does not report heat, so `hot` is always false there.
- A changed models folder takes effect after reopening.
- `data/quality.json` is refreshed by hand with `scripts/quality.py`.
