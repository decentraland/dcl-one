/** `url` as a CSS `url("…")` value. A quote or a backslash is escaped and a line break is removed, so
    the address cannot end the string and add CSS of its own. */
export function cssUrl(url: string): string {
  return `url("${url.replace(/[\\"]/g, '\\$&').replace(/[\r\n]/g, '')}")`
}
