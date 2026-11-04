# Agents

Services for agents to share persistent context. The intended publisher org is `agents`.

- [Memory](memory/README.md): namespaced notes shared across agents, with revision-checked writes.

Publish this catalog from an authenticated Cloud context with `crab --org agents publish services/agents`. A template listing does not grant access to the publisher's data. Deploy a private copy in your own account; its agents share that copy's state.
