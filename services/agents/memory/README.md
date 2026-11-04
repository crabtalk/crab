# Agent memory

Share persistent notes and context across your agents. Each private copy has one shared set of namespaced memories; it does not isolate individual agents within the account.

## Deploy and call

If this listing offers **Deploy your own**, create a private copy without a local build:

```sh
crab deploy --from owner/memory
crab call you/memory put '{"namespace":"project","key":"release-notes","content":"The next release needs a migration guide.","expected_revision":0}'
```

Replace `owner` with the template publisher and `you` with your deploying account. Connect the same copy to multiple agents through [MCP](https://crab.talk/docs/guides/mcp). No provider credentials or settings are required. The copy owns its data and execution bill.

`get` reads a key. `search` matches an ASCII case-insensitive substring within one namespace; an empty query lists it. Pass the returned `next` as `after` to continue. It is text search, without embeddings or semantic ranking.

`put` returns a revision. Pass it as `expected_revision` when updating or deleting to reject a concurrent change; zero creates only when absent. Omitting it allows an unconditional write. Revisions stay unique across deletion and recreation within this copy.

## Limits

Namespaces are 1–64 bytes, keys 1–128 bytes, and content 1–16384 bytes. Search accepts up to 256 bytes and returns up to ten memories per page. A copy permits 10000 entries within its 64 MiB database reservation. It fails explicitly when full; memories have no automatic expiry.

Keep the deployment private: making it public opens its shared memory tools to other authenticated accounts. The platform's account membership and keys control access; namespaces are organization, not an authorization boundary.

[Source deployment](https://github.com/crabtalk/crab/blob/dev/services/README.md#building-from-source) · [Rust API](https://github.com/crabtalk/crab/blob/dev/services/agents/memory/LIBRARY.md)
