# `@dcl/rpc`

Transport-agnostic RPC between a client and a server: the client opens
named ports, loads modules on them and calls procedures that return a
single response or a back-pressured stream; the server registers module
factories per port. Transports ship for in-memory pairs, WebSockets and Web
Workers, and `codegen` turns `@dcl/ts-proto` service definitions into typed
clients and server handlers.

Provenance: `decentraland/rpc` 1.1.2.

## Layout

- `src/protocol/index.proto` is the wire protocol. `make build` and `make test`
  generate `src/protocol/index.ts` from it with `protoc` and the
  `@dcl/ts-proto` plugin; the generated file is not tracked.
- `src/transports/` holds the `Memory`, `WebSocket` and `WebWorker` transports.
- `test/codegen/client.proto` is compiled at test time into
  `test/codegen/client.ts` (generic service definitions) for the codegen
  tests.

## Build and test

```sh
make install   # pnpm install --frozen-lockfile
make build     # protoc + tsc -> dist/
make test      # protoc for the test fixtures, then jest twice (SIMMULATE_JITTER=false and =true)
```

`PROTOC=/path/to/protoc make build` selects another protoc binary.

## Messages

The protocol resembles JSON-RPC with a numeric `procedure_id`, binary
payloads and a `port_id` on every packet. Streams share one `message_id`,
carry an incremental `sequence_id`, and are acknowledged per element so a
generator on the server never runs ahead of the consumer; either side closes
a stream with a close acknowledgement.

## License

Apache-2.0 ([LICENSE](LICENSE)).
