# berm-bare

The reference guest declaring nothing beyond the ABI, what berm's refusals of undeclared syscalls are tested against. Its tools are in the module doc in `src/lib.rs`.

```sh
cargo build --release -p berm-bare --target wasm32-unknown-unknown
```

## License

Apache-2.0
