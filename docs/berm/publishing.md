---
title: Publishing a Program
description: "A program is one file, so it travels as one OCI layer with no tarball around it."
---

`berm-oci` pushes a program to an OCI registry and pulls it back:

```rust,ignore
let reference: berm_oci::Reference = "ghcr.io/org/example:v1".parse()?;
let registry = berm_oci::Registry::open(&reference, berm_oci::Access::Write)?;
let digest = registry.push("v1", &wasm)?;
```

A host that pulls from a registry deploys a reference; `bermd` takes the image
itself, so it never talks to a registry, holds no credentials, and restores from
`--root` on start without a network.

## The artifact

```json
{
  "schemaVersion": 2,
  "mediaType": "application/vnd.oci.image.manifest.v1+json",
  "artifactType": "application/vnd.berm.program.v1",
  "config": { "mediaType": "application/vnd.berm.manifest.v1+json", "digest": "sha256:…" },
  "layers": [ { "mediaType": "application/vnd.berm.program.v1", "digest": "sha256:…" } ]
}
```

The config blob is the image's [manifest](./manifest.md) as JSON, as `Manifest::from_image` reads it when the image is pushed. The tools, their schemas and the README are one blob, and reading them never means pulling the program.

Nothing about the program is repeated in annotations. Only
`org.opencontainers.image.source` and `.revision` are set, from
`GITHUB_REPOSITORY` and `GITHUB_SHA` when a build has them, which is what makes
GHCR show a package against its repository.

## One digest

Because the layer is the image and nothing else, the layer's digest is sha256 of
the image — the same hash a host reports for the deployed program, carrying the
registry's `sha256:` prefix:

```console
$ shasum -a256 target/berm/berm_fixture.wasm
222890c498ed28f4bf60670a223141489d9879020bd1890111b8c11ac79fa31d
```

A pulled image is checked against the digest the registry advertised before it
goes anywhere, so a registry that serves the wrong bytes is caught by the deploy
rather than by the model.

## Credentials

`GITHUB_TOKEN` is the registry credential; in a workflow that pushes, it needs
`permissions: packages: write`, and locally `GITHUB_TOKEN=$(gh auth token)` does
the same. `~/.docker/config.json` is not read. A pull from a public repository
needs none.

`BERM_REGISTRY_MIRRORS` reaches a registry at another address, as
`name=endpoint` pairs separated by commas. A host in a container pulls what was
pushed to `localhost:5001` from `http://registry:5000` with
`BERM_REGISTRY_MIRRORS=localhost:5001=http://registry:5000`.
