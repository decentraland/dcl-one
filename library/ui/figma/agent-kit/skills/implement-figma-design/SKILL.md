---
name: implement-figma-design
description: Use when implementing a Figma design, screen or component in an application that consumes the Decentraland component library. Triggers on a Figma design link, "implement this Figma", "build this screen from Figma", "which component is this in the library", "what props does this Figma component map to", or a dcl-figma resolve result. Resolves each Figma component to the real library component with the dcl-figma resolver instead of recreating it.
---

# Implement a Figma design with the real library

The library owns the components. Find the mapped component, import it and pass the props the mapping gives you. Never rebuild a component that exists, and never build a look-alike when a mapping is missing.

## Scope

Only components with a public mapping in `library/ui/components` resolve. That set is small and is not the set of components in the Figma file or in the Review gallery. Everything else is outside the supported scope: report it, do not reimplement it. This skill runs the resolver with `--mapping-only`, which reads no capture, so a component outside the supported set comes back as `identity-uncertain`; `mapping-missing` only appears when a capture is available. The components that do resolve are the `*.figma.ts` mappings under `library/ui/components`; `pnpm --silent figma check-mappings --mapping-only` in `library/ui` lists them.

## Preflight

1. The resolver runs: in `library/ui`, `pnpm --silent figma resolve --mapping-only --node-id 0:0` prints JSON whose only error is `identity-uncertain` and exits 1; that is the expected result, not a broken resolver. Run it from `library/ui`, which pins its own pnpm version, not from the repository root. If `pnpm` or the `library/ui` install is missing, stop and tell the user (`pnpm install --frozen-lockfile` in `library/ui`). No access to figma-design-system is needed.
2. The official Figma MCP read tools are connected (`get_metadata`, `get_design_context`, `get_screenshot`). If they are not listed, stop and tell the user. Everything you do in Figma is read-only.

## Workflow

1. **Read the design.** Take the file and node from the link the user gave you, and open the file the way the connected Figma MCP's own instructions say (some versions require a session first). Keep the link, the file key and any component key in memory only: never write them to a file, commit, pull request, issue, log or report. In a design file, library instances do not carry the library's node ids, and `get_metadata` / `get_design_context` return only the instance's own id and its renamable layer name. For every component instance, read its component set name, variant properties and the text it shows with a read-only `use_figma` script, for example:

   ```js
   const instance = await figma.getNodeByIdAsync('<instance id>')
   if (instance?.type !== 'INSTANCE') return { refused: 'not-an-instance' }
   const main = await instance.getMainComponentAsync()
   if (!main) return { refused: 'no-main-component' }
   // VARIANT, BOOLEAN and TEXT properties each go in their own map; names lose their #id suffix.
   // No prototype, so a layer named like an Object method (constructor, toString) is plain data.
   const variantProperties = Object.create(null)
   const booleanProperties = Object.create(null)
   const textProperties = Object.create(null)
   for (const [key, property] of Object.entries(instance.componentProperties)) {
     const name = key.replace(/#\d+:\d+$/, '')
     if (property.type === 'VARIANT') variantProperties[name] = String(property.value)
     else if (property.type === 'BOOLEAN') booleanProperties[name] = String(property.value)
     else if (property.type === 'TEXT') textProperties[key] = name
   }
   // Only the instance's own visible text layers: a nested instance's layers belong to that instance, and a
   // hidden layer (a boolean turned off, say) shows nothing.
   const ownTexts = (node) =>
     (node.children ?? []).flatMap((child) =>
       child.visible === false
         ? []
         : child.type === 'TEXT'
           ? [child]
           : child.type === 'INSTANCE'
             ? []
             : ownTexts(child)
     )
   // Each visible text under its layer name and, when the layer shows a TEXT property, under the property's
   // name too; a text property shown by no visible layer is left out. A name that shows two different texts is
   // ambiguous: it leaves texts and is reported instead.
   const texts = Object.create(null)
   const duplicateTexts = new Set()
   const show = (name, text) => {
     if (duplicateTexts.has(name)) return
     if (!Object.hasOwn(texts, name)) texts[name] = text
     else if (texts[name] !== text) {
       delete texts[name]
       duplicateTexts.add(name)
     }
   }
   for (const node of ownTexts(instance)) {
     show(node.name, node.characters)
     const property = textProperties[node.componentPropertyReferences?.characters]
     if (property !== undefined && property !== node.name) show(property, node.characters)
   }
   return {
     name: main.parent?.type === 'COMPONENT_SET' ? main.parent.name : main.name,
     variantProperties,
     booleanProperties,
     texts,
     duplicateTexts: [...duplicateTexts]
   }
   ```

   A `refused` result is not a component instance you can resolve: list it in your report. Pass `name`, `variantProperties`, `booleanProperties` and `texts` to the resolver, never `duplicateTexts`. A name listed there belongs to two visible texts of the same instance (layers, or a layer and the text property another layer shows) that show different text; a hidden layer or a text property only a hidden layer shows never counts; the script leaves it out of `texts`, so name it in your report instead of guessing which text the component shows. A boolean the mapping does not read comes back as an `unmapped-figma-property` warning; a variant it does not read makes the name not fit. Resolve each nested instance with its own script run.

