import { render } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { AnimatedBackground } from './AnimatedBackground'

type FakeGl = Record<string, Mock> & { loseContext: Mock }

// A WebGL context double: every method is a spy, uniform locations carry their name and the constants are their own
// names, so assertions read which uniform received which value.
function fakeGl({
  compiles = true,
  links = true
}: { compiles?: boolean; links?: boolean } = {}): FakeGl {
  const loseContext = vi.fn()
  const methods: Record<string, Mock> = {
    createShader: vi.fn(() => ({})),
    getShaderParameter: vi.fn(() => compiles),
    createProgram: vi.fn(() => ({})),
    createBuffer: vi.fn(() => ({})),
    createTexture: vi.fn(() => ({})),
    getProgramParameter: vi.fn(() => links),
    getUniformLocation: vi.fn((_program: unknown, name: string) => ({ name })),
    getAttribLocation: vi.fn(() => 0),
    getExtension: vi.fn((name: string) => (name === 'WEBGL_lose_context' ? { loseContext } : null))
  }
  return new Proxy({ loseContext } as FakeGl, {
    get(target, key) {
      if (typeof key !== 'string') return undefined
      if (key in target) return target[key]
      if (/^[A-Z][A-Z0-9_]*$/.test(key)) return key
      target[key] = methods[key] ?? vi.fn()
      return target[key]
    }
  })
}

function tilingValues(gl: FakeGl) {
  return gl.uniform1f!.mock.calls
    .filter(([location]) => (location as { name: string }).name === 'u_overlayTiling')
    .map(([, value]) => value)
}

