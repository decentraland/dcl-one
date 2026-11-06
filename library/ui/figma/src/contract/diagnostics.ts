type DiagnosticCode =
  | 'invalid-input'
  | 'mapping-root-not-configured'
  | 'mapping-missing'
  | 'mapping-unreadable'
  | 'identity-not-found'
  | 'identity-ambiguous'
  | 'variant-unsupported'
  | 'variant-not-in-figma'
  | 'contract-incompatible'
  | 'unsupported-construct'
  | 'unmapped-figma-property'
  | 'figma-drift'
  | 'export-unverifiable'
  | 'figma-identity-unverified'
  | 'superseded-by-sync'
  | 'superseded-pending'

type Diagnostic = {
  code: DiagnosticCode
  severity: 'error' | 'warning'
  message: string
  // Machine-readable specifics, e.g. the incompatibility reason or the candidates of an ambiguity.
  details?: Record<string, unknown>
}

function diagnostic(
  code: DiagnosticCode,
  severity: Diagnostic['severity'],
  message: string,
  details?: Record<string, unknown>
): Diagnostic {
  return details ? { code, severity, message, details } : { code, severity, message }
}

const hasErrors = (diagnostics: Diagnostic[]) => diagnostics.some((d) => d.severity === 'error')

export { diagnostic, hasErrors }
export type { Diagnostic, DiagnosticCode }
