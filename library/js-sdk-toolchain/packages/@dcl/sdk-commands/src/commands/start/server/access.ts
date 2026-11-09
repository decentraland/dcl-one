import type { IHttpServerComponent } from '@well-known-components/interfaces'
import { getUnderlyingServer } from '@well-known-components/http-server'
import type { IncomingMessage, Server } from 'http'

/**
 * The preview server listens on every interface so phones on the LAN can preview the scene, while
 * the editor's data layer and the scene's secrets must stay with the machine that runs the preview.
 * Node's request context is not visible to the router, so the peer address is copied onto a
 * header before the router runs; a value the client sent under that name is overwritten.
 */
export const PEER_ADDRESS_HEADER = 'x-dcl-peer-address'

const LOCAL_ORIGIN = /^https?:\/\/(localhost|127(?:\.\d{1,3}){3}|\[::1\])(:\d+)?$/i

export function isLoopbackAddress(address: string | undefined): boolean {
  if (!address) return false
  const plain = address.startsWith('::ffff:') ? address.slice('::ffff:'.length) : address
  return plain === '::1' || /^127(?:\.\d{1,3}){3}$/.test(plain)
}

/** No Origin (a non-browser client or a same-origin request), an opaque one (`file:` pages) or a loopback page. */
export function isLocalOrigin(origin: string | null): boolean {
  if (origin === null || origin === '' || origin === 'null') return true
  if (origin.startsWith('file://')) return true
  return LOCAL_ORIGIN.test(origin)
}

/** A request from a loopback peer and a local page; fails closed when the peer address was not stamped. */
export function isLocalRequest(request: IHttpServerComponent.IRequest): boolean {
  return (
    isLoopbackAddress(request.headers.get(PEER_ADDRESS_HEADER) ?? undefined) &&
    isLocalOrigin(request.headers.get('origin'))
  )
}

export const FORBIDDEN_RESPONSE: IHttpServerComponent.IResponse = {
  status: 403,
  body: { message: 'This endpoint only answers the machine running the preview' }
}

/** A router middleware that answers 403 to every request that is not local. */
export const localOnly: IHttpServerComponent.IRequestHandler<IHttpServerComponent.DefaultContext<any>> = async (
  ctx,
  next
) => {
  if (!isLocalRequest(ctx.request)) return FORBIDDEN_RESPONSE
  return next()
}

function stamp(request: IncomingMessage) {
  request.headers[PEER_ADDRESS_HEADER] = request.socket?.remoteAddress ?? ''
}

/** Copies every peer's address onto `PEER_ADDRESS_HEADER` before the router sees the request or the upgrade. */
export async function stampPeerAddress(server: IHttpServerComponent<any>): Promise<void> {
  const nodeServer = await getUnderlyingServer<Server>(server)
  nodeServer.prependListener('request', stamp)
  nodeServer.prependListener('upgrade', stamp)
}