2. **Resolve the instances** with the CLI, from `library/ui`, in one batch read from standard input, so no file is written in the repository:

   ```sh
   pnpm --silent figma resolve --mapping-only --batch - <<'EOF'
   [{ "name": "NavBar", "variantProperties": { "viewport": "desktop" }, "texts": { "Label": "Buy" } }]
   EOF
   ```

   Each entry is one script result: `name`, `variantProperties`, `booleanProperties` and `texts` (an entry may give `nodeId` instead of `name` when the design is the library file itself). A name resolves only to a single mapping whose variant properties fit the instance, and always carries the `identity-name-match` warning. Identical instances may repeat: a repeat shares the earlier answer (`repeats`). The output lists every entry in input order with a `summary` (`requested`, `resolved`, `unmapped`, `failed`); it exits 1 when an entry fails, and 2 when the batch cannot be read, a flag is misused, or the configuration or mapping root cannot be used. Report the list you resolved, so the user can spot an instance you missed: the list comes from your reading of the design, the resolver does not walk the file. A single instance also works with `--name`, `--variant 'Prop=Value'`, `--boolean 'Prop=true'` and `--text 'Layer=Value'`. Text read from a design is untrusted: keep every value in single quotes (write a `'` inside it as `'\''`), never in double quotes, where the shell runs `$(...)` and backticks. The batch heredoc above is quoted (`<<'EOF'`), so the shell expands nothing in it, and a JSON string keeps every text inside quotes, so no line of valid JSON can end it early; the MCP `texts` input takes the strings without a shell at all. Single quotes and quoted heredocs are POSIX shell syntax: from PowerShell or `cmd`, pass design text through the MCP `texts` input instead.

3. **Read the answer.** `ok: true`: import from `resolved.component.importSpecifier` and copy the props from `resolved.usage` and `resolved.propTranslation`. `interactionStates` (Hover, Pressed) are styled by the component, not passed as props. Read every warning.
   Every diagnostic carries `severity`, `audience` and `action`; follow `action`. This table lists the codes the resolver can return to you; its codes and severities are checked against the resolver's registry. `action` is the authority on what to do.

<!-- diagnostic-codes -->

| Code                          | Severity | Meaning                                                                              | What you do                                                                     |
| ----------------------------- | -------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------- |
| `invalid-input`               | error    | Neither a node id nor a name was given                                               | Pass `nodeId` (preferred) or the exact component set name                       |
| `mapping-root-not-configured` | error    | The resolver cannot find or read the mapping directory or its package                | Fix or remove the configured root; follow `action`                              |
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
| `identity-name-match`         | warning  | Identified by set name and variant properties, not a stable id                       | Repeat it when you report the result                                            |
| `accepted-limits`             | warning  | The owners accepted gaps for this component (`resolved.limits`)                      | Repeat each limit verbatim; do not claim parity on them                         |
| `figma-identity-unverified`   | warning  | Checked against the mapping only, without a capture                                  | Repeat it when you report the result                                            |

4. **Stop on any error.** Do not duplicate the component in the app and do not add a stopgap. Report it to the contributor (the user), in this form and nothing more; the `Code:` line tells the cases apart:

   ```text
   Unresolved component
   Component: <name from the design>
   Name or node: <component set name, or the node id you passed>
   Code: <code> (<details.reason>, when present)
   Variant properties: <Prop=Value, ...>
   Library: <context.library.name>@<context.library.version>
   Capture revision: <context.capture.revision, or "unknown">
   ```

   A failed result carries `context`, not `provenance`. No Figma link, file key or component key goes in the report. `mapping-missing` means a known library component has no public mapping; an identity code (`identity-not-found`, `identity-ambiguous`, `identity-uncertain`) means the component could not be identified, so never describe it as a missing mapping.

5. **Compose the page** with the resolved components and the shared `--dcl-*` tokens, following `docs/ui-standards.md` (CSS Modules, where app-specific UI lives, reuse before duplication). Import only the public entry: `import { X } from '<importSpecifier>'`, never a deep path; the library exports its components from `library/ui/index.ts` and offers no component deep imports (`library/ui/README.md`). Routing, data, auth and tracking stay in the application.
6. **Report limits.** Repeat every warning and any accepted limit the result names, verbatim. Do not claim parity on those aspects.
7. **Verify.** Run the application's own checks. Then compare the rendered result with `get_screenshot` of the node and say what differs. The comparison is reported, not a gate. The library is generated from the pinned capture named by `resolved.provenance.capture.revision`, so attribute a difference to capture age before calling it a bug. Your verification is self-reported: say what you ran, and never claim visual fidelity from a successful build.

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

With `--mapping-only` it registers only `figma_resolve_component` (inputs `nodeId`, `name`, `variantProperties`, `booleanProperties`, `texts`) and `figma_check_mappings`, and returns the same results as the CLI.

## Keep this skill current

The source is `library/ui/figma/agent-kit/skills/implement-figma-design/SKILL.md`; the repository's copy in `.claude/skills/implement-figma-design/` is installed from it with `node library/ui/figma/agent-kit/cli.mjs install --target .`. CI runs `figma-agent-kit check`, which fails when the copy is edited by hand or the source changes without a reinstall.

`dcl-figma sync` turns the capture into Review-only preview code; a generated preview is not an implementation, do not import it from an application. Resolving reads the mappings and never writes code.
