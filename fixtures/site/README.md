# berm-site

The reference crawler, what crab-crawl's ticks are tested against. It reads the sites it watches through `crab-crawl` and keeps their pricing pages, `/pricing`, and their comparison pages, under `/compare/`. Its tools are in the module doc in `src/lib.rs`.

```sh
cargo build --release -p berm-site --target wasm32-unknown-unknown
```
