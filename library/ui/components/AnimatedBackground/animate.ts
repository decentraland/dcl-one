import overlayTextureUrl from '../../assets/logo-pattern.webp'
import { FRAGMENT_SHADER, VERTEX_SHADER } from './shaders'
import { createProgram, createShader, loadTexture } from './webgl'

const DEFAULT_PATTERN_TILING = 1.66

// The animation AnimatedBackground runs in `root`, kept free of React so the static 404 page
// (apps/sites/scripts/gen-error-pages.mts) runs this same code from an inline script. Returns the cleanup.
// `tiling` is read every frame, so tiling changes apply live without rebuilding the GL state.
function animateBackground(
  root: HTMLElement,
  canvasClassName: string,
  tiling: () => number = () => DEFAULT_PATTERN_TILING
): () => void {
  // Respect reduced motion — keep the static fallback, skip the animation loop entirely.
  if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return () => {}

  // A fresh canvas per mount: cleanup gives the context back, and a canvas keeps returning its lost
  // context, so a remount (StrictMode, a re-keyed parent) on the same canvas could never draw again.
  const canvas = document.createElement('canvas')
  canvas.className = canvasClassName
  const gl = canvas.getContext('webgl', { alpha: false, antialias: false })
  if (!gl) return () => {} // fallback image stays visible
  root.appendChild(canvas)

  // An opaque context that never draws would cover the still image: give the context back and drop the canvas.
  const giveUp = () => {
    gl.getExtension('WEBGL_lose_context')?.loseContext()
    canvas.remove()
  }

  const vs = createShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER)
  const fs = createShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER)
  if (!vs || !fs) {
    if (vs) gl.deleteShader(vs)
    if (fs) gl.deleteShader(fs)
    giveUp()
    return () => {}
  }

  const program = createProgram(gl, vs, fs)
  if (!program) {
    gl.deleteShader(vs)
    gl.deleteShader(fs)
    giveUp()
    return () => {}
  }

  const positionLoc = gl.getAttribLocation(program, 'a_position')
  const timeLoc = gl.getUniformLocation(program, 'u_time')
  const resolutionLoc = gl.getUniformLocation(program, 'u_resolution')
  const overlayTexLoc = gl.getUniformLocation(program, 'u_overlayTex')
  const overlayTilingLoc = gl.getUniformLocation(program, 'u_overlayTiling')

  const buffer = gl.createBuffer()
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
  gl.bufferData(
    gl.ARRAY_BUFFER,
    new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
    gl.STATIC_DRAW
  )

  // If we unmount before the texture image decodes, skip its (now stale) GL upload.
  let disposed = false
  const overlayTexture = loadTexture(gl, overlayTextureUrl, () => disposed)

  const resize = () => {
    const dpr = window.devicePixelRatio || 1
    // Whole pixels: the canvas stores integers, so a fractional size (DPR 1.25, 1.5, …) would never compare equal
    // and the drawing buffer would be reallocated every frame.
    const w = Math.round(canvas.clientWidth * dpr)
    const h = Math.round(canvas.clientHeight * dpr)
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w
      canvas.height = h
    }
  }

  const startTime = performance.now() / 1000
  let frame = 0

  const render = () => {
    resize()
    gl.viewport(0, 0, canvas.width, canvas.height)
    gl.useProgram(program)

    gl.uniform1f(timeLoc, performance.now() / 1000 - startTime)
    gl.uniform2f(resolutionLoc, canvas.width, canvas.height)
    gl.uniform1f(overlayTilingLoc, tiling())

    gl.activeTexture(gl.TEXTURE0)
    gl.bindTexture(gl.TEXTURE_2D, overlayTexture)
    gl.uniform1i(overlayTexLoc, 0)

    gl.enableVertexAttribArray(positionLoc)
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
    gl.vertexAttribPointer(positionLoc, 2, gl.FLOAT, false, 0, 0)

    gl.drawArrays(gl.TRIANGLES, 0, 6)

    frame = requestAnimationFrame(render)
  }

  // If the GPU reclaims the context (mobile memory pressure), stop the now no-op loop and reveal the
  // static fallback (behind the canvas) instead of spinning rAF forever.
  const onContextLost = () => {
    cancelAnimationFrame(frame)
    canvas.remove()
  }
  canvas.addEventListener('webglcontextlost', onContextLost, false)

  frame = requestAnimationFrame(render)

  return () => {
    disposed = true
    cancelAnimationFrame(frame)
    canvas.removeEventListener('webglcontextlost', onContextLost)
    gl.deleteProgram(program)
    gl.deleteShader(vs)
    gl.deleteShader(fs)
    gl.deleteBuffer(buffer)
    gl.deleteTexture(overlayTexture)
    // Release the context itself, not only its objects: a tile that scrolls in and out mounts a fresh canvas each
    // time, and browsers drop their OLDEST live context (here the avatar preview's) past about 16 of them.
    gl.getExtension('WEBGL_lose_context')?.loseContext()
    canvas.remove()
  }
}

export { animateBackground, DEFAULT_PATTERN_TILING }
