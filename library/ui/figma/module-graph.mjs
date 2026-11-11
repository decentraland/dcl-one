// module-graph: the static import closure of the component installer, computed without dependencies so the offline
// guard can recompute it. It reads JavaScript and TypeScript (not JSX), and fails closed on every import it cannot
// resolve to exactly one tracked file or a sanctioned package: a non-literal dynamic import, require, an
// extensionless, query or escaped specifier, a path outside its scope, a test, fixture or Markdown target, a missing
// or ambiguous file, a bare specifier that is not a `node:` built-in or a sanctioned package, and any text it cannot
// tokenize. The installed contents of sanctioned packages are bound by the dependency manifests, not here.
import { posix } from 'node:path'

const KEYWORDS_BEFORE_EXPRESSION = new Set([
  'return',
  'typeof',
  'instanceof',
  'in',
  'of',
  'new',
  'delete',
  'void',
  'throw',
  'case',
  'do',
  'else',
  'yield',
  'await',
  'extends',
  'satisfies',
  'as'
])
const IDENTIFIER_START = /[\p{ID_Start}$_\\]/u
const IDENTIFIER_PART = /[\p{ID_Continue}$\\]/u
const PUNCTUATORS = [
  '>>>=',
  '...',
  '===',
  '!==',
  '**=',
  '<<=',
  '>>=',
  '>>>',
  '&&=',
  '||=',
  '??=',
  '=>',
  '==',
  '!=',
  '<=',
  '>=',
  '&&',
  '||',
  '??',
  '?.',
  '++',
  '--',
  '+=',
  '-=',
  '*=',
  '/=',
  '%=',
  '&=',
  '|=',
  '^=',
  '**',
  '<<',
  '>>'
]
const CLOSERS = { '(': ')', '[': ']', '{': '}' }

class GraphError extends Error {}
const fail = (file, message) => {
  throw new GraphError(`${message} in ${file}`)
}

/**
 * Tokens of one module: identifiers, numbers, string and template pieces, regular expressions and punctuators,
 * without comments or whitespace. Throws on an unterminated literal or comment and on unbalanced brackets, so a
 * tokenizer mistake surfaces as an error instead of a hidden import.
 * @param {string} text
 * @param {string} file
 * @returns {{ type: 'name' | 'number' | 'string' | 'template' | 'regex' | 'punct', value: string }[]}
 */
export function tokenize(text, file) {
  const tokens = []
  // Each open bracket, plus 'template' for a `${` whose closing brace resumes a template literal.
  const stack = []
  let at = 0
  const previous = () => tokens[tokens.length - 1]
  const regexAllowed = () => {
    const last = previous()
    if (!last) return true
    if (last.type === 'name') return KEYWORDS_BEFORE_EXPRESSION.has(last.value)
    if (last.type === 'punct') return ![')', ']', '}'].includes(last.value)
    return false
  }
  const readTemplate = () => {
    // `at` is just after a backtick or the `}` that closes a substitution.
    let value = ''
    while (at < text.length) {
      const character = text[at]
      if (character === '\\') {
        value += text.slice(at, at + 2)
        at += 2
      } else if (character === '`') {
        at += 1
        tokens.push({ type: 'template', value, tail: true })
        return
      } else if (character === '$' && text[at + 1] === '{') {
        at += 2
        tokens.push({ type: 'template', value, tail: false })
        stack.push('template')
        return
      } else {
        value += character
        at += 1
      }
    }
    fail(file, 'Unterminated template literal')
  }
  while (at < text.length) {
    const character = text[at]
    if (/\s/.test(character) || character === '﻿') {
      at += 1
    } else if (character === '/' && text[at + 1] === '/') {
      const end = text.indexOf('\n', at)
      at = end < 0 ? text.length : end
    } else if (character === '/' && text[at + 1] === '*') {
      const end = text.indexOf('*/', at + 2)
      if (end < 0) fail(file, 'Unterminated comment')
      at = end + 2
    } else if (character === '#' && at === 0 && text[1] === '!') {
      const end = text.indexOf('\n')
      at = end < 0 ? text.length : end
    } else if (character === '"' || character === "'") {
      let value = ''
      at += 1
      for (;;) {
        if (at >= text.length || text[at] === '\n') fail(file, 'Unterminated string literal')
        if (text[at] === character) break
        if (text[at] === '\\') {
          value += text.slice(at, at + 2)
          at += 2
        } else value += text[at++]
      }
      at += 1
      tokens.push({ type: 'string', value })
    } else if (character === '`') {
      at += 1
      readTemplate()
    } else if (character === '/' && regexAllowed()) {
      let inClass = false
      at += 1
      for (;;) {
        if (at >= text.length || text[at] === '\n') fail(file, 'Unterminated regular expression')
        const current = text[at]
        if (current === '\\') at += 2
        else {
          at += 1
          if (current === '[') inClass = true
          else if (current === ']') inClass = false
          else if (current === '/' && !inClass) break
        }
      }
      while (at < text.length && /[a-z]/i.test(text[at])) at += 1
      tokens.push({ type: 'regex', value: '' })
    } else if (IDENTIFIER_START.test(character) || character === '#') {
      const start = at
      at += 1
      while (at < text.length && IDENTIFIER_PART.test(text[at])) at += 1
      tokens.push({ type: 'name', value: text.slice(start, at) })
    } else if (/[0-9]/.test(character) || (character === '.' && /[0-9]/.test(text[at + 1] ?? ''))) {
      const start = at
      at += 1
      while (at < text.length && /[\w.]/.test(text[at])) at += 1
      tokens.push({ type: 'number', value: text.slice(start, at) })
    } else if (character in CLOSERS) {
      stack.push(character)
      tokens.push({ type: 'punct', value: character })
      at += 1
    } else if (character === ')' || character === ']' || character === '}') {
      const open = stack.pop()
      if (open === 'template' && character === '}') {
        at += 1
        readTemplate()
        continue
      }
      if (CLOSERS[open] !== character) fail(file, 'Unbalanced bracket')
      tokens.push({ type: 'punct', value: character })
      at += 1
    } else {
      const punctuator =
        PUNCTUATORS.find((candidate) => text.startsWith(candidate, at)) ?? character
      tokens.push({ type: 'punct', value: punctuator })
      at += punctuator.length
    }
  }
  if (stack.length) fail(file, 'Unbalanced bracket')
  return tokens
}

