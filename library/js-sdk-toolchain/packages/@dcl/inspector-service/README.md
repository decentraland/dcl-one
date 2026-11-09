# @dcl/inspector-service

The inspector's data layer and editor model, without any UI or renderer:

- the file-system and remote data layers plus the RPC host (`createDataLayerHost`,
  `createEngineContext`, `DataServiceDefinition`), which `@dcl/sdk-commands start --data-layer`
  runs in Node;
- the editor components (`lib/sdk/components`), the composite/CRDT dumps
  (`dumpEngineToComposite`, `dumpEngineToCrdtCommands`) and the scene tree helpers;
- `getConfig()` / `InspectorConfig` (the content server and the analytics source), the inspector
  preferences, the in-memory storage and the asset catalog.

Node consumers (`@dcl/sdk-commands`) load the esbuild bundle in `dist/index.js`; the `source`
export condition lets a bundler consume the TypeScript source directly.

```bash
pnpm run build   # dist/index.js + declarations
pnpm run test    # vitest
```
