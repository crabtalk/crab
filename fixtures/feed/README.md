# berm-feed

The reference source service, what crab-lake's ticks are tested against. It follows one RSS feed, `http://127.0.0.1/feed.xml`, through `crab-lake`: `tick` stores each item new to it, and `posts` reads them back. Its tools are in the module doc in `src/lib.rs`.

```sh
cargo build --release -p berm-feed --target wasm32-unknown-unknown
```

## License

Apache-2.0