describe('AnimatedBackground', () => {
  let frames: Map<number, FrameRequestCallback>
  let nextFrame: number
  let requestFrame: Mock<(callback: FrameRequestCallback) => number>
  let cancelFrame: Mock<(id: number) => void>
  let getContext: Mock
  let gl: FakeGl | null

  const runFrame = () => {
    const [id, callback] = [...frames][0]!
    frames.delete(id)
    callback(0)
  }

  beforeEach(() => {
    frames = new Map()
    nextFrame = 0
    requestFrame = vi.fn((callback: FrameRequestCallback) => {
      frames.set(++nextFrame, callback)
      return nextFrame
    })
    cancelFrame = vi.fn((id: number) => void frames.delete(id))
    vi.stubGlobal('requestAnimationFrame', requestFrame)
    vi.stubGlobal('cancelAnimationFrame', cancelFrame)
    gl = fakeGl()
    getContext = vi.fn(() => gl)
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
      getContext as unknown as HTMLCanvasElement['getContext']
    )
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  const reducedMotion = (matches: boolean) =>
    vi.stubGlobal(
      'matchMedia',
      vi.fn((query: string) => ({ matches: matches && query.includes('reduce'), media: query }))
    )

  describe('when WebGL is available', () => {
    beforeEach(() => reducedMotion(false))

    it('should draw on a canvas over the still image, hidden from assistive technology', () => {
      const { container } = render(<AnimatedBackground />)
      const root = container.firstElementChild!
      expect(root).toHaveAttribute('aria-hidden', 'true')
      expect(root).toHaveClass('root')
      expect(root.querySelector('.fallback')).toBeInTheDocument()
      expect(root.querySelector('canvas')).toHaveClass('canvas')
      expect(requestFrame).toHaveBeenCalledTimes(1)
    })

    it('should draw the pattern at the default tiling of 1.66', () => {
      render(<AnimatedBackground />)
      runFrame()
      expect(tilingValues(gl!)).toEqual([1.66])
      expect(gl!.drawArrays).toHaveBeenCalledTimes(1)
    })

    it('should draw the pattern at the given tiling and apply a change on the next frame without a new context', () => {
      const { rerender } = render(<AnimatedBackground patternTiling={1} />)
      runFrame()
      rerender(<AnimatedBackground patternTiling={2.5} />)
      runFrame()
      expect(tilingValues(gl!)).toEqual([1, 2.5])
      expect(getContext).toHaveBeenCalledTimes(1)
    })

    it('should stop the animation, free the GL objects, give the context back and remove the canvas on unmount', () => {
      const { container, unmount } = render(<AnimatedBackground />)
      runFrame()
      const canvas = container.querySelector('canvas')!
      unmount()
      expect(cancelFrame).toHaveBeenLastCalledWith(2)
      expect(frames.size).toBe(0)
      expect(gl!.deleteProgram).toHaveBeenCalledTimes(1)
      expect(gl!.deleteShader).toHaveBeenCalledTimes(2)
      expect(gl!.deleteBuffer).toHaveBeenCalledTimes(1)
      expect(gl!.deleteTexture).toHaveBeenCalledTimes(1)
      expect(gl!.loseContext).toHaveBeenCalledTimes(1)
      expect(canvas.isConnected).toBe(false)
    })

    it('should draw on a fresh canvas and context when mounted again', () => {
      const first = render(<AnimatedBackground />)
      first.unmount()
      const second = render(<AnimatedBackground />)
      expect(getContext).toHaveBeenCalledTimes(2)
      expect(second.container.querySelectorAll('canvas')).toHaveLength(1)
    })

    it('should stop the animation and reveal the still image when the context is lost', () => {
      const { container } = render(<AnimatedBackground />)
      const canvas = container.querySelector('canvas')!
      canvas.dispatchEvent(new Event('webglcontextlost'))
      expect(cancelFrame).toHaveBeenCalledWith(1)
      expect(frames.size).toBe(0)
      expect(container.querySelector('canvas')).not.toBeInTheDocument()
      expect(container.querySelector('.fallback')).toBeInTheDocument()
    })
  })

  describe('when the browser has no WebGL', () => {
    beforeEach(() => {
      reducedMotion(false)
      gl = null
    })

    it('should keep only the still image and not start an animation', () => {
      const { container, unmount } = render(<AnimatedBackground />)
      expect(container.querySelector('.fallback')).toBeInTheDocument()
      expect(container.querySelector('canvas')).not.toBeInTheDocument()
      expect(requestFrame).not.toHaveBeenCalled()
      expect(() => unmount()).not.toThrow()
    })
  })

  describe('when the browser has no matchMedia', () => {
    it('should still animate', () => {
      vi.stubGlobal('matchMedia', undefined)
      const { container } = render(<AnimatedBackground />)
      expect(container.querySelector('canvas')).toBeInTheDocument()
      expect(requestFrame).toHaveBeenCalledTimes(1)
    })
  })

  describe('when the user prefers reduced motion', () => {
    beforeEach(() => reducedMotion(true))

    it('should keep only the still image without requesting a WebGL context', () => {
      const { container } = render(<AnimatedBackground />)
      expect(container.querySelector('.fallback')).toBeInTheDocument()
      expect(container.querySelector('canvas')).not.toBeInTheDocument()
      expect(getContext).not.toHaveBeenCalled()
      expect(requestFrame).not.toHaveBeenCalled()
    })
  })

  describe.each([
    ['a shader does not compile', { compiles: false }],
    ['the program does not link', { links: false }]
  ])('when %s', (_case, failure) => {
    beforeEach(() => {
      reducedMotion(false)
      gl = fakeGl(failure)
      vi.spyOn(console, 'error').mockImplementation(() => {})
    })

    it('should give the context back, remove the canvas and leave the still image', () => {
      const { container } = render(<AnimatedBackground />)
      expect(container.querySelector('canvas')).not.toBeInTheDocument()
      expect(container.querySelector('.fallback')).toBeInTheDocument()
      expect(gl!.loseContext).toHaveBeenCalledTimes(1)
      expect(requestFrame).not.toHaveBeenCalled()
      expect(console.error).toHaveBeenCalled()
    })
  })

  describe('when unmounted before the pattern image loads', () => {
    let images: HTMLImageElement[]

    beforeEach(() => {
      reducedMotion(false)
      images = []
      // The texture loader sets the image's source once; capture that image to fire its load by hand.
      vi.spyOn(HTMLImageElement.prototype, 'src', 'set').mockImplementation(function (
        this: HTMLImageElement
      ) {
        images.push(this)
      })
    })

    it('should skip uploading the late image to the freed texture', () => {
      const { unmount } = render(<AnimatedBackground />)
      unmount()
      const uploads = gl!.texImage2D!.mock.calls.length
      images[0]!.onload?.(new Event('load'))
      expect(gl!.texImage2D!.mock.calls.length).toBe(uploads)
      expect(gl!.generateMipmap).not.toHaveBeenCalled()
    })

    it('should upload the image when it loads while mounted', () => {
      render(<AnimatedBackground />)
      images[0]!.onload?.(new Event('load'))
      expect(gl!.generateMipmap).toHaveBeenCalledTimes(1)
    })
  })
})
