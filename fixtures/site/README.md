# berm-site

The reference crawler, what crabkit-crawl's ticks are tested against. It reads the sites it watches through `crabkit-crawl` and keeps their pricing pages, `/pricing`, and their comparison pages, under `/compare/`. Its tools are in the module doc in `src/lib.rs`.

```sh
cargo build --release -p berm-site --target wasm32-unknown-unknown
```
