// The passport's live avatar: what it tells the scene, what the column shows while the scene loads it,
// once it is drawn and when it cannot be, and how a drag turns it.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import type { AvatarLook } from '@explorer-core/engine/avatarEquip'
import type { PassportPreviewMessage } from '@explorer-core/engine/protocol'
import type { EngineSession } from '../../features/session/useEngineSession'
import { SessionProvider } from '../../features/session/SessionContext'
import { PassportAvatar, playPassportEmote } from '../../features/profile/PassportAvatar'
import { useEngineWindow } from '../../lib/engineWindow'
import { fakeSession } from '../harness'

const LOOK: AvatarLook = {
  bodyShape: 'urn:decentraland:off-chain:base-avatars:BaseMale',
  eyes: null,
  hair: { r: 0.2, g: 0.1, b: 0 },
  skin: null,
  wearables: ['urn:decentraland:off-chain:base-avatars:eyes_00'],
  emotes: [],
  forceRender: []
}
const COLUMN = new DOMRect(135, 76, 514, 928)

type Sent = PassportPreviewMessage
const opens = (sent: Sent[]) => sent.flatMap((m) => (m.kind === 'passportPreview' ? [m.open] : []))

function Window(): React.JSX.Element {
  const engineWindow = useEngineWindow()
  return <div data-testid="window">{engineWindow == null ? 'none' : JSON.stringify(engineWindow.rect)}</div>
}

function setup(): {
  sent: Sent[]
  session: EngineSession
  show: (state?: EngineSession['passportPreview']['state'], props?: Partial<React.ComponentProps<typeof PassportAvatar>>) => void
  onLive: ReturnType<typeof vi.fn>
  onReady: ReturnType<typeof vi.fn>
  unmount: () => void
} {
  const sent: Sent[] = []
  const session = { ...fakeSession(), passportPreview: { send: (msg: Sent) => void sent.push(msg), state: null } } as EngineSession
  const onLive = vi.fn()
  const onReady = vi.fn()
  const tree = (state: EngineSession['passportPreview']['state'], props: Partial<React.ComponentProps<typeof PassportAvatar>>): React.JSX.Element => (
    <SessionProvider value={{ ...session, passportPreview: { ...session.passportPreview, state } }}>
      <PassportAvatar look={LOOK} name="kurd" background="overview" wave picture={<img alt="kurd" src="body.png" />} onLive={onLive} onReady={onReady} {...props} />
      <Window />
    </SessionProvider>
  )
  const view = render(tree(null, {}))
  return { sent, session, onLive, onReady, show: (state = null, props = {}) => view.rerender(tree(state, props)), unmount: view.unmount }
}

const settle = async (): Promise<void> => {
  await act(async () => {
    await Promise.resolve()
  })
}
const lastOpen = (sent: Sent[]) => opens(sent).at(-1)

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(COLUMN)
  vi.stubGlobal('devicePixelRatio', 2)
})
afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

describe('when the passport shows its live avatar', () => {
  it('should ask the scene for the look in the column, over the panel background on show', async () => {
    const { sent, onLive } = setup()
    await settle()
    expect(lastOpen(sent)).toMatchObject({ look: LOOK, rect: { x: 135, y: 76, width: 514, height: 928 }, dpr: 2, background: 'overview', wave: true })
    expect(onLive).toHaveBeenLastCalledWith(true)
    // the opening under the panel runs a little past the column, under the panel's own background
    expect(screen.getByTestId('window')).toHaveTextContent('"width":522')
  })

  it('should wait with a spinner, and without the picture, until the scene has drawn it', async () => {
    const { sent, show } = setup()
    await settle()
    expect(screen.getByRole('status')).toBeInTheDocument()
    expect(screen.queryByRole('img', { name: 'kurd' })).not.toBeInTheDocument()
    show({ id: lastOpen(sent)!.id, state: 'ready' })
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
    expect(screen.queryByRole('img', { name: 'kurd' })).not.toBeInTheDocument()
  })

  it('should say it is ready only once the scene has drawn it, and no longer when it goes', async () => {
    const { sent, show, onReady, unmount } = setup()
    await settle()
    expect(onReady).not.toHaveBeenCalled()
    show({ id: lastOpen(sent)!.id, state: 'ready' })
    expect(onReady).toHaveBeenLastCalledWith(true)
    unmount()
    expect(onReady).toHaveBeenLastCalledWith(false)
  })

  it('should send the same avatar again when the panel background changes', async () => {
    const { sent, show } = setup()
    await settle()
    const first = lastOpen(sent)!
    show(null, { background: 'photos' })
    await settle()
    expect(lastOpen(sent)).toMatchObject({ id: first.id, background: 'photos' })
  })

  it('should close the scene avatar and the window when it goes', async () => {
    const { sent, unmount, onLive } = setup()
    await settle()
    unmount()
    expect(lastOpen(sent)).toBeNull()
    expect(onLive).toHaveBeenLastCalledWith(false)
  })
})

