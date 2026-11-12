import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'
import { emailState, explorerState, searchState } from '../src/review/InteractivePreview.helpers.ts'

describe('when editing an email in the playground', () => {
  it('should move from empty focus to editing and valid completion', () => {
    assert.equal(emailState('', false, false), 'default')
    assert.equal(emailState('', true, false), 'focus')
    assert.equal(emailState('person@example.com', true, false), 'editing')
    assert.equal(emailState('person@example.com', false, true), 'completed')
  })
  it('should show validation after blur without treating an empty optional field as invalid', () => {
    assert.equal(emailState('invalid', false, true), 'error')
    assert.equal(emailState('invalid', false, false), 'default')
    assert.equal(emailState('', false, true), 'default')
    assert.equal(emailState('person @example.com', false, true), 'error')
  })
})

describe('when editing an Explorer name in the playground', () => {
  it('should retain the actual editing and completed source variants', () => {
    assert.equal(explorerState('', false, false, false), 'empty')
    assert.equal(explorerState('', true, false, false), 'focusedEdit')
    assert.equal(explorerState('Carolo', true, false, false), 'focusedEditing')
    assert.equal(explorerState('Carolo', false, false, false), 'edited')
  })
  it('should accept fifteen characters and show the source error at sixteen', () => {
    assert.equal(explorerState('a'.repeat(15), true, false, false), 'focusedEditing')
    assert.equal(explorerState('a'.repeat(16), true, false, false), 'error')
    assert.equal(explorerState('a'.repeat(16), false, false, false), 'error')
  })
  it('should keep password visibility separate from the name character limit', () => {
    assert.equal(explorerState('', false, true, false), 'password')
    assert.equal(explorerState('a'.repeat(16), true, true, false), 'passwordFocusedEditingHidden')
    assert.equal(explorerState('a'.repeat(16), true, true, true), 'passwordFocusedEditingShown')
    assert.equal(explorerState('secret', false, true, true), 'passwordFocusedEditingShown')
  })
})

describe('when searching in the playground', () => {
  it('should show recent searches on focus and predictions while typing', () => {
    assert.equal(searchState('', false, false), 'default')
    assert.equal(searchState('', true, false), 'history')
    assert.equal(searchState('genesis', true, false), 'predictive')
    assert.equal(searchState('genesis', false, false), 'default')
  })
  it('should show no results only after submitting a non-blank query', () => {
    assert.equal(searchState('genesis', true, true), 'notFound')
    assert.equal(searchState('   ', true, true), 'predictive')
    assert.equal(searchState('', true, true), 'history')
  })
})
