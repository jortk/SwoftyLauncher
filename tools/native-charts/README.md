# Native chart development checks

Chart data and validation live in `packages/addon-api/src/view/chart.rs`; the launcher renderer is `apps/launcher/src/blocks/chart.rs`. The gallery and JSON-lines example use the same API and native renderer.

Helpers must check `hello.params.capabilities` for `view.chart`. Treat a missing list as empty. Do not infer chart support from version strings; the PR does not change the release version or API version.

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cargo build -p swofty-addon --example native_charts
python tools/native-charts/protocol-smoke.py target/debug/examples/native_charts
cargo run -p swofty-launcher -- --chart-demo
```

The smoke test verifies capability detection (including older/future version strings), a normal reload Effect after the example button, persisted chart data on subsequent views, null tick replies, explicit gaps and absence of generated chart files. It needs only Python's standard library, not a runtime chart service.