describe('when the scene cannot draw the avatar', () => {
  it('should show the picture and close the scene avatar when the scene says so', async () => {
    const { sent, show, onLive } = setup()
    await settle()
    show({ id: lastOpen(sent)!.id, state: 'failed' })
    expect(screen.getByRole('img', { name: 'kurd' })).toBeInTheDocument()
    expect(lastOpen(sent)).toBeNull()
    expect(onLive).toHaveBeenLastCalledWith(false)
    expect(screen.getByTestId('window')).toHaveTextContent('none')
  })

  it('should show the picture when the scene never answers', async () => {
    vi.useFakeTimers()
    setup()
    await act(async () => {
      await vi.advanceTimersByTimeAsync(15000)
    })
    expect(screen.getByRole('img', { name: 'kurd' })).toBeInTheDocument()
  })

  it('should ignore the word on another avatar', async () => {
    const { sent, show } = setup()
    await settle()
    show({ id: lastOpen(sent)!.id + 100, state: 'failed' })
    expect(screen.queryByRole('img', { name: 'kurd' })).not.toBeInTheDocument()
  })
})

describe('when the avatar is turned', () => {
  it('should send a left drag to the scene in screen pixels', async () => {
    vi.useFakeTimers()
    const { sent } = setup()
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0)
    })
    const surface = screen.getByRole('img', { name: /kurd's avatar/ })
    fireEvent.pointerDown(surface, { button: 0, pointerId: 1, clientX: 300 })
    fireEvent.pointerMove(surface, { pointerId: 1, clientX: 310 })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(32)
    })
    const drag = sent.find((m) => m.kind === 'passportPreviewDrag')
    expect(drag).toMatchObject({ id: lastOpen(sent)!.id, dx: 20 })
    expect(drag != null && drag.kind === 'passportPreviewDrag' && drag.seconds).toBeGreaterThan(0)
  })

  it('should not turn under another button', async () => {
    const { sent } = setup()
    await settle()
    const surface = screen.getByRole('img', { name: /kurd's avatar/ })
    fireEvent.pointerDown(surface, { button: 2, pointerId: 1, clientX: 300 })
    fireEvent.pointerMove(surface, { pointerId: 1, clientX: 340 })
    await settle()
    expect(sent.some((m) => m.kind === 'passportPreviewDrag')).toBe(false)
  })

  it('should leave the arrow keys alone and take no keyboard focus', async () => {
    const { sent } = setup()
    await settle()
    const surface = screen.getByRole('img', { name: /kurd's avatar/ })
    const before = sent.length
    fireEvent.keyDown(surface, { key: 'ArrowRight' })
    expect(sent).toHaveLength(before)
    expect(surface).not.toHaveAttribute('tabindex')
  })

  it('should play an emote on it', async () => {
    const { sent } = setup()
    await settle()
    playPassportEmote('urn:emote:clap')
    expect(sent.at(-1)).toEqual({ kind: 'passportPreviewEmote', id: lastOpen(sent)!.id, urn: 'urn:emote:clap' })
  })
})

describe('when another passport opens over it', () => {
  it('should leave the stage to the newer avatar and show its own picture', async () => {
    const { sent, session } = setup()
    await settle()
    const first = lastOpen(sent)!
    const over = render(
      <SessionProvider value={session}>
        <PassportAvatar look={LOOK} name="ana" background="overview" wave picture={<img alt="ana" src="ana.png" />} onLive={vi.fn()} onReady={vi.fn()} />
      </SessionProvider>
    )
    await settle()
    expect(screen.getByRole('img', { name: 'kurd' })).toBeInTheDocument()
    expect(lastOpen(sent)!.id).not.toBe(first.id)
    // the older one must not have closed the newer one's avatar
    expect(opens(sent).at(-1)).not.toBeNull()
    over.unmount()
    await settle()
    expect(lastOpen(sent)).toMatchObject({ id: first.id })
  })
})
