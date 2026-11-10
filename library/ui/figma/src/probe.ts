import { FigmaApiError, type FigmaClient } from './figma/client.js'

type ProbeStatus = 'ok' | 'forbidden' | 'not-found' | 'error'

type ProbeResult = {
  file: ProbeStatus
  variables: ProbeStatus
  pages: string[]
}

function toStatus(error: unknown): ProbeStatus {
  if (error instanceof FigmaApiError) {
    if (error.status === 403) return 'forbidden'
    if (error.status === 404) return 'not-found'
  }
  return 'error'
}

async function probeAccess(client: FigmaClient, fileKey: string): Promise<ProbeResult> {
  const result: ProbeResult = { file: 'ok', variables: 'ok', pages: [] }

  try {
    const file = await client.get<{ document: { children: { name: string }[] } }>(
      `/v1/files/${fileKey}?depth=1`
    )
    result.pages = file.document.children.map((page) => page.name.trim())
  } catch (error) {
    result.file = toStatus(error)
  }

  try {
    await client.get(`/v1/files/${fileKey}/variables/local`)
  } catch (error) {
    result.variables = toStatus(error)
  }

  return result
}

export { probeAccess }
export type { ProbeResult }
