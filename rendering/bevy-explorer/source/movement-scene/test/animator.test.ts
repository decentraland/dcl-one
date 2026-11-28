import assert from 'node:assert/strict'
import { test } from 'node:test'
import { Animator, AnimatorController, Condition, State, Transition } from '../src/animator'

// Small state machines: every state plays its own looping clip of 1 s, so normalized time is seconds.
const on = (param: string): Condition[] => [[param, 'if']]
const go = (to: string, o: Partial<Transition> = {}): Transition => ({ to, duration: 0, fixed: true, ...o })
function machine(states: Record<string, Transition[]>, any: Transition[] = []): AnimatorController {
  const entries = Object.entries(states)
  return {
    entry: entries[0][0],
    any,
    states: Object.fromEntries(entries.map(([name, transitions]): [string, State] => [name, { motion: { clip: name }, transitions }])),
    clips: Object.fromEntries(entries.map(([name]) => [name, { length: 1, loop: true }])),
  }
}

test('an Any State transition is taken before the current state\'s own', () => {
  const a = new Animator(machine({ A: [go('B', { when: on('go') })], B: [], C: [] }, [go('C', { when: on('go') })]))
  assert.equal(a.update({ go: true }, 0.1).state, 'C')
})

test('an Any State transition that may not transition to itself is skipped in its own destination', () => {
  const a = new Animator(machine({ A: [go('B', { when: on('toB') })], B: [] }, [go('A', { when: on('toA'), noSelf: true })]))
  assert.equal(a.update({ toA: true }, 0.1).entered, false)
  assert.equal(a.update({ toB: true }, 0.1).state, 'B')
  assert.equal(a.update({ toA: true }, 0.1).state, 'A')
})

// A -> B with a cross-fade of 0.5 s; then B -> C, or A's other transition A -> D, during that cross-fade.
function crossFading(interruption?: 'source' | 'destination') {
  const a = new Animator(machine({
    A: [go('B', { when: on('toB'), duration: 0.5, interruption }), go('D', { when: on('toD') })],
    B: [go('C', { when: on('toC') })],
    C: [],
    D: [],
  }))
  a.update({ toB: true }, 0.1)
  return a
}

test('during a cross-fade with no interruption source no transition starts', () => {
  const a = crossFading()
  assert.equal(a.update({ toC: true, toD: true }, 0.1).state, 'B')
  assert.equal(a.update({}, 0.5).state, 'B')
  assert.equal(a.update({ toC: true }, 0.1).state, 'C')
})

test('a transition that a condition starts waits for the update after the one that ends the cross-fade', () => {
  const a = crossFading()
  a.update({}, 0.4)
  // the update from 0.4 s to 0.5 s ends the 0.5 s cross-fade into B
  assert.equal(a.update({ toC: true }, 0.1).state, 'B')
  assert.equal(a.update({ toC: true }, 0.1).state, 'C')
})

test('a cross-fade that the destination interrupts lets the destination\'s transitions start', () => {
  const a = crossFading('destination')
  assert.equal(a.update({ toD: true }, 0.1).state, 'B')
  assert.equal(a.update({ toC: true }, 0.1).state, 'C')
})

test('a cross-fade that the source interrupts lets the source\'s other transitions start, not the destination\'s', () => {
  const a = crossFading('source')
  assert.equal(a.update({ toC: true }, 0.1).state, 'B')
  assert.equal(a.update({ toB: true }, 0.1).entered, false)
  assert.equal(a.update({ toD: true }, 0.1).state, 'D')
})

// A -> B with a cross-fade of `fade` s; B leaves to C at the exit time 0.5. Returns the update of `dt` s, counted from
// the one that starts B, that starts C.
function exitUpdate(fade: number, dt = 0.25): number {
  const a = new Animator(machine({ A: [go('B', { when: on('toB'), duration: fade })], B: [go('C', { exitTime: 0.5 })], C: [] }))
  a.update({ toB: true }, dt)
  for (let n = 1; n <= 12; n++) if (a.update({}, dt).state === 'C') return n
  return NaN
}

test('an exit time crossed inside the cross-fade waits for the next cycle', () => {
  // the crossing at 0.5 s falls inside a 0.6 s cross-fade; the next one is at 1.5 s
  assert.equal(exitUpdate(0.6), 6)
  // so does one inside the update that ends the cross-fade: that update ends at 0.75 s, the next at 1.5 s
  assert.equal(exitUpdate(0.6, 0.75), 2)
})

test('an exit time crossed after the cross-fade is taken on the update that ends the cross-fade', () => {
  // the update from 0.25 s to 0.5 s ends a 0.4 s cross-fade and then crosses 0.5 s
  assert.equal(exitUpdate(0.4), 2)
})

test('a trigger stays set until a transition that has it among its conditions starts', () => {
  const c = machine({ A: [go('B', { when: [['go', 'if'], ['ready', 'if']] })], B: [go('A', { when: on('go') })] })
  const a = new Animator({ ...c, triggers: ['go'] })
  assert.equal(a.update({ go: true, ready: false }, 0.1).state, 'A')
  assert.equal(a.update({ ready: true }, 0.1).state, 'B', 'the trigger set on the update before still counts')
  assert.equal(a.update({}, 0.1).state, 'B', 'the transition into B reset it')
})

test('a parameter that is not a trigger counts only on the updates that pass it', () => {
  const a = new Animator(machine({ A: [go('B', { when: [['go', 'if'], ['ready', 'if']] })], B: [] }))
  assert.equal(a.update({ go: true, ready: false }, 0.1).state, 'A')
  assert.equal(a.update({ ready: true }, 0.1).state, 'A')
})
