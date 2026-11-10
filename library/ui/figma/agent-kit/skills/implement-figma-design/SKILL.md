---
name: implement-figma-design
description: Use when implementing a Figma design, screen or component in an application that consumes the Decentraland component library. Triggers on a figma.com design URL, "implement this Figma", "build this screen from Figma", "which component is this in the library", "what props does this Figma component map to", or a figma_resolve_component result. Resolves each Figma component to the real library component through the dcl-figma MCP instead of recreating it.
---

# Implement a Figma design with the real library

The library owns the components. Your job is to find the mapped component, import it and pass the props the mapping gives you. Never rebuild a component that exists, and never invent a look-alike when a mapping is missing.

## Requirements

- The `dcl-figma` MCP server is registered in this repository (see "Register the MCP"). If its tools are not listed, stop and tell the user: installing the library does not register the server.
- The official Figma MCP is connected for reading the design (`get_metadata`, `get_design_context`, `get_screenshot`). Everything you do on Figma is read-only.

## Workflow

1. **Read the design.** Call `create_session` with the file key, then `get_metadata` / `get_design_context` for the node. Note, for every component instance, its main component node id and its variant properties (for example `Property 1=Terciary`, `Hover=No`).
2. **Resolve each component.** Call `figma_resolve_component` with `nodeId` (preferred over `name`), `variantProperties` and `texts` (labels the design shows).
3. **Read the answer.**
   - `ok: true`: import from `component.importSpecifier`, copy the props from `usage` and `propTranslation`. Read `warnings`; they are drift or unverified identity, not noise. `interactionStates` (Hover, Pressed) are styled by the component, not passed as props.
   - `ok: false`: act on the code, never work around it.

   Every diagnostic carries `severity`, `audience` and `action`; follow `action`. The table is checked against the resolver's registry.

| Code                          | Severity | Meaning                                                                              | What you do                                                                   |
| ----------------------------- | -------- | ------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------- |
| `invalid-input`               | error    | Neither a node id nor a name was given                                               | Pass `nodeId` (preferred) or the exact component set name                     |
| `mapping-root-not-configured` | error    | The resolver cannot find the mapping directory or its package                        | Pass `--mapping-root` or use `--mapping-only`                                 |
| `mapping-missing`             | error    | A known library component has no public mapping: outside the supported scope         | Do not build a replacement. Report name, variant properties, code, provenance |
| `identity-not-found`          | error    | The node or name is not a component of the captured library                          | Check the instance comes from the library; pass its main component node id    |
| `identity-ambiguous`          | error    | Several components share the name, or several mappings claim the node                | Call again with a `nodeId` from `details.candidates`                          |
| `identity-uncertain`          | error    | No capture and no mapping: it cannot be established that this is a library component | Not a missing mapping. Report it; do not build a replacement                  |
| `variant-unsupported`         | error    | The design uses a value the mapping does not cover                                   | Report it. Do not pick the closest value                                      |
| `variant-not-in-figma`        | error    | The combination is not defined by the design                                         | Ask the designer; do not invent the state                                     |
| `contract-incompatible`       | error    | Library version, contract version, Figma file or export does not match               | Follow `action`, which depends on `details.reason`                            |
| `unsupported-construct`       | error    | The mapping file uses something this resolver cannot read                            | Report it                                                                     |
| `unmapped-figma-property`     | warning  | The mapping ignores a Figma property                                                 | Do not emulate it; mention it                                                 |
| `figma-drift`                 | warning  | Mapping and captured component differ in some options                                | Use only mapped values; mention the drift                                     |
| `export-unverifiable`         | warning  | The export cannot be confirmed statically                                            | Let the app typecheck confirm the import                                      |
| `figma-identity-unverified`   | warning  | Checked against the mapping only, without a capture                                  | Repeat it when you report the result                                          |

4. **Compose the page** from the resolved components and the library's design tokens (CSS custom properties such as `var(--dcl-space-12)`). No hard-coded colors, spacing or fonts that a token covers. Public imports only: `import { X } from '<importSpecifier>'`, never a deep path.
5. **Keep business behavior in the app.** The library renders; routing, data, auth and tracking stay in the application.
6. **Verify.** Run the application's own checks. Then compare the rendered result with `get_screenshot` of the Figma node and say exactly what differs. Do not claim visual fidelity from a successful build.

## Repository rules that still apply

The consuming repository's own rules win over a visual match. In dcl-one that means `docs/ui-standards.md`: co-located CSS Modules with the shared `--dcl-*` tokens, no CSS-in-JS, app-specific UI in `library/ui/apps/<app>/`, reuse before duplication, and translation parity across locales. This skill adds no routing, auth or coverage policy.

## Register the MCP

In dcl-one the library and its tooling are in the same tree; register the server in the repository's `.mcp.json` (no personal paths):

```json
{
  "mcpServers": {
    "dcl-figma": {
      "command": "pnpm",
      "args": ["--silent", "--dir", "library/ui", "figma:mcp", "--config", "figma.config.json"]
    }
  }
}
```

With `--config` the server verifies identity and variants against the saved Figma capture and reads the mappings from `library/ui/components`. Resolving never needs the Figma file key; only live Figma reads do (`FIGMA_FILE_KEY`, never written to a file). Codex reads its own MCP configuration; this file does not register the server there.

## Keep this skill current

This skill ships inside the library under `agent-kit/` and is copied into the repository with `figma-agent-kit install`. After upgrading the library, run `figma-agent-kit install`; in CI run `figma-agent-kit check` (exit 1 when the installed skill is stale or edited).
