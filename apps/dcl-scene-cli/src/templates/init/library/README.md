# {{TITLE}}

{{DESCRIPTION}}

A Decentraland SDK7 library. Scenes call `initLibrary(engine)` with their own
engine, so the library and the scene share components.

- `dcl-scene-cli start` previews the library as a scene (`main()` in `src/index.ts`).
- `npm run build:lib` bundles `src/index.ts` to `src/index.js` (the SDK stays a
  `require`, so a scene bundles one SDK) and writes the declarations to `dist/`.
- Publish with `npm publish`; `files` in `package.json` lists what ships.
