import { existsSync } from 'node:fs'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import semver from 'semver'
import { diagnostic, type Diagnostic } from './diagnostics.js'
import type { FigmaSet } from './figma-index.js'
import type { LibraryInfo } from './library.js'
import { FIGMA_FILE_PLACEHOLDER, SUPPORTED_CONTRACT, type ParsedMapping } from './mapping-file.js'

type ValidationContext = {
  library: LibraryInfo
  libraryRange?: string
}

const incompatible = (reason: string, message: string, details: Record<string, unknown> = {}) =>
  diagnostic('contract-incompatible', 'error', message, { reason, ...details })

function importSpecifier(mapping: ParsedMapping): string | undefined {
  return mapping.imports
    .map((statement) => /from\s+['"]([^'"]+)['"]/.exec(statement)?.[1])
    .find((specifier) => specifier !== undefined)
}

async function validateAgainstLibrary(
  mapping: ParsedMapping,
  { library, libraryRange }: ValidationContext
): Promise<Diagnostic[]> {
  const found: Diagnostic[] = [...mapping.issues]
  const rel = path.relative(library.rootDir, mapping.file)

  if (mapping.header.contract !== String(SUPPORTED_CONTRACT)) {
    found.push(
      incompatible(
        'contract-version',
        `${rel} declares contract=${mapping.header.contract ?? 'none'}, this MCP reads contract=${SUPPORTED_CONTRACT}`
      )
    )
  }
  if (mapping.figma && mapping.figma.fileKey !== FIGMA_FILE_PLACEHOLDER) {
    found.push(
      incompatible(
        'wrong-figma-file',
        `${rel} must name its Figma file through the ${FIGMA_FILE_PLACEHOLDER} placeholder, not a literal Figma URL`
      )
    )
  }
  if (libraryRange) {
    if (!semver.validRange(libraryRange)) {
      found.push(incompatible('library-version', `Invalid library range "${libraryRange}"`))
    } else if (!semver.satisfies(library.version, libraryRange)) {
      found.push(
        incompatible(
          'library-version',
          `${library.name}@${library.version} does not satisfy the required range ${libraryRange}`,
          { installed: library.version, required: libraryRange }
        )
      )
    }
  }

  const specifier = importSpecifier(mapping)
  const component = mapping.header.component
  if (specifier !== library.name) {
    found.push(
      incompatible(
        'import-path-mismatch',
        `${rel} imports from "${specifier ?? 'nothing'}" but the owning package is "${library.name}"`
      )
    )
  }
  if (
    component &&
    !mapping.imports.some((statement) => new RegExp(`\\b${component}\\b`).test(statement))
  ) {
    found.push(
      incompatible('import-path-mismatch', `${rel} never imports the component "${component}"`)
    )
  }

  if (mapping.header.source && !existsSync(path.resolve(library.rootDir, mapping.header.source))) {
    found.push(
      incompatible('source-missing', `${rel}: source file ${mapping.header.source} does not exist`)
    )
  }
  if (component && library.entryFile && existsSync(library.entryFile)) {
    const entry = await readFile(library.entryFile, 'utf8')
    if (!new RegExp(`\\b${component}\\b`).test(entry)) {
      if (/export\s+\*/.test(entry)) {
        found.push(
          diagnostic(
            'export-unverifiable',
            'warning',
            `${rel}: the package entry uses "export *", ${component} cannot be confirmed as a public export statically`
          )
        )
      } else {
        found.push(
          incompatible(
            'export-missing',
            `${component} is not exported from the package entry of ${library.name}`
          )
        )
      }
    }
  }
  return found
}

// Compares what the template reads with what the captured Figma component defines. Differences are drift
// that a library maintainer has to resolve, never something the MCP papers over.
function validateAgainstFigma(mapping: ParsedMapping, set: FigmaSet): Diagnostic[] {
  const found: Diagnostic[] = []
  const rel = path.basename(mapping.file)
  const read = new Set<string>()

  for (const local of mapping.locals) {
    if (local.kind === 'text') continue
    read.add(local.figmaProperty)
    const property = set.properties[local.figmaProperty]
    if (!property) {
      found.push(
        incompatible(
          'figma-drift',
          `${rel} reads Figma property "${local.figmaProperty}" which "${set.name}" no longer defines`,
          { property: local.figmaProperty, figmaProperties: Object.keys(set.properties) }
        )
      )
      continue
    }
    if (local.kind === 'enum' && property.options) {
      const uncovered = property.options.filter((option) => !(option in local.values))
      const stale = Object.keys(local.values).filter((key) => !property.options!.includes(key))
      if (uncovered.length > 0) {
        found.push(
          diagnostic(
            'figma-drift',
            'warning',
            `${rel}: Figma options of "${local.figmaProperty}" not covered by the mapping: ${uncovered.join(', ')}`,
            { property: local.figmaProperty, uncovered }
          )
        )
      }
      if (stale.length > 0) {
        found.push(
          diagnostic(
            'figma-drift',
            'warning',
            `${rel}: mapping lists options of "${local.figmaProperty}" that Figma no longer has: ${stale.join(', ')}`,
            { property: local.figmaProperty, stale }
          )
        )
      }
    }
  }

  for (const state of mapping.header.states) {
    read.add(state)
    if (!set.properties[state]) {
      found.push(
        diagnostic(
          'figma-drift',
          'warning',
          `${rel}: declared state "${state}" is not a Figma property of "${set.name}"`,
          {
            state
          }
        )
      )
    }
  }

  for (const [name, property] of Object.entries(set.properties)) {
    if (!read.has(name)) {
      found.push(
        diagnostic(
          'unmapped-figma-property',
          'warning',
          `${rel} does not read Figma property "${name}" (${property.type}). Map it or declare it in "// states=".`,
          { property: name, type: property.type }
        )
      )
    }
  }
  return found
}

export { importSpecifier, validateAgainstFigma, validateAgainstLibrary }
export type { ValidationContext }
