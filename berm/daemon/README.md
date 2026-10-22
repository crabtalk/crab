# bermd

A long-running service that deploys programs and serves their tools over HTTP,
and their `anonymous` tools over MCP.

```sh
bermd --addr 127.0.0.1:7777 --root ~/.berm
curl -X PUT --data-binary @program.wasm 'http://127.0.0.1:7777/programs/example?public=echo'
curl -X POST -H 'X-Berm-Caller: {"id": "alice"}' -d '{"query":"hi"}' \
  http://127.0.0.1:7777/programs/example/tools/echo
```

[Running the Service](https://berm.run/docs/berm/service/) covers the control
API, calls, budgets, secrets and MCP.

The endpoint carries no authorization and binds loopback by default.

## License

Apache-2.0
