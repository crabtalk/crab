# crab-guest

Every guest crab provides, re-exported from one crate under the guest's own name.

| Guest    | Crate          | Does                                                                        |
| -------- | -------------- | --------------------------------------------------------------------------- |
| `stripe` | [`crab-guest-stripe`](https://crates.io/crates/crab-guest-stripe) | Stripe subscriptions for a product's users: Checkout, the customer portal, and each user's plan |

```rust
use crab_guest::stripe;
```

## License

Apache-2.0
