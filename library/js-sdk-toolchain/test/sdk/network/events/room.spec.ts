import { Schemas } from '../../../../packages/@dcl/ecs/src'
import { Atom } from '../../../../packages/@dcl/sdk/atom'
import { CommsMessage } from '../../../../packages/@dcl/sdk/network/binary-message-bus'
import {
  Room,
  getEventRegistry,
  registerMessages,
  setGlobalRoom
} from '../../../../packages/@dcl/sdk/network/events/implementation'
import { encodeEvent } from '../../../../packages/@dcl/sdk/network/events/protocol'

const AUTH_SERVER = 'authoritative-server'
const PEER = 'peer'
const Messages = { ping: Schemas.Map({ n: Schemas.Int }) }

describe('Room', () => {
  let emitted: { type: number; to?: string[] }[]
  let deliver: (data: Uint8Array, sender: string) => void
  let error: jest.SpyInstance

  function flush() {
    return new Promise<void>((resolve) => setTimeout(resolve, 0))
  }

  function ping(n: number): Uint8Array {
    return encodeEvent('ping', { n }, getEventRegistry())
  }

  function createRoom(isServer: boolean) {
    emitted = []
    const bus = {
      on: (type: number, cb: (data: Uint8Array, sender: string) => void) => {
        if (type === CommsMessage.CUSTOM_EVENT) deliver = cb
      },
      emit: (type: number, _data: Uint8Array, to?: string[]) => emitted.push({ type, to })
    }
    const room = new Room({} as any, bus, Atom<boolean>(isServer), Atom<boolean>(true))
    setGlobalRoom(room)
    return registerMessages(Messages)
  }

  beforeEach(() => {
    error = jest.spyOn(console, 'error').mockImplementation(() => {})
  })

  afterEach(() => {
    error.mockRestore()
  })

  describe('when a client sends an event', () => {
    beforeEach(async () => {
      const room = createRoom(false)
      await room.send('ping', { n: 1 })
    })

    it('should address it to the authoritative server only', () => {
      expect(emitted).toEqual([{ type: CommsMessage.CUSTOM_EVENT, to: [AUTH_SERVER] }])
    })
  })

  describe('when the server sends an event', () => {
    let room: ReturnType<typeof createRoom>

    beforeEach(() => {
      room = createRoom(true)
    })

    it('should broadcast when no peers are named', async () => {
      await room.send('ping', { n: 1 })
      expect(emitted).toEqual([{ type: CommsMessage.CUSTOM_EVENT, to: undefined }])
    })

    it('should address the peers it names', async () => {
      await room.send('ping', { n: 1 }, { to: [PEER] })
      expect(emitted).toEqual([{ type: CommsMessage.CUSTOM_EVENT, to: [PEER] }])
    })
  })

  describe('when an event reaches listeners that fail', () => {
    let received: number[]

    beforeEach(async () => {
      received = []
      const room = createRoom(false)
      room.onMessage('ping', () => {
        throw new Error('sync failure')
      })
      room.onMessage('ping', async () => {
        await flush()
        throw new Error('async failure')
      })
      room.onMessage('ping', ({ n }) => {
        received.push(n)
      })
      deliver(ping(3), AUTH_SERVER)
      await flush()
      await flush()
    })

    it('should still run the other listeners', () => {
      expect(received).toEqual([3])
    })

    it('should report each failure instead of leaking a rejection', () => {
      expect(error).toHaveBeenCalledTimes(2)
    })
  })

  describe('when a client receives an event from someone other than the server', () => {
    let received: number[]

    beforeEach(async () => {
      received = []
      const room = createRoom(false)
      room.onMessage('ping', ({ n }) => {
        received.push(n)
      })
      deliver(ping(4), PEER)
      await flush()
    })

    it('should drop it', () => {
      expect(received).toEqual([])
    })
  })

  describe('when the server receives an event from a peer', () => {
    let seen: { n: number; from?: string }[]

    beforeEach(async () => {
      seen = []
      const room = createRoom(true)
      room.onMessage('ping', ({ n }, context) => {
        seen.push({ n, from: context?.from })
      })
      deliver(ping(5), PEER)
      await flush()
    })

    it('should hand the listener the sender', () => {
      expect(seen).toEqual([{ n: 5, from: PEER }])
    })
  })

  describe('when an event cannot be decoded', () => {
    beforeEach(async () => {
      createRoom(true)
      deliver(new Uint8Array([1, 2, 3]), PEER)
      await flush()
    })

    it('should report it', () => {
      expect(error).toHaveBeenCalledTimes(1)
    })
  })
})
