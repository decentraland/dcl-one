import fs from 'fs'
import path from 'path'
import {
  validateActionItem,
  validateCreateQuest,
  validateStepsAndConnections
} from '../../../../packages/@dcl/sdk-commands/src/commands/quests/validation'

const fixture = (name: string) => JSON.parse(fs.readFileSync(path.resolve(__dirname, name), 'utf-8'))

const task = (id: string) => ({ id, description: `task ${id}`, actionItems: [{ type: 'CUSTOM', parameters: { id } }] })
const step = (id: string, tasks = [task(`${id}-1`)]) => ({ id, description: `step ${id}`, tasks })
const reward = (overrides: Record<string, unknown> = {}) => ({
  hook: { webhookUrl: 'https://example.org/hook' },
  items: [{ name: 'hat', imageLink: 'https://example.org/hat.png' }],
  ...overrides
})
const quest = (overrides: Record<string, unknown> = {}) => ({
  name: 'A fine quest',
  description: 'Collect the things',
  imageUrl: 'https://example.org/quest.png',
  definition: { steps: [step('a'), step('b')], connections: [{ stepFrom: 'a', stepTo: 'b' }] },
  ...overrides
})

describe('validateCreateQuest', () => {
  it('should accept the valid fixture and refuse the invalid one', () => {
    expect(validateCreateQuest(fixture('valid_quest.json'))).toBe(true)
    expect(() => validateCreateQuest(fixture('invalid_quest.json'))).toThrow()
  })

  it('should accept a quest with a reward', () => {
    expect(validateCreateQuest(quest({ reward: reward() }) as any)).toBe(true)
  })

  it.each([
    ['a short name', { name: 'abc' }, "Quest's name must be at least 5 chars"],
    ['a short description', { description: 'abc' }, "Quest's description must be at least 5 chars"],
    ['an image that is not a URL', { imageUrl: 'quest.png' }, "Quest's image URL must be a valid URL"],
    ['no definition', { definition: undefined }, 'Quest must have a definition'],
    ['no steps', { definition: { steps: [], connections: [] } }, 'must have its steps defined'],
    [
      'a repeated step id',
      { definition: { steps: [step('a'), step('a')], connections: [{ stepFrom: 'a', stepTo: 'a' }] } },
      "a is repeated. Each Step's id must be unique"
    ],
    [
      'a repeated task id',
      { definition: { steps: [step('a', [task('t'), task('t')])], connections: [] } },
      "a is repeated. Each Task's id must be unique"
    ],
    [
      'a step without a description',
      { definition: { steps: [{ id: 'a', tasks: [task('t')] }], connections: [] } },
      'Step: a is missing its description'
    ],
    [
      'a task without action items',
      { definition: { steps: [step('a', [{ id: 't', description: 'd', actionItems: [] }])], connections: [] } },
      'Task: t contains invalid action items'
    ],
    [
      'several steps without connections',
      { definition: { steps: [step('a'), step('b')], connections: [] } },
      'must have its connections defined'
    ],
    [
      'a connection missing an end',
      { definition: { steps: [step('a'), step('b')], connections: [{ stepFrom: 'a' }] } },
      'must have valid connections'
    ],
    [
      'a connection from a step to itself',
      { definition: { steps: [step('a'), step('b')], connections: [{ stepFrom: 'a', stepTo: 'a' }] } },
      'cannot go from and to the same step'
    ],
    ['a reward without a webhook', { reward: reward({ hook: undefined }) }, 'must have its webhook defined'],
    ['a reward with a bad webhook', { reward: reward({ hook: { webhookUrl: 'hook' } }) }, 'must have a valid Webhook URL'],
    ['a reward without items', { reward: reward({ items: [] }) }, 'must have its items defined'],
    [
      'a reward item with a short name',
      { reward: reward({ items: [{ name: 'ab', imageLink: 'https://example.org/hat.png' }] }) },
      'must have valid items'
    ],
    [
      'a reward item without an image',
      { reward: reward({ items: [{ name: 'hat' }] }) },
      'must have valid items'
    ]
  ])('should refuse %s', (_name, overrides, message) => {
    expect(() => validateCreateQuest(quest(overrides) as any)).toThrow(message)
  })
})

describe('validateStepsAndConnections', () => {
  it('should ask for the connections array even on a single-step quest', () => {
    expect(validateStepsAndConnections({ definition: { steps: [step('a')], connections: [] } })).toBe(true)
    expect(validateStepsAndConnections({ definition: { steps: [step('a')] } as any })).toBe(false)
  })

  it('should accept connected steps', () => {
    expect(validateStepsAndConnections(quest())).toBe(true)
  })
})

describe('validateActionItem', () => {
  it.each([
    ['CUSTOM', { id: 'x' }],
    ['LOCATION', { x: '1', y: '2' }],
    ['EMOTE', { x: '1', y: '2', id: 'wave' }],
    ['JUMP', { x: '1', y: '2' }]
  ])('should accept a complete %s action', (type, parameters) => {
    expect(validateActionItem({ type, parameters })).toBe(true)
  })

  it.each([
    ['CUSTOM', {}, 'at least one parameter'],
    ['LOCATION', { x: '1' }, "'x' and 'y' parameters"],
    ['EMOTE', { x: '1', y: '2' }, "'x', 'y', and 'id' parameters"],
    ['JUMP', { y: '2' }, "'x' and 'y' parameters"],
    ['DANCE', { x: '1' }, 'Invalid Action']
  ])('should refuse an incomplete %s action', (type, parameters, message) => {
    expect(() => validateActionItem({ type, parameters })).toThrow(message)
  })
})
