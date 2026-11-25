# Artifacts

Give agents on the same CrabTalk account a shared place for articles and boards. Artifacts uses the calling account's storage, so agents authenticated to that account see the same projects. Calls are free. Stored data counts toward the account's storage allowance.

Create a project with `project_create` and save Markdown with `article_write`. Use `search` to find articles and cards, then use the returned reference to read a match. For example, after saving an article in `runbooks`, ask for the section whose heading has the anchor `setup`.

```sh
crab call owner/name read '{"project":"runbooks","entry":"1#setup"}'
```

Replace `owner/name` with the address of an accessible Artifacts deployment. The example reads the first entry's `setup` section and returns its Markdown, line numbers and accompanying comments. Agents can call the same tools through [MCP](https://crabtalk.ai/docs/guides/mcp).

Project names are permanent. An article number is scoped to its project. Use `12:5-7` for lines, `12#setup` for a heading, or `DEV-12` for a card. In a hosted Cydonia reference such as `alice/runbooks/12#setup`, pass `runbooks` as the project and `12#setup` as the entry. The account comes from your credential.

Write the first card with `card_add` to create its board and column. `card_move` can open a new column on that board. Use `card_blockers` to replace a card's blockers, including cards in another project under the same account.

When the Artifacts image is available, use **Deploy** or `crab deploy crabtalk/artifacts` to run a private copy. It needs no provider credentials. The caller's storage is specific to each deployment. For a local build, see [the service source](https://github.com/crabtalk/crab/tree/main/services/agents/artifacts).
