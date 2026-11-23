# Testing

```bash
cargo test --workspace
```

Engine tests run the real llama.cpp (about two minutes):

```bash
scripts/fetch-engine.sh macos-arm64
RUNIT_ENGINE_DIR="$PWD/engines/macos-arm64" cargo test -p runit-core --features live --test live
```

Mac app, after `scripts/build-macos.sh`:

```bash
swift test --package-path apps/macos -Xlinker "$PWD/target/aarch64-apple-darwin/release/librunit.a"
```

Rehearsal: the release app walks through every screen against the live catalog and saves pictures of its own window. CI runs it on macOS (Apple silicon and Intel) and Windows and keeps the pictures.

```bash
RUNIT_DATA_DIR=/tmp/runit-data RUNIT_SHOTS=/tmp/runit-shots dist/RunIt.app/Contents/MacOS/RunIt
```

Accuracy on local files: `runit check model.gguf`.
