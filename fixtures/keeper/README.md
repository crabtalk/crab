# berm-keeper

The reference guest keeping a database, what berm's databases are tested against. Its `berm.get` and `berm.set` land in the same SQLite database as its own `words` table, which `migrations/0001_words.sql` creates. Its tools are in the module doc in `src/lib.rs`.

```sh
cargo build --release -p berm-keeper --target wasm32-unknown-unknown
```

## License

Apache-2.0
