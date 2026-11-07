# `@dcl/ecs-math`

Math primitives for the Decentraland ECS: `Vector2`, `Vector3`, `Quaternion`,
`Matrix`, `Plane`, `Color3`, `Color4`, `Scalar` and the shared `types`. The
package has no runtime dependencies and compiles to ES2020 modules under
`dist/` with declarations.

Provenance: `decentraland/ecs-math` 2.1.0.

## Build and test

```sh
make install   # pnpm install --frozen-lockfile
make build     # tsc -p tsconfig.json -> dist/
make test      # jest over test/*.spec.ts
```

## License

Apache-2.0 ([LICENSE](LICENSE)).
