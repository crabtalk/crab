# berm-account

The reference sign-in service, what crabkit-auth is tested against. It signs users in with GitHub and Google through `crabkit-auth`, links further identities to an account, and reads and ends sessions. Its tools are in `src/lib.rs`.

```sh
cargo build --release -p berm-account --target wasm32-unknown-unknown
```

## License

Apache-2.0
