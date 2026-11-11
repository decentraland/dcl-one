import { z } from 'zod'
import { checkMappingsReport, resolveComponent } from '../contract/resolve.js'
import type { Runtime } from '../runtime.js'
import { tool, type ToolDefinition } from './index.js'

// Resolving an existing component and generating one are separate operations on purpose: this file only
// reads the library's mapping, it never writes code.
function createContractTools(runtime: Runtime): ToolDefinition[] {
  return [
    tool({
      name: 'figma_resolve_component',
      description:
        'Resolve a Figma component to the REAL React component of the component library: import statement, exported name, how Figma properties translate to props, a usage snippet and provenance (Figma node, mapping file, library version, and the pinned capture revision in provenance.capture). Reads the library mapping (*.figma.ts). Returns ok:false with coded errors instead of guessing; every error and warning carries severity, audience and an action to follow, and a failure carries context (library name and version, capture revision) for a report. mapping-missing means a known library component has no public mapping; identity-not-found, identity-ambiguous and identity-uncertain mean the component could not be identified (a component set name resolves only when its variantProperties fit a single mapping). It never generates a replacement component.',
      inputSchema: {
        nodeId: z
          .string()
          .optional()
          .describe(
            'Figma node id of the component set, of one of its variants, or of the main component of an instance (e.g. "14113:6739" or "14113-6739"). Preferred over name. In a design file library instances do not carry the library node ids: pass name and variantProperties instead.'
          ),
        name: z
          .string()
          .optional()
          .describe(
            'Exact Figma component set name (not the instance layer name). Without a capture it resolves only to a single mapping whose variant properties fit variantProperties. Rejected as ambiguous when several sets or mappings share it.'
          ),
        variantProperties: z
          .record(z.string(), z.string())
          .optional()
          .describe(
            'Figma variant properties of the instance, e.g. {"Property 1":"Terciary","Hover":"No"}.'
          ),
        texts: z
          .record(z.string(), z.string())
          .optional()
          .describe('Text for text layers or TEXT properties, keyed by layer or property name.')
      },
      handler: async (input) => resolveComponent(await runtime.resolveContext(), input)
    }),
    tool({
      name: 'figma_check_mappings',
      description:
        'Validate every *.figma.ts mapping of the configured library: contract version, Figma file, library version range, import path, source and public export, and drift against the captured Figma component. Returns captureChecked and a capture-unchecked warning when no capture was read, and ok:false with mapping-root-not-configured when there is no mapping source.',
      inputSchema: {},
      handler: async () => checkMappingsReport(await runtime.resolveContext())
    })
  ]
}

export { createContractTools }
