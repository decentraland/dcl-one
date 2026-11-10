import { StrictMode } from 'react'
import { act, cleanup, render } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import AnimatedBackground from './AnimatedBackground'
import { createProgram, createShader, loadTexture } from './webgl'

vi.mock('./webgl', () => ({ createProgram: vi.fn(), createShader: vi.fn(), loadTexture: vi.fn() }))

describe('when the shared animated background manages its WebGL lifecycle', () => {
  let gl: WebGLRenderingContext
  let lost: boolean
  let reduced: boolean
  let changes: Set<() => void>
  let loseContext: ReturnType<typeof vi.fn>
  let frames: ReturnType<typeof vi.fn>
  let cancel: ReturnType<typeof vi.fn>

  beforeEach(() => {
    lost = false
    reduced = false
    changes = new Set()
    loseContext = vi.fn(() => {
      lost = true
    })
    gl = {
      VERTEX_SHADER: 35633,
      FRAGMENT_SHADER: 35632,
      getExtension: vi.fn(() => ({ loseContext })),
      isContextLost: vi.fn(() => lost),
      getAttribLocation: vi.fn(() => 0),
      getUniformLocation: vi.fn(() => ({})),
      createBuffer: vi.fn(() => ({})),
      bindBuffer: vi.fn(),
      bufferData: vi.fn(),
      deleteProgram: vi.fn(),
      deleteShader: vi.fn(),
      deleteBuffer: vi.fn(),
      deleteTexture: vi.fn(),
    } as unknown as WebGLRenderingContext
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(gl)
    vi.mocked(createShader).mockImplementation(() => (lost ? null : ({} as WebGLShader)))
    vi.mocked(createProgram).mockReturnValue({} as WebGLProgram)
    vi.mocked(loadTexture).mockReturnValue({} as WebGLTexture)
    frames = vi.fn(() => 1)
    cancel = vi.fn()
    vi.stubGlobal('requestAnimationFrame', frames)
    vi.stubGlobal('cancelAnimationFrame', cancel)
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({
        get matches() {
          return reduced
        },
        addEventListener: (_event: string, listener: () => void) => changes.add(listener),
        removeEventListener: (_event: string, listener: () => void) => changes.delete(listener),
      })),
    )
  })
  afterEach(async () => {
    cleanup()
    await Promise.resolve()
    vi.resetAllMocks()
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('should keep the context usable across StrictMode replay and release it after a real unmount', async () => {
    const { unmount } = render(
      <StrictMode>
        <AnimatedBackground />
      </StrictMode>,
    )
    await act(async () => {
      await Promise.resolve()
    })
    expect(createShader).toHaveBeenCalledTimes(4)
    expect(lost).toBe(false)
    expect(loseContext).not.toHaveBeenCalled()
    expect(frames).toHaveBeenCalledTimes(2)
    unmount()
    await act(async () => {
      await Promise.resolve()
    })
    expect(loseContext).toHaveBeenCalledTimes(1)
  })

  it('should stop and resume motion live while retaining the static fallback during the pause', () => {
    const { container } = render(<AnimatedBackground />)
    expect(frames).toHaveBeenCalledTimes(1)
    act(() => {
      reduced = true
      for (const update of changes) update()
    })
    expect(cancel).toHaveBeenCalled()
    expect(container.querySelector('canvas')).toHaveStyle({ opacity: '0' })
    expect(frames).toHaveBeenCalledTimes(1)
    act(() => {
      reduced = false
      for (const update of changes) update()
    })
    expect(frames).toHaveBeenCalledTimes(2)
    expect(lost).toBe(false)
  })

  it('should release its retained context when unmounted while reduced motion is enabled', async () => {
    const { unmount } = render(<AnimatedBackground />)
    act(() => {
      reduced = true
      for (const update of changes) update()
    })
    unmount()
    await act(async () => {
      await Promise.resolve()
    })
    expect(loseContext).toHaveBeenCalledTimes(1)
  })

  it('should avoid requesting WebGL or animation frames for an initial reduced-motion preference', () => {
    reduced = true
    const { container } = render(<AnimatedBackground />)
    expect(HTMLCanvasElement.prototype.getContext).not.toHaveBeenCalled()
    expect(frames).not.toHaveBeenCalled()
    expect(container.querySelector('canvas')).toHaveStyle({ opacity: '0' })
  })

  it('should preserve the fallback without scheduling frames when WebGL is unavailable', () => {
    vi.mocked(HTMLCanvasElement.prototype.getContext).mockReturnValue(null)
    const { container } = render(<AnimatedBackground />)
    expect(createShader).not.toHaveBeenCalled()
    expect(frames).not.toHaveBeenCalled()
    expect(container.querySelector('canvas')).toHaveStyle({ opacity: '0' })
  })

  it('should return a failed shader context and leave the fallback visible', () => {
    vi.mocked(createShader).mockReturnValueOnce(null)
    const { container } = render(<AnimatedBackground />)
    expect(loseContext).toHaveBeenCalledTimes(1)
    expect(frames).not.toHaveBeenCalled()
    expect(container.querySelector('canvas')).toHaveStyle({ opacity: '0' })
  })
})
