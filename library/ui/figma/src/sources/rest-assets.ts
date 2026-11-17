import type { FigmaClient } from '../figma/client.js'

// The two helpers the read-only REST source needs from the removed live pull (automation/exports.ts) and chunk
// ingest (capture/ingest-chunks.ts), kept as they were.

const HOSTS = ['.figma.com', '.amazonaws.com', '.figmausercontent.com']

// Signed export URLs: https on a Figma or export-storage host only, no redirects, bounded size.
async function download(client: FigmaClient, address: string, label: string): Promise<Buffer> {
  const url = new URL(address)
  if (url.protocol !== 'https:' || !HOSTS.some((suffix) => url.hostname.endsWith(suffix)))
    throw new Error(`Unexpected export host for ${label}`)
  return client.download(url, label, 16 * 1024 * 1024)
}

function svgProblem(svg: string): string | undefined {
  const text = svg.trim()
  if (!/^<svg[\s>]/.test(text) || !text.endsWith('</svg>')) return 'not a complete <svg> document'
  const opened = (text.match(/<(?!\/|!|\?)[^>]*[^/]>/g) ?? []).length
  const closed = (text.match(/<\/[a-zA-Z]/g) ?? []).length
  return opened === closed ? undefined : `unbalanced tags (${opened} opened, ${closed} closed)`
}

export { download, svgProblem }
