type NestedMessages = { [key: string]: string | NestedMessages }

/** Flattens nested translation JSON into the dotted-id map react-intl expects. */
const flattenMessages = (nested: NestedMessages, prefix = ''): Record<string, string> => {
  const flat: Record<string, string> = {}
  for (const [key, value] of Object.entries(nested)) {
    const id = prefix ? `${prefix}.${key}` : key
    if (typeof value === 'string') {
      flat[id] = value
    } else {
      Object.assign(flat, flattenMessages(value, id))
    }
  }
  return flat
}

export { flattenMessages, type NestedMessages }
