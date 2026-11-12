type ExportSymbol = { key: string; raw: string }
type ExportStatement = { text: string; from: string; typeOnly: boolean; symbols: ExportSymbol[] }
const NAMED = /^[\t ]*export\s+(type\s+)?\{([^}]*)\}\s+from\s+(['"])([^'"]+)\3[\t ]*;?/gm

function exportStatements(text: string): ExportStatement[] {
  return [...text.matchAll(NAMED)].map((match) => ({
    text: match[0],
    from: match[4]!,
    typeOnly: !!match[1],
    symbols: match[2]!
      .split(',')
      .map((raw) => raw.trim())
      .filter(Boolean)
      .map((raw) => {
        const parsed = /^(type\s+)?([A-Za-z_$][\w$]*)(?:\s+as\s+([A-Za-z_$][\w$]*))?$/.exec(raw)
        if (!parsed) throw new Error(`Unsupported public export: ${raw}`)
        return { key: `${match[1] || parsed[1] ? 'type' : 'value'}:${parsed[3] ?? parsed[2]}`, raw }
      })
  }))
}

function publicExportSet(text: string): Set<string> {
  const found = new Set(
    exportStatements(text).flatMap((statement) => statement.symbols.map((symbol) => symbol.key))
  )
  for (const match of text.matchAll(/^export\s+\*\s+from\s+['"]([^'"]+)['"]/gm))
    found.add(`star:${match[1]}`)
  for (const match of text.matchAll(
    /^export\s+(?:declare\s+)?(type|interface|const|let|var|function|class|enum)\s+([A-Za-z_$][\w$]*)/gm
  ))
    found.add(`${['type', 'interface'].includes(match[1]!) ? 'type' : 'value'}:${match[2]}`)
  return found
}

function removePublicExports(text: string, keys: ReadonlySet<string>): string {
  let output = text
  for (const statement of exportStatements(text)) {
    const kept = statement.symbols.filter((symbol) => !keys.has(symbol.key))
    if (kept.length === statement.symbols.length) continue
    const replacement = kept.length
      ? `export ${statement.typeOnly ? 'type ' : ''}{ ${kept.map((symbol) => symbol.raw).join(', ')} } from '${statement.from}'`
      : ''
    output = output.replace(statement.text, replacement)
  }
  return output
}

export { exportStatements, publicExportSet, removePublicExports }
