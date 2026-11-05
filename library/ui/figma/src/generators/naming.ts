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