/**
 * The specifiers one module loads: static imports, re-exports and literal dynamic imports. `import type` and
 * `export type` statements are left out because they are erased before the module runs; every other import counts,
 * even when only its types are used. Throws on require, on a dynamic import whose argument is not one plain string
 * and on an import statement it cannot read to its end.
 * @param {string} text
 * @param {string} file
 * @param {boolean} [erased] also return the type-only specifiers, for comparison with a full parser
 * @returns {string[]}
 */
export function moduleSpecifiers(text, file, erased = false) {
  const tokens = tokenize(text, file)
  const specifiers = []
  const is = (index, type, value) =>
    tokens[index]?.type === type && (value === undefined || tokens[index].value === value)
  const member = (index) => is(index - 1, 'punct', '.') || is(index - 1, 'punct', '?.')
  // The string after `from` that ends a statement starting at `index`, skipping a braced binding list.
  const fromClause = (index) => {
    let depth = 0
    for (let at = index; at < tokens.length; at += 1) {
      const token = tokens[at]
      if (token.type === 'punct' && token.value === '{') depth += 1
      else if (token.type === 'punct' && token.value === '}') depth -= 1
      else if (depth === 0 && token.type === 'punct' && token.value === ';') return undefined
      else if (depth === 0 && is(at, 'name', 'from') && is(at + 1, 'string')) return at + 1
      else if (depth === 0 && (is(at, 'name', 'import') || is(at, 'name', 'export')) && at > index)
        return undefined
    }
    return undefined
  }
  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index]
    if (token.type !== 'name' || member(index)) continue
    // An object key or a method named like the keyword is not a statement.
    if (is(index + 1, 'punct', ':') && (is(index - 1, 'punct', '{') || is(index - 1, 'punct', ',')))
      continue
    if (token.value === 'require') {
      if (is(index + 1, 'punct', '(')) fail(file, 'require() is not a static import')
      continue
    }
    if (token.value === 'import') {
      if (is(index + 1, 'punct', '.')) continue // import.meta
      if (is(index + 1, 'punct', '(')) {
        if (
          is(index + 2, 'string') &&
          (is(index + 3, 'punct', ')') || is(index + 3, 'punct', ','))
        ) {
          specifiers.push(tokens[index + 2].value)
          continue
        }
        fail(file, 'A dynamic import needs one string literal')
      }
      if (is(index + 1, 'string')) {
        specifiers.push(tokens[index + 1].value)
        continue
      }
      if (is(index + 2, 'punct', '=')) fail(file, 'import = require() is not a static import')
      const typeOnly =
        is(index + 1, 'name', 'type') &&
        !is(index + 2, 'name', 'from') &&
        !is(index + 2, 'punct', ',')
      const at = fromClause(index + 1)
      if (at === undefined) fail(file, 'Unreadable import statement')
      if (erased || !typeOnly) specifiers.push(tokens[at].value)
      index = at
      continue
    }
    if (token.value === 'export') {
      const typeOnly =
        is(index + 1, 'name', 'type') &&
        (is(index + 2, 'punct', '{') || is(index + 2, 'punct', '*'))
      const start = typeOnly ? index + 2 : index + 1
      if (!is(start, 'punct', '*') && !is(start, 'punct', '{')) continue
      let at = start
      if (is(start, 'punct', '{')) {
        let depth = 0
        for (; at < tokens.length; at += 1) {
          if (is(at, 'punct', '{')) depth += 1
          else if (is(at, 'punct', '}') && --depth === 0) break
        }
        if (!is(at + 1, 'name', 'from')) continue // a local export list
      }
      const clause = fromClause(start)
      if (clause === undefined) fail(file, 'Unreadable re-export statement')
      if (erased || !typeOnly) specifiers.push(tokens[clause].value)
      index = clause
    }
  }
  return specifiers
}

