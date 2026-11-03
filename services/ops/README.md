# Ops

Services for monitoring endpoints and processing operational events. The intended publisher org is `ops`.

- [Uptime](uptime/README.md): scheduled HTTP checks, retained observations and status-page data.
- [Webhooks](webhooks/README.md): signed event ingestion, deduplication and agent acknowledgments.

Publish this catalog from an authenticated Cloud context with `crab --org ops publish services/ops`. Published services use addresses such as `ops/uptime` and `ops/webhooks`; availability depends on publication. Stateful templates run as private copies in the deploying account.
