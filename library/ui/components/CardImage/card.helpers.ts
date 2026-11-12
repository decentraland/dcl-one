export function hasUnsafeURLCharacters(value: string): boolean {
  return (
    value.includes('\\') ||
    [...value].some((character) => {
      const code = character.charCodeAt(0)
      return code < 32 || code === 127
    })
  )
}
