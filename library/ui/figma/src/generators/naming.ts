const words = (input: string) => input.split(/[^A-Za-z0-9]+/).filter(Boolean)

const capitalize = (word: string) => word.charAt(0).toUpperCase() + word.slice(1)

function camelCase(input: string): string {
  const [first = '', ...rest] = words(input).map((word) => word.toLowerCase())
  return first + rest.map(capitalize).join('')
}

function pascalCase(input: string): string {
  return words(input)
    .map((word) => capitalize(word.toLowerCase()))
    .join('')
}

function kebabCase(input: string): string {
  return words(input)
    .map((word) => word.toLowerCase())
    .join('-')
}

function uniqueName(base: string, used: Set<string>): string {
  let candidate = base
  for (let counter = 2; used.has(candidate); counter += 1) candidate = `${base}_${counter}`
  used.add(candidate)
  return candidate
}

const textStyleClass = (name: string) => `text${Buffer.from(name).toString('hex')}`

export { textStyleClass, camelCase, kebabCase, pascalCase, uniqueName }

const RESERVED = new Set(
  'break case catch class const continue debugger default delete do else enum export extends false finally for function if import in instanceof new null return super switch this throw true try typeof var void while with let static yield await implements interface package private protected public eval arguments'.split(
    ' '
  )
)
function assertIdentifier(value: string): string {
  if (!/^[A-Za-z_$][\w$]*$/.test(value) || RESERVED.has(value))
    throw new Error(`Invalid generated TypeScript identifier: ${JSON.stringify(value)}`)
  return value
}
export { assertIdentifier }
