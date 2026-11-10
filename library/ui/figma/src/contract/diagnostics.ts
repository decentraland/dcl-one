type Severity = 'error' | 'warning'

// `consumer` codes can come back from resolving a component; `maintainer` codes only from check-mappings.
type Audience = 'consumer' | 'maintainer'

type DiagnosticSpec = {
  severity: Severity
  audience: Audience
  // What the reader does next. A map is keyed by `details.reason` and must name every reason the code emits.
  action: string | Readonly<Record<string, string>>
}

const REPORT =
  'Report the component name, its variant properties, this code and provenance (library version, capture revision) to the contributor; never include a Figma URL or key.'

// The single list of diagnostic codes. SKILL.md and MAINTENANCE.md are checked against it by
// src/contract/diagnostics-docs.test.ts, so a new code fails until both documents list it.
const DIAGNOSTICS = {
  'invalid-input': {
    severity: 'error',
    audience: 'consumer',
    action: 'Pass a nodeId (preferred) or an exact Figma component set name, then resolve again.'
  },
  'mapping-root-not-configured': {
    severity: 'error',
    audience: 'consumer',
    action:
      'Point --mapping-root (or DCL_FIGMA_MAPPING_ROOT, or contract.mappingRoot in figma.config.json) at the mapping directory inside the library package. With --mapping-only the default is the library components directory.'
  },
  'mapping-missing': {
    severity: 'error',
    audience: 'consumer',
    action: `The component is a known library component without a public mapping, so it is outside the supported scope. Do not build a replacement or a look-alike. ${REPORT} details.supported lists the components that do resolve.`
  },
  'identity-not-found': {
    severity: 'error',
    audience: 'consumer',
    action: {
      name: 'No captured component has this name. Check the instance comes from the library and resolve again with its component set nodeId; this is an identity problem, not a missing mapping.',
      node: 'The node is not a component of the captured library. Check the instance comes from the library and resolve again with its component set nodeId; this is an identity problem, not a missing mapping.',
      'mapping-node':
        'A mapping targets a node that is not in the capture. Point the mapping at the captured component set, or retire it.'
    }
  },
  'identity-ambiguous': {
    severity: 'error',
    audience: 'consumer',
    action: {
      name: 'Several captured components share this name. Resolve again with a nodeId from details.candidates; never pick one yourself.',
      mappings: `Several mappings claim this node, which is a library defect. Do not pick one. ${REPORT}`,
      'duplicate-node':
        'Two mappings in the library target the same Figma node. Keep one mapping per component set.',
      'duplicate-id':
        'Two mappings in the library share a template id. Give each mapping its own id.'
    }
  },
  'identity-uncertain': {
    severity: 'error',
    audience: 'consumer',
    action: {
      name: 'Without a capture a component cannot be found by name. Resolve again with the component set nodeId from the design.',
      node: `No capture is available and no mapping names this node, so it cannot be established whether it is a library component. Without a capture only a component set id matches a mapping: if you passed a variant id, resolve again with its component set id. Otherwise do not treat it as a missing mapping and do not build a replacement. ${REPORT}`
    }
  },
  'capture-revision-unknown': {
    severity: 'warning',
    audience: 'consumer',
    action:
      'The library lock pins no readable capture revision, so the result names none. Report the result without a capture revision and tell a maintainer that library/ui/upstream.lock.json is unreadable.'
  },
  'variant-unsupported': {
    severity: 'error',
    audience: 'consumer',
    action: `The mapping does not cover this variant value. Do not pick the closest value. ${REPORT}`
  },
  'variant-not-in-figma': {
    severity: 'error',
    audience: 'consumer',
    action:
      'The design names a property or combination the component does not define. Ask the designer; do not invent the state.'
  },
  'contract-incompatible': {
    severity: 'error',
    audience: 'consumer',
    action: {
      'contract-version': `The mapping declares a contract version this resolver does not read. ${REPORT}`,
      'wrong-figma-file': `The mapping names a literal Figma URL instead of the placeholder. ${REPORT}`,
      'library-version':
        'The installed library does not satisfy the range given by --library-range, DCL_FIGMA_LIBRARY_RANGE or contract.libraryRange. Update the library or the range; reinstalling the skill does not change it.',
      'import-path-mismatch': `The mapping imports from another package than the one that owns it. ${REPORT}`,
      'source-missing': `The mapping names a source file that does not exist. ${REPORT}`,
      'export-missing': `The mapped component is not a public export of the library. ${REPORT}`,
      'figma-drift': `The mapping reads a Figma property the captured component no longer defines. ${REPORT}`
    }
  },
  'unsupported-construct': {
    severity: 'error',
    audience: 'consumer',
    action: `The mapping file uses a construct this reader does not support. ${REPORT}`
  },
  'unmapped-figma-property': {
    severity: 'warning',
    audience: 'consumer',
    action:
      'The mapping ignores this Figma property. Do not emulate it in the app; mention it when you report the result.'
  },
  'figma-drift': {
    severity: 'warning',
    audience: 'consumer',
    action:
      'The mapping and the captured component differ in the listed options. Use only the values the mapping supports and mention the drift when you report the result.'
  },
  'export-unverifiable': {
    severity: 'warning',
    audience: 'consumer',
    action:
      'The export could not be confirmed statically. Let the application typecheck confirm the import.'
  },
  'figma-identity-unverified': {
    severity: 'warning',
    audience: 'consumer',
    action:
      'Identity and variants were checked against the mapping only, not against a capture. Repeat this when you report the result; differences from the live design may come from capture age.'
  },
  'superseded-by-sync': {
    severity: 'warning',
    audience: 'maintainer',
    action:
      'The synced component named by promotion.include replaces this hand-written mapping on the next promotion; no action unless the replacement should not land.'
  },
  'superseded-pending': {
    severity: 'warning',
    audience: 'maintainer',
    action:
      'The hand-written mapping stays until its synced replacement can be promoted; resolve the listed reasons in figma-design-system or keep the mapping.'
  }
} as const satisfies Record<string, DiagnosticSpec>

type DiagnosticCode = keyof typeof DIAGNOSTICS

type Diagnostic = {
  code: DiagnosticCode
  severity: Severity
  audience: Audience
  message: string
  action: string
  // Machine-readable specifics, e.g. the incompatibility reason or the candidates of an ambiguity.
  details?: Record<string, unknown>
}

function actionOf(code: DiagnosticCode, details?: Record<string, unknown>): string {
  const { action } = DIAGNOSTICS[code] as DiagnosticSpec
  if (typeof action === 'string') return action
  const reason = details?.reason
  if (typeof reason !== 'string' || !Object.hasOwn(action, reason))
    throw new Error(`Diagnostic ${code} has no action for reason ${String(reason)}`)
  return action[reason]!
}

function diagnostic(
  code: DiagnosticCode,
  message: string,
  details?: Record<string, unknown>
): Diagnostic {
  const { severity, audience } = DIAGNOSTICS[code]
  const action = actionOf(code, details)
  return details
    ? { code, severity, audience, message, action, details }
    : { code, severity, audience, message, action }
}

const hasErrors = (diagnostics: Diagnostic[]) => diagnostics.some((d) => d.severity === 'error')

export { DIAGNOSTICS, diagnostic, hasErrors }
export type { Audience, Diagnostic, DiagnosticCode, DiagnosticSpec, Severity }
