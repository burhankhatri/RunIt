# RunIt: design

**Goal.** Install with one command, open, and within about a minute know which open models this
computer runs well and how fast. Download any of them, or anything else on Hugging Bay, and check
the prediction against reality.

## What keeps it honest

1. **The test is the real engine.** RunIt ships pinned llama.cpp binaries. It writes GGUF files with
   real layer shapes and random weights, then times them with `llama-bench` on the GPU (Metal or
   Vulkan) and on the CPU. Speed does not depend on weight values, so this needs no download.
2. **Predictions are a fitted cost model**, not a lookup table:
   - decode: `t_token = touched_bytes / B + layers * k + kv_bytes(depth) / B`
   - prompt: `t_token = active_params * c + p`
   Touched bytes exclude an untied embedding table and the experts a MoE skips for a given token.
   Layers that do not fit on the GPU are costed at the measured CPU rate.
3. **Shape comes from the file.** The first bytes of the GGUF header give layers, heads, KV size and
   experts. If a header cannot be read, the result is labelled a rougher estimate.
4. **Quality is not invented.** Ratings come from LMArena's leaderboard dataset (CC BY 4.0) with date
   and vote count. A model matches only when its name matches exactly after removing packaging
   words. Fine-tunes inherit nothing. Unmatched models say "unrated" and are not headline picks.
5. **The loop closes.** After a download RunIt measures real speed and shows it next to the
   prediction. The measurement then replaces the prediction for that file.

## Not predicted

Image, audio and embedding models, and anything not GGUF, show a size-versus-memory check only and
say no speed estimate is available. They can still be downloaded.

## Parts

| Part | Language | Job |
|---|---|---|
| `crates/runit-core` | Rust | hardware, synthetic GGUF, bench, fit, catalog, ratings, picks, downloads, chat |
| `crates/runit-ffi` | Rust | C ABI: open, call, free, close. JSON in and out, one event callback |
| `crates/runit-cli` | Rust | `runit` command for headless use and end-to-end tests |
| `apps/macos` | Swift, SwiftUI | native macOS 14+ app |
| `apps/windows` | C#, WPF, .NET 10 | native Windows 10/11 x64 app |

llama.cpp runs as a child process, so a GPU driver crash cannot take the app down.

## Catalog

- Snapshot: `/api/v1/catalog/snapshot` then the JSONL.gz, verified by SHA-256, distilled to hosted
  GGUF rows, refreshed at most daily.
- Search: `/api/v1/search`, live, covers everything on the site.
- Files: `/api/v1/artifacts/{id}/download-plan`. Only rows with a `downloadUrl` can be downloaded.
- Bytes: `downloadUrl` redirects to a link valid 300 s. Reopen on expiry, resume with `Range`,
  verify SHA-256, discard on mismatch. Back off on 429 and 503 using `Retry-After`.
- Requests carry a generic `RunIt/<version>` agent. No account, telemetry or upload of results.

## Memory rule

`need = weights + KV(context) + compute buffer`.
Unified memory: capacity is the smaller of the engine's GPU budget and total RAM minus
`max(4 GB, 25%)`. Discrete: capacity is free VRAM. Comfortable at or under 80% of capacity, tight at
or under 100%, then partial offload if RAM allows, otherwise it does not fit.

## Picks

From comfortable (or, for "smartest", tight) rated models at Q4 or better:
- **Best for this computer**: highest rating at 15 tokens/s or more.
- **Quickest**: highest rating at 40 tokens/s or more.
- **For code**: highest LMArena coding rating at 15 tokens/s or more.
- **Smartest it can hold**: highest rating at 6 tokens/s or more, tight allowed.
Duplicates collapse. One file per base model, choosing the best quant that stays in budget, from the
original publisher or an allow-listed quantizer.

## Look and motion

References on Mobbin: Noom "Based on your answers, Sam…" (full-screen serif reveal), Co–Star
(editorial serif verdict), Cal AI plan generation (live checklist under a big figure), Vercel model
list (dense numeric table), Krea model picker (list and detail), Canva proposal (sage paper, forest
ink, italic contrast), Tripadvisor (marker-highlight band).

- A lab report that writes itself. Sage paper, forest ink, and a highlighter that marks the answer.
- Newsreader for voice, Geist for interface, Geist Mono for figures. All OFL, bundled.
- **Signature:** a sample reply types at the model's predicted speed, so 47 tokens/s is felt, not
  only read. Animation here is data.
- Reduced motion shows finished text.

## Errors

Offline: cached catalog and Library keep working. Bench failure: fall back from GPU to CPU and say
so. Not enough disk: smaller test, or refuse a download before it starts.

## Testing

Rust unit and integration tests (parsers, fit, matching, picks, resume and checksum against a local
HTTP server), CLI end to end against the live catalog, prediction accuracy against real models on an
M4, and CI on macOS and Windows runners that installs, opens and screenshots the app.
