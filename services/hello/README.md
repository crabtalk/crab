# Hello

Hello answers a greeting. It keeps no state and calls no provider, so it is the smallest service to call, deploy or read as a starting point.

Call the running deployment with Crab, without an account, or connect your agent through [MCP](https://crabtalk.ai/docs/guides/mcp). It returns `{"message":"Hello, crab!"}`, or greets the world when `name` is omitted.

```sh
crab call crabtalk/hello hello '{"name":"crab"}'
```

Select **Deploy** or run `crab deploy crabtalk/hello` to start your own copy. It needs no secrets or variables. You can also [deploy from source](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source) with the `services/hello` directory.
