import { describe, expect, it } from 'vitest'
import { chromiumBrowser, integratedGpuOnWindows } from './integratedGpu'
import type { GpuAdapterInfo } from './gpu'

const WINDOWS_CHROME = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36'
const WINDOWS_EDGE = WINDOWS_CHROME + ' Edg/142.0.0.0'
const WINDOWS_OPERA = WINDOWS_CHROME + ' OPR/120.0.0.0'
const MAC_CHROME = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36'
const LINUX_CHROME = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36'

const adapter = (vendor: string, description: string): GpuAdapterInfo => ({ vendor, architecture: '', device: '', description })

describe('integratedGpuOnWindows', () => {
  it('is an Intel adapter on Windows, by vendor or by the driver string', () => {
    expect(integratedGpuOnWindows(adapter('intel', 'Intel(R) Iris(R) Xe Graphics'), WINDOWS_CHROME)).toBe(true)
    expect(integratedGpuOnWindows(adapter('intel', ''), WINDOWS_EDGE)).toBe(true)
    expect(integratedGpuOnWindows(adapter('', 'Intel(R) UHD Graphics 630'), WINDOWS_CHROME)).toBe(true)
  })

  it('is not a discrete GPU (NVIDIA, AMD, Intel Arc): switching would change nothing', () => {
    expect(integratedGpuOnWindows(adapter('nvidia', 'NVIDIA GeForce RTX 4060 Laptop GPU'), WINDOWS_CHROME)).toBe(false)
    expect(integratedGpuOnWindows(adapter('amd', 'AMD Radeon RX 7600'), WINDOWS_CHROME)).toBe(false)
    expect(integratedGpuOnWindows(adapter('intel', 'Intel(R) Arc(TM) A770 Graphics'), WINDOWS_CHROME)).toBe(false)
  })

  it('is Windows only: elsewhere the browser already picks the GPU, and an "Intel Mac" is the CPU', () => {
    expect(integratedGpuOnWindows(adapter('intel', 'Intel(R) Iris(R) Plus Graphics'), MAC_CHROME)).toBe(false)
    expect(integratedGpuOnWindows(adapter('intel', 'Intel(R) UHD Graphics'), LINUX_CHROME)).toBe(false)
  })

  it('needs adapter info: a probe that timed out or was skipped tells nothing', () => {
    expect(integratedGpuOnWindows(null, WINDOWS_CHROME)).toBe(false)
  })
})

describe('chromiumBrowser', () => {
  it('names the browser and its own flags page; unmarked Chromium reads as Chrome', () => {
    expect(chromiumBrowser(WINDOWS_CHROME)).toEqual({ name: 'Chrome', flagsUrl: 'chrome://flags/#force-high-performance-gpu' })
    expect(chromiumBrowser(WINDOWS_EDGE)).toEqual({ name: 'Edge', flagsUrl: 'edge://flags/#force-high-performance-gpu' })
    expect(chromiumBrowser(WINDOWS_OPERA)).toEqual({ name: 'Opera', flagsUrl: 'opera://flags/#force-high-performance-gpu' })
  })
})
