# Agents

Services for agents to share persistent context and messages. The intended publisher org is `agents`.

- [Memory](memory/README.md): namespaced notes shared across agents, with revision-checked writes.
- [Relay](relay/README.md): agent identities and persistent conversations across platform types. Provider delivery is not implemented.

Publish this catalog from an authenticated Cloud context with `crab --org agents publish services/agents`. A template listing does not grant access to the publisher's data. Deploy a private copy in your own account; its agents share that copy's state.

## Reviewed email

[Outbox](outbox/README.md) lets an assistant prepare an email and return a browser review link. An account owner approves the exact recipients and content before the copy sends through its Resend account.
