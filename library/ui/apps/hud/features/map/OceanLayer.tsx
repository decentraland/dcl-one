// The sea and shores around Genesis City, drawn under the satellite map. One canvas the size of
// the map's view, redrawn every frame from the camera while the map is open; where the browser
// gives no WebGL, or takes it back, the map's plain background shows instead. The canvas goes
// when the map closes, so everything the context holds is given back then.

import { useEffect, useRef } from 'react'
import type { MapView } from '@explorer-core/features/map/mapZoom'
import { OCEAN_ART } from './mapArt'
import { OCEAN_FRAGMENT, OCEAN_VERTEX } from './oceanShader'
import styles from './MapCanvas.module.css'

/** The widest the sea is drawn, px: beyond it the canvas is stretched. */
const MAX_WIDTH = 1920

function compile(gl: WebGLRenderingContext, type: number, source: string): WebGLShader | null {
  const shader = gl.createShader(type)
  if (shader == null) return null
  gl.shaderSource(shader, source)
  gl.compileShader(shader)
  if (gl.getShaderParameter(shader, gl.COMPILE_STATUS) !== true) {
    console.error('[map] the sea shader did not compile', gl.getShaderInfoLog(shader))
    return null
  }
  return shader
}

function texture(
  gl: WebGLRenderingContext,
  unit: number,
  src: string,
  mipmaps: boolean,
  onReady: () => void
): { tex: WebGLTexture | null; image: HTMLImageElement } {
  const tex = gl.createTexture()
  gl.activeTexture(gl.TEXTURE0 + unit)
  gl.bindTexture(gl.TEXTURE_2D, tex)
  // stands in until the image arrives
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([0, 0, 0, 255]))
  const image = new Image()
  image.onload = () => {
    gl.activeTexture(gl.TEXTURE0 + unit)
    gl.bindTexture(gl.TEXTURE_2D, tex)
    // the textures' first row is their bottom
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true)
    gl.pixelStorei(gl.UNPACK_COLORSPACE_CONVERSION_WEBGL, gl.NONE)
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image)
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.REPEAT)
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.REPEAT)
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR)
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, mipmaps ? gl.LINEAR_MIPMAP_LINEAR : gl.LINEAR)
    if (mipmaps) gl.generateMipmap(gl.TEXTURE_2D)
    onReady()
  }
  image.src = src
  return { tex, image }
}

export function OceanLayer({ view }: { view: { current: MapView } }): React.JSX.Element {
  const ref = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    const canvas = ref.current
    if (canvas == null) return
    // without a sea the canvas would cover the map's background with a blank sheet
    canvas.hidden = true
    if (typeof WebGLRenderingContext === 'undefined') return
    const gl = canvas.getContext('webgl', { alpha: false, antialias: false, powerPreference: 'low-power' })
    if (gl == null || gl.isContextLost()) return
    const textures: Array<{ tex: WebGLTexture | null; image: HTMLImageElement }> = []
    let buffer: WebGLBuffer | null = null
    let frame = 0
    const vertex = compile(gl, gl.VERTEX_SHADER, OCEAN_VERTEX)
    const fragment = compile(gl, gl.FRAGMENT_SHADER, OCEAN_FRAGMENT)
    const program = gl.createProgram()
    // the browser took the context back: nothing more is drawn, and the plain background shows
    const lost = (): void => {
      cancelAnimationFrame(frame)
      canvas.hidden = true
    }
    const release = (): void => {
      cancelAnimationFrame(frame)
      canvas.removeEventListener('webglcontextlost', lost)
      for (const { tex, image } of textures) {
        // an image still on its way must not write into a texture that is gone
        image.onload = null
        gl.deleteTexture(tex)
      }
      gl.deleteBuffer(buffer)
      gl.deleteProgram(program)
      gl.deleteShader(vertex)
      gl.deleteShader(fragment)
      // the context goes with the canvas; a canvas still in the page is about to be set up again
      const loser = gl.getExtension('WEBGL_lose_context')
      setTimeout(() => {
        if (!canvas.isConnected) loser?.loseContext()
      }, 0)
    }
    if (vertex == null || fragment == null || program == null) return release
    gl.attachShader(program, vertex)
    gl.attachShader(program, fragment)
    gl.linkProgram(program)
    if (gl.getProgramParameter(program, gl.LINK_STATUS) !== true) {
      console.error('[map] the sea shader did not link', gl.getProgramInfoLog(program))
      return release
    }
    canvas.hidden = false
    canvas.addEventListener('webglcontextlost', lost)
    gl.useProgram(program)

    buffer = gl.createBuffer()
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW)
    const position = gl.getAttribLocation(program, 'aPosition')
    gl.enableVertexAttribArray(position)
    gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0)

    // still water for those who ask for less motion: drawn again only when the camera moves or an image arrives
    const still = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true
    let drawn = ''
    const redraw = (): void => {
      drawn = ''
    }
    // the height and the noise are read at full detail; the ground is mip-mapped
    textures.push(
      texture(gl, 0, OCEAN_ART.height, false, redraw),
      texture(gl, 1, OCEAN_ART.ground, true, redraw),
      texture(gl, 2, OCEAN_ART.noise, false, redraw)
    )
    gl.uniform1i(gl.getUniformLocation(program, 'uHeight'), 0)
    gl.uniform1i(gl.getUniformLocation(program, 'uGround'), 1)
    gl.uniform1i(gl.getUniformLocation(program, 'uNoise'), 2)
    const uCamera = gl.getUniformLocation(program, 'uCamera')
    const uOrtho = gl.getUniformLocation(program, 'uOrtho')
    const uResolution = gl.getUniformLocation(program, 'uResolution')
    const uTime = gl.getUniformLocation(program, 'uTime')

    const draw = (now: number): void => {
      frame = requestAnimationFrame(draw)
      const v = view.current
      const key = `${v.x},${v.y},${v.ortho},${v.width},${v.height}`
      if (still && key === drawn) return
      const rect = canvas.getBoundingClientRect()
      if (rect.width === 0 || rect.height === 0) return
      const scale = Math.min(1, MAX_WIDTH / rect.width)
      const width = Math.round(rect.width * scale)
      const height = Math.round(rect.height * scale)
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width
        canvas.height = height
        gl.viewport(0, 0, width, height)
      }
      drawn = key
      gl.uniform2f(uCamera, v.x, v.y)
      gl.uniform1f(uOrtho, v.ortho)
      gl.uniform2f(uResolution, width, height)
      gl.uniform1f(uTime, still ? 0 : now / 1000)
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4)
    }
    frame = requestAnimationFrame(draw)
    return release
  }, [view])

  return <canvas ref={ref} className={styles.ocean} aria-hidden="true" />
}
