const ME = 'me'
const AUTH_SERVER = 'authoritative-server'

describe('when the runtime cannot say whether the scene is the server', () => {
  // addSyncTransport binds to the global engine, so the case gets its own module registry.
  let engine: any
  let CommsMessage: any
  let encodeString: (value: string) => Uint8Array
  let inbox: Uint8Array[]
  let sent: { type: number; address: string[] }[]
  let error: jest.SpyInstance
  let room: any

  function commsMessage(sender: string, type: number, payload: Uint8Array): Uint8Array {
    const encoded = encodeString(sender)
    const out = new Uint8Array(1 + encoded.byteLength + 1 + payload.byteLength)
    out.set([encoded.byteLength], 0)
    out.set(encoded, 1)
    out.set([type], 1 + encoded.byteLength)
    out.set(payload, 1 + encoded.byteLength + 1)
    return out
  }

  function flush() {
    return new Promise<void>((resolve) => setTimeout(resolve, 0))
  }

  async function tick(seconds: number) {
    await engine.update(seconds)
    await flush()
  }

  beforeEach(async () => {
    jest.resetModules()
    error = jest.spyOn(console, 'error').mockImplementation(() => {})
    const ecs = require('../../../packages/@dcl/ecs/dist')
    const bus = require('../../../packages/@dcl/sdk/network/binary-message-bus')
    const { addSyncTransport } = require('../../../packages/@dcl/sdk/network/message-bus-sync')
    const { registerMessages } = require('../../../packages/@dcl/sdk/network/events/implementation')
    engine = ecs.engine
    CommsMessage = bus.CommsMessage
    encodeString = bus.encodeString
    inbox = []
    sent = []

    addSyncTransport(
      engine,
      async (message: any) => {
        for (const peer of message.peerData ?? []) {
          for (const data of peer.data ?? []) {
            if (data.byteLength) sent.push({ type: data[0], address: peer.address })
          }
        }
        const pending = inbox
        inbox = []
        return { data: pending }
      },
      async () => ({ data: { userId: ME, version: 1, displayName: ME, hasConnectedWeb3: true } }),
      async () => {
        throw new Error('IsServer is not implemented')
      },
      'test'
    )
    room = registerMessages({ ping: ecs.Schemas.Map({ n: ecs.Schemas.Int }) })

    ecs.EngineInfo.createOrReplace(engine.RootEntity, {
      tickNumber: 400,
      frameNumber: 400,
      totalRuntime: 1,
      sceneHidden: false
    })
    ecs.RealmInfo.createOrReplace(engine.RootEntity, {
      baseUrl: 'http://localhost',
      realmName: 'test',
      networkId: 1,
      commsAdapter: 'offline',
      isPreview: true,
      room: 'room',
      isConnectedSceneRoom: true
    })
    await tick(0.1)
    void room.send('ping', { n: 1 })
    inbox.push(commsMessage(AUTH_SERVER, CommsMessage.RES_CRDT_STATE, new Uint8Array()))
    await tick(0.1)
    await tick(0.1)
  })

  afterEach(() => {
    error.mockRestore()
  })

  it('should report the failure once', () => {
    expect(error).toHaveBeenCalledTimes(1)
    expect(String(error.mock.calls[0][0])).toContain('running as a client')
  })

  it('should carry on as a client: ask for the state and send room events to the server', () => {
    expect(sent.some(($) => $.type === CommsMessage.REQ_CRDT_STATE)).toBe(true)
    expect(sent.filter(($) => $.type === CommsMessage.CUSTOM_EVENT)).toEqual([
      { type: CommsMessage.CUSTOM_EVENT, address: [AUTH_SERVER] }
    ])
  })
})
