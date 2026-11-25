import { WebSocketServer } from 'ws'
import type { PreviewComponents } from '../types'
import type { IBaseComponent } from '@well-known-components/interfaces'

export type WebSocketComponent = IBaseComponent & {
  ws: WebSocketServer
}

/**
 * Creates a ws-server component
 * @public
 */
export async function createWsComponent(_: Pick<PreviewComponents, 'logs'>): Promise<WebSocketComponent> {
  const ws = new WebSocketServer({ noServer: true })
  async function stop() {
    ws.close()
  }

  return {
    stop,
    ws
  }
}
