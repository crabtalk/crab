# Service documentation

- Follow [README-guide.md](README-guide.md) when writing or changing a service README. These READMEs appear on crabtalk.ai and in MCP: lead with calling a deployment and deploying a private copy, and keep source integration details separate.
- Public source links must target `main`; Rust dependency examples must pin a released tag matching `berm-lang`. Never link to or depend on `dev` in service documentation.
- Update affected READMEs, `LIBRARY.md` files and website guides when tools, credentials, variables, deployment commands or source paths change. Check examples against the implementation and built schemas when available, and verify link targets before finishing.
