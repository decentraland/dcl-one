import assert from 'node:assert/strict'
import { test } from 'node:test'
import { gpuPageFilled } from '../scripts/gpu-page.ts'

// chrome://gpu as a probe read it in CI before the GPU process answered: the headings are there,
// every section still says it is loading, and no adapter is listed yet.
const LOADING = [
  'GPU Internals', 'Graphics Feature Status', '=======================',
  'Version Information', '===================', '... loading ...',
  'Driver Information', '==================', '... loading ...',
  'Dawn Info', 'Log Messages', '============'
].join('\n')

const FILLED = [
  'GPU Internals', 'Graphics Feature Status', '=======================',
  'WebGPU: Hardware accelerated', 'Driver Information', '==================',
  'GPU0 VENDOR= 0x1414, DEVICE=0x008c [Microsoft Basic Render Driver], LUID={0,47098} *ACTIVE*',
  'GPU1 VENDOR= 0x10de, DEVICE=0x1eb8 [NVIDIA Tesla T4], LUID={0,47535}',
  'Dawn Info', 'Log Messages', '============'
].join('\n')

test('a chrome://gpu page whose sections are still loading is not filled in, though its Dawn Info heading shows', () => {
  assert.equal(gpuPageFilled(LOADING), false)
})

test('a chrome://gpu page with the GPU process answer and its adapters is filled in', () => {
  assert.equal(gpuPageFilled(FILLED), true)
})

test('a page before the feature status renders is not filled in', () => {
  assert.equal(gpuPageFilled('GPU Internals\nDawn Info'), false)
})
