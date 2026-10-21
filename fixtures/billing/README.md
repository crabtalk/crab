# berm-billing

The reference subscriptions service, what crabkit-stripe is tested against. It opens Stripe Checkout and customer portal sessions for the calling user through `crabkit-stripe`, answers their plan, and keeps the subscriptions Stripe's events name. Its tools are in `src/lib.rs`.

```sh
cargo build --release -p berm-billing --target wasm32-unknown-unknown
```

## License

Apache-2.0
