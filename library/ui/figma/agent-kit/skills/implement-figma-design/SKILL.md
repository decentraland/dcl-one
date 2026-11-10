---
name: implement-figma-design
description: Use when implementing a Figma design, screen or component in an application that consumes the Decentraland component library. Triggers on a Figma design link, "implement this Figma", "build this screen from Figma", "which component is this in the library", "what props does this Figma component map to", or a dcl-figma resolve result. Resolves each Figma component to the real library component with the dcl-figma resolver instead of recreating it.
---

# Implement a Figma design with the real library

The library owns the components. Find the mapped component, import it and pass the props the mapping gives you. Never rebuild a component that exists, and never build a look-alike when a mapping is missing.

## Scope

Only components with a public mapping in `library/ui/components` resolve. That set is small and is not the set of components in the Figma file or in the Review gallery. Everything else is outside the supported scope: report it, do not reimplement it. A resolve error lists the components that do resolve in `details.supported`.

## Preflight

1. The resolver runs: in `library/ui`, `pnpm --silent figma resolve --mapping-only --node-id 0:0` prints JSON with `identity-uncertain`. Run it from `library/ui`, which pins its own pnpm version, not from the repository root. If `pnpm` or the `library/ui` install is missing, stop and tell the user (`pnpm install --frozen-lockfile` in `library/ui`). No access to figma-design-system is needed.
2. The official Figma MCP read tools are connected (`get_metadata`, `get_design_context`, `get_screenshot`). If they are not listed, stop and tell the user. Everything you do in Figma is read-only.

## Workflow

1. **Read the design.** Take the file and node from the link the user gave you, and open the file the way the connected Figma MCP's own instructions say (some versions require a session first). Keep the link and the file key in memory only: never write them to a file, commit, pull request, issue, log or report. For every component instance, note its main component node id, its variant properties (for example `Kind=Primary`, `Hover=No`) and the text it shows.
2. **Resolve each instance** with the CLI, from `library/ui`:

   ```sh
   pnpm --silent figma resolve --mapping-only --node-id <main component id> --variant "Kind=Primary" --text "Label=Buy"
   ```

   Repeat `--variant` and `--text` as needed. `--node-id` is preferred over `--name`. Without a capture only a component set id matches a mapping: pass the set id, not a variant id; a name lookup is refused. The command exits 0 with `ok: true`, or 1 with coded `errors`. List the instances you resolved when you report back, so the user can spot one you missed: the list comes from your reading of the design, the resolver does not walk the file.

3. **Read the answer.** `ok: true`: import from `component.importSpecifier` and copy the props from `usage` and `propTranslation`. `interactionStates` (Hover, Pressed) are styled by the component, not passed as props. Read every warning.
   Every diagnostic carries `severity`, `audience` and `action`; follow `action`. The codes, severities and audiences in this table are checked against the resolver's registry; `action` is the authority on what to do.

<!-- diagnostic-codes -->

| Code                          | Severity | Meaning                                                                              | What you do                                                                     |
| ----------------------------- | -------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------- |
| `invalid-input`               | error    | Neither a node id nor a name was given                                               | Pass `nodeId` (preferred) or the exact component set name                       |
| `mapping-root-not-configured` | error    | The resolver cannot find the mapping directory or its package                        | Pass `--mapping-root` or use `--mapping-only`                                   |
| `mapping-missing`             | error    | A known library component has no public mapping: outside the supported scope         | Do not build a replacement. Report name, variant properties, code and `context` |
| `identity-not-found`          | error    | The node or name is not a component of the captured library                          | Not a missing mapping. Follow `action` (it depends on `details.reason`)         |
| `identity-ambiguous`          | error    | Several components share the name, or several mappings claim the node                | Follow `action`; never pick a candidate yourself                                |
| `identity-uncertain`          | error    | No capture and no mapping: it cannot be established that this is a library component | Not a missing mapping. Follow `action`; do not build a replacement              |
| `variant-unsupported`         | error    | The mapping does not cover the value, or it is not a value the property takes        | Report it. Do not pick the closest value                                        |
| `variant-not-in-figma`        | error    | The combination is not defined by the design                                         | Ask the designer; do not invent the state                                       |
| `contract-incompatible`       | error    | Library version, contract version, Figma file or export does not match               | Follow `action`, which depends on `details.reason`                              |
| `unsupported-construct`       | error    | The mapping file uses something this resolver cannot read                            | Report it                                                                       |
| `unmapped-figma-property`     | warning  | The mapping ignores a Figma property or text layer                                   | Do not emulate it; mention it                                                   |
| `figma-drift`                 | warning  | Mapping and captured component differ in some options                                | Use only mapped values; mention the drift                                       |
| `export-unverifiable`         | warning  | The export cannot be confirmed statically                                            | Let the app typecheck confirm the import                                        |
| `capture-revision-unknown`    | warning  | The library lock pins no readable capture revision                                   | Report without a revision; tell a maintainer                                    |
| `figma-identity-unverified`   | warning  | Checked against the mapping only, without a capture                                  | Repeat it when you report the result                                            |

4. **Stop on `mapping-missing`, an identity code or `variant-unsupported`.** Do not duplicate the component in the app and do not add a stopgap. Report it to the contributor (the user), in this form and nothing more:

   ```text
   Missing library mapping
   Component: <name from the design>
   Code: <code>
   Variant properties: <Prop=Value, ...>
   Library: <provenance.library or details.library name@version>
   Capture revision: <details.captureRevision or provenance.capture.revision>
   ```

   No Figma link, file key or component key goes in it. Identity codes (`identity-not-found`, `identity-ambiguous`, `identity-uncertain`) say the component could not be identified; never report them as a missing mapping.

5. **Compose the page** with the resolved components and the shared `--dcl-*` tokens, following `docs/ui-standards.md` (CSS Modules, public imports only, where app-specific UI lives, reuse before duplication, translation parity). Public imports only: `import { X } from '<importSpecifier>'`, never a deep path. Routing, data, auth and tracking stay in the application.
6. **Report limits.** Repeat every warning and any accepted limit the result names, verbatim. Do not claim parity on those aspects.
7. **Verify.** Run the application's own checks. Then compare the rendered result with `get_screenshot` of the node and say what differs. The comparison is reported, not a gate. The library is generated from the pinned capture named by `provenance.capture.revision`, so attribute a difference to capture age before calling it a bug. Your verification is self-reported: say what you ran, and never claim visual fidelity from a successful build.

## The MCP server (optional)

The CLI is the primary path. A client without a shell can register the same resolver as an MCP server, outside the repository's tracked files:

```json
{
  "mcpServers": {
    "dcl-figma": {
      "command": "sh",
      "args": ["-c", "cd library/ui && exec pnpm --silent figma:mcp --mapping-only"]
    }
  }
}
```

With `--mapping-only` it registers only `figma_resolve_component` (inputs `nodeId`, `name`, `variantProperties`, `texts`) and `figma_check_mappings`, and returns the same results as the CLI.

## Keep this skill current

The source is `library/ui/figma/agent-kit/skills/implement-figma-design/SKILL.md`; the repository's copy in `.claude/skills/implement-figma-design/` is installed from it with `node library/ui/figma/agent-kit/cli.mjs install --target .`. CI runs `figma-agent-kit check`, which fails when the copy is edited by hand or the source changes without a reinstall.

`dcl-figma sync` turns the capture into Review-only preview code; a generated preview is not an implementation, do not import it from an application. Resolving reads the mappings and never writes code.
