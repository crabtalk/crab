# Documentation

- Update affected documentation in the same change as code, API, CLI, configuration, repository layout, or platform support changes. A change is unfinished while its documentation describes the previous behavior.
- Search the whole repository for affected concepts, commands, examples, paths, and claims before editing. Review `skills/crab/SKILL.md`, the root and crate READMEs, and relevant guides and examples; update each affected source together.
- CrabTalk website guides live in `../crabtalk/www/site/content/docs/`. Check and update affected guides alongside repository docs.
- Verify documented behavior against the implementation. Update or remove stale instructions and claims, and check links and example paths after moves. Report documentation that needs updating outside this repository.
- Public documentation describes reusable services and Rust libraries. Do not present “kits” as a separate product or deployment model; retain literal crate and ABI names where the implementation requires them.
