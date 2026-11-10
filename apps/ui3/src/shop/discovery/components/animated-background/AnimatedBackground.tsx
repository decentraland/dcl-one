// from: shop@3a322cf9:app/src/components/AnimatedBackground/AnimatedBackground.tsx (+ .styles.ts), kept as upstream
//       writes it (quotes, no semicolons) so it diffs line for line against the original; here it also gives its
//       context back on unmount and on a failed setup, and sizes the buffer in whole pixels.

import { useEffect, useRef, useState } from 'react'
import fallbackBackgroundUrl from './custom-welcome-background.webp'
import overlayTextureUrl from './DCL_LogoPattern.png'
import { FRAGMENT_SHADER, VERTEX_SHADER } from './shaders'
import { createProgram, createShader, loadTexture } from './webgl'
import styles from './AnimatedBackground.module.css'

// Ported from decentraland/auth (src/components/AnimatedBackground): a WebGL purple vignette with the
// DCL logo pattern drifting across it. The static image is the fallback while WebGL boots — or the only
// thing shown when WebGL is unavailable or the user prefers reduced motion.
export default function AnimatedBackground({ patternTiling = 1.66 }: { patternTiling?: number }) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const contextRef = useRef<WebGLRenderingContext | null>(null)
  const [reducedMotion, setReducedMotion] = useState(false)
  const animFrameRef = useRef<number>(0)
  // Read by the render loop each frame, so tiling changes apply live without rebuilding the GL state.
  const tilingRef = useRef(patternTiling)
  tilingRef.current = patternTiling

  useEffect(() => {
    const preference = window.matchMedia?.('(prefers-reduced-motion: reduce)')
    const update = () => setReducedMotion(preference?.matches ?? false)
    update()
    preference?.addEventListener('change', update)
    return () => preference?.removeEventListener('change', update)
  }, [])

  useEffect(() => {
    const canvas = canvasRef.current
    return () => {
      // StrictMode replays effects on the same connected canvas. Losing its context there makes
      // the next setup fail; an actual unmount disconnects it before this microtask runs.
      queueMicrotask(() => {
        const gl = contextRef.current
        if (canvas && !canvas.isConnected && gl && !gl.isContextLost()) gl.getExtension('WEBGL_lose_context')?.loseContext()
      })
    }
  }, [])

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    // Keep the fallback visible until a frame draws, including while motion is disabled.
    canvas.style.opacity = '0'
    if (reducedMotion || window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return

    const gl = canvas.getContext('webgl', { alpha: false, antialias: false })
    if (!gl) return // fallback image stays visible
    contextRef.current = gl

    // An opaque context that never draws would cover the still image: hide the canvas and give the context back.
    const giveUp = () => {
      canvas.style.opacity = '0'
      gl.getExtension('WEBGL_lose_context')?.loseContext()
    }

    const vs = createShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER)
    const fs = createShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER)
    if (!vs || !fs) {
      if (vs) gl.deleteShader(vs)
      if (fs) gl.deleteShader(fs)
      giveUp()
      return
    }

    const program = createProgram(gl, vs, fs)
    if (!program) {
      gl.deleteShader(vs)
      gl.deleteShader(fs)
      giveUp()
      return
    }

    const positionLoc = gl.getAttribLocation(program, 'a_position')
    const timeLoc = gl.getUniformLocation(program, 'u_time')
    const resolutionLoc = gl.getUniformLocation(program, 'u_resolution')
    const overlayTexLoc = gl.getUniformLocation(program, 'u_overlayTex')
    const overlayTilingLoc = gl.getUniformLocation(program, 'u_overlayTiling')

    const buffer = gl.createBuffer()
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]), gl.STATIC_DRAW)

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

    const render = () => {
      resize()
      gl.viewport(0, 0, canvas.width, canvas.height)
      gl.useProgram(program)

      gl.uniform1f(timeLoc, performance.now() / 1000 - startTime)
      gl.uniform2f(resolutionLoc, canvas.width, canvas.height)
      gl.uniform1f(overlayTilingLoc, tilingRef.current)

      gl.activeTexture(gl.TEXTURE0)
      gl.bindTexture(gl.TEXTURE_2D, overlayTexture)
      gl.uniform1i(overlayTexLoc, 0)

      gl.enableVertexAttribArray(positionLoc)
      gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
      gl.vertexAttribPointer(positionLoc, 2, gl.FLOAT, false, 0, 0)

      gl.drawArrays(gl.TRIANGLES, 0, 6)
      canvas.style.opacity = '1'

      animFrameRef.current = requestAnimationFrame(render)
    }

    // If the GPU reclaims the context (mobile memory pressure), stop the now no-op loop and reveal the
    // static fallback (behind the canvas) instead of spinning rAF forever.
    const onContextLost = (e: Event) => {
      e.preventDefault()
      cancelAnimationFrame(animFrameRef.current)
      canvas.style.opacity = '0'
    }
    canvas.addEventListener('webglcontextlost', onContextLost, false)

    animFrameRef.current = requestAnimationFrame(render)

    return () => {
      disposed = true
      cancelAnimationFrame(animFrameRef.current)
      canvas.removeEventListener('webglcontextlost', onContextLost)
      gl.deleteProgram(program)
      gl.deleteShader(vs)
      gl.deleteShader(fs)
      gl.deleteBuffer(buffer)
      gl.deleteTexture(overlayTexture)
      canvas.style.opacity = '0'
    }
  }, [reducedMotion])

  return (
    <div className={styles.root} aria-hidden>
      <div className={styles.fallback} style={{ backgroundImage: `url(${fallbackBackgroundUrl})` }} />
      <canvas className={styles.canvas} ref={canvasRef} />
    </div>
  )
}
