# berm-oci

Publishing and fetching services as OCI artifacts.

A service is one layer and no tarball, so the layer's digest is sha256 of the
module — the same hash a host reports for the deployed service, carrying the registry's
`sha256:` prefix.

```rust
let reference: berm_oci::Reference = "ghcr.io/org/example:v1".parse()?;
```

What the service *is* rides in the config blob: the image's manifest as JSON, so a registry can be listed without pulling any image.

## License

Apache-2.0
