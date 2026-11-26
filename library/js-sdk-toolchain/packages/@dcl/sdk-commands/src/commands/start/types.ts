import type {
  ILoggerComponent,
  IMetricsComponent,
  IHttpServerComponent,
  IConfigComponent
} from '@well-known-components/interfaces'
import type { HTTPProvider } from 'eth-connect'
import type { RoomComponent } from '@dcl/mini-comms/dist/adapters/rooms'
import type { WebSocketComponent } from './server/ws'
import type { CliComponents } from '../../components'
import type { ISignalerComponent } from '../../components/exit-signal'

export type PreviewComponents = CliComponents & {
  logs: ILoggerComponent
  server: IHttpServerComponent<PreviewComponents>
  config: IConfigComponent
  metrics: IMetricsComponent<any>
  ethereumProvider: HTTPProvider
  rooms: RoomComponent
  ws: WebSocketComponent
  signaler: ISignalerComponent
}