const SCANNED = /\.(?:mjs|js|mts|ts)$/
const DATA = /\.json$/
/** Paths a generation module may never resolve to, so the closure stays disjoint from port-verification exemptions. */
const EXCLUDED = /(?:\.(?:test|spec)\.[^/]+$|(?:^|\/)(?:fixtures|test|__tests__)\/|\.md$)/i

/**
 * The tracked file one relative specifier names at run time under tsx: `.js` resolves to the one of `.js`, `.ts`
 * that exists, `.mjs` to `.mjs` or `.mts`, `.json` to itself. Anything else, or none or both candidates, fails.
 */
function resolveRelative(importer, specifier, files, scope) {
  if (/[?#]/.test(specifier))
    fail(importer, 'An import with a query or fragment is not read: ' + specifier)
  const target = posix.normalize(posix.join(posix.dirname(importer), specifier))
  if (!scope(target)) fail(importer, 'An import outside the generation scope: ' + specifier)
  let candidates
  if (/\.js$/.test(target))
    candidates = ['.js', '.ts', '.tsx', '.jsx'].map((extension) => target.slice(0, -3) + extension)
  else if (/\.mjs$/.test(target)) candidates = [target, target.slice(0, -4) + '.mts']
  else if (DATA.test(target)) candidates = [target]
  else fail(importer, 'An import needs a .js, .mjs or .json extension: ' + specifier)
  const present = candidates.filter((candidate) => files.has(candidate))
  if (present.length !== 1)
    fail(
      importer,
      (present.length ? 'An ambiguous import: ' : 'An import of an untracked file: ') + specifier
    )
  const [resolved] = present
  if (EXCLUDED.test(resolved))
    fail(importer, 'A generation module imports a test, fixture or document: ' + specifier)
  return resolved
}

/**
 * Every tracked module reachable from `roots` through static imports, re-exports and literal dynamic imports,
 * roots included, sorted. A bare specifier adds no file and must be a `node:` built-in or one of `packages` (or a
 * subpath of one), whose installed contents the dependency manifests bind; any other bare, `#` subpath, absolute or
 * URL specifier fails, because it could resolve to a local file this graph does not follow.
 * @param {{ roots: readonly string[], files: Iterable<string>, read: (file: string) => string, scope: (file: string) => boolean, packages: readonly string[] }} options
 * @returns {string[]}
 */
export function moduleClosure({ roots, files, read, scope, packages }) {
  const tracked = new Set(files)
  const reached = new Set()
  const pending = [...roots]
  while (pending.length) {
    const file = pending.pop()
    if (reached.has(file)) continue
    if (!tracked.has(file)) fail(file, 'A generation root is not tracked')
    reached.add(file)
    if (DATA.test(file)) continue
    if (!SCANNED.test(file)) fail(file, 'A generation module must be JavaScript or TypeScript')
    for (const specifier of moduleSpecifiers(read(file), file)) {
      if (/\\/.test(specifier)) fail(file, 'An escaped import specifier is not read: ' + specifier)
      if (specifier.startsWith('./') || specifier.startsWith('../'))
        pending.push(resolveRelative(file, specifier, tracked, scope))
      else if (
        !/^node:[a-z_/]+$/.test(specifier) &&
        !packages.some((name) => specifier === name || specifier.startsWith(name + '/'))
      )
        fail(file, 'An import outside the sanctioned packages: ' + specifier)
    }
  }
  return [...reached].sort()
}

export { GraphError }
