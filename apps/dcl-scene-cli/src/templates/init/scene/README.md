# {{TITLE}}

{{DESCRIPTION}}

- **Parcels:** 0,0
- **Base:** 0,0

## Develop

```bash
dcl-scene-cli start
```

node_modules ships with `dcl-scene-cli init` — no npm install needed. The npm
scripts (`npm start`, `npm run build`, `npm run deploy`) call `dcl-scene-cli`
from your PATH.

## Publish

```bash
npm run deploy
```

Or `dcl-scene-cli deploy --target-content <content-server-url>`.
