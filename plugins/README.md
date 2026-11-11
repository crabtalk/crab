# CrabTalk for ChatGPT

Build `crabtalk.zip` with the command below, then upload it through ChatGPT's plugin upload control, install CrabTalk, and complete its OAuth approval. Select CrabTalk in a conversation to use its tools. Upload availability depends on account and workspace permissions.

The package connects to `https://mcp.crabtalk.ai` with OAuth and dynamic client registration. It includes the CrabTalk logo and a skill for discovering services and calling their tools. Each user authorizes their own account; the archive contains no credentials.

Reuse an existing CrabTalk account connection. When approval is needed, use the host's connection flow and approve the account, permissions and daily budget on the CrabTalk page. Keep the pending flow running until it completes or expires. Once an authenticated `search` succeeds, the connection is ready. The plugin does not require installing the Crab CLI or manually supplying an API key.

Source: [crabtalk/](crabtalk/). From the repository root, build the archive with Python 3:

```sh
python3 plugins/bundle.py
```

The command writes `plugins/crabtalk.zip`, replacing any previous bundle. Generated ZIPs are ignored by Git. The release workflow builds this archive and uploads it to the draft release before publication.

OAuth discovery was checked against the live endpoint. Authenticated tool discovery and ChatGPT import are unverified for this package. This is a private-upload bundle, not a public directory submission.
