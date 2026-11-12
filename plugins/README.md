# CrabTalk for ChatGPT

Build `crabtalk.zip` with the command below, then upload it through ChatGPT's plugin upload control, install CrabTalk, and complete its OAuth approval. Select CrabTalk in a conversation to use its tools. Upload availability depends on account and workspace permissions.

The package uses OpenAI's `.codex-plugin/plugin.json` layout with explicit `skills` and `mcpServers` paths. Root `.mcp.json` declares `https://mcp.crabtalk.ai`; the host discovers its OAuth authorization server and dynamic registration endpoint. It includes the CrabTalk logo and the shared skill for selecting and invoking individual service operations. Each hosted operation is advertised upfront with its input schema and effect annotations; use its exact listed name and pass arguments directly. Each user authorizes their own account; the archive contains no credentials.

The repository marketplace at `.agents/plugins/marketplace.json` points to the generated package at `plugins/build/crabtalk` and sets authentication to `ON_INSTALL`. Build before using that local marketplace. The marketplace policy is not part of the upload ZIP.

Reuse an existing CrabTalk account connection. When approval is needed, use the host's connection flow and approve the account, permissions and daily budget on the CrabTalk page. Keep the pending flow running until it completes or expires. Once an authenticated tool listing succeeds, the connection is ready. The plugin does not require installing the Crab CLI or manually supplying an API key.

Package configuration: [crabtalk/](crabtalk/). The canonical [crab skill](../skills/crab/SKILL.md) and its references are shared with CLI users; there is no separate plugin skill. From the repository root, build the archive with Python 3:

```sh
python3 plugins/bundle.py
```

The command assembles `plugins/build/crabtalk` from the package configuration and canonical skill, then writes `plugins/crabtalk.zip`, replacing the previous build and bundle. Both generated outputs are ignored by Git. The release workflow builds this archive and uploads it to the draft release before publication.

The manifest includes five positive and three negative review cases under `extensions.com.openai.review.test_cases`. They were drafted from the live public service definitions and have not been run through the native plugin. Release notes are under `extensions.com.openai.publication`. The plugin version stays `0.1.0`.

OAuth discovery and listing URLs were checked against live endpoints. End-to-end OAuth and authenticated tool use remain unverified. Supporting content still needs a reviewer-accessible demo recording; reviewer access belongs in the secure portal form. Country targeting and any saved recording are omitted from the ZIP so existing portal values are preserved.
