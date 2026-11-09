import { Action, Connection, CreateQuest, Step, Task } from './types'

/**
 * The checks the Quests service applies to a quest definition, run before the quest is signed and
 * sent. They used to come from the @dcl/quests-client package (1.0.3, Apache-2.0), whose own @dcl/sdk
 * dependency pulled an older SDK into this package's dependency tree.
 */

const URL_PATTERN =
  /https?:\/\/(www\.)?[-a-zA-Z0-9@:%._+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()!@:%_+.~#?&/=]*)/

export function validateStep(step: Partial<Step>): boolean {
  if (!step?.id?.length) {
    throw new Error(`Step: ${JSON.stringify(step)} is missing an 'id'`)
  }
  if (!step?.description?.length) {
    throw new Error(`Step: ${step.id} is missing its description`)
  }
  if (!step.tasks?.length || !Array.isArray(step.tasks)) {
    throw new Error(`Step: ${step.id} contains invalid tasks`)
  }
  const ids = new Set<string>()
  for (const task of step.tasks) {
    validateTask(task)
    if (ids.has(task.id)) {
      throw new Error(`${step.id} is repeated. Each Task's id must be unique`)
    }
    ids.add(task.id)
  }
  return true
}

export function validateTask(task: Partial<Task>): boolean {
  if (!task?.id?.length) {
    throw new Error(`Task: ${JSON.stringify(task)} is missing an 'id'`)
  }
  if (!task?.description?.length) {
    throw new Error(`Task: ${task.id} is missing its description`)
  }
  if (!task?.actionItems?.length || !Array.isArray(task.actionItems)) {
    throw new Error(`Task: ${task.id} contains invalid action items`)
  }
  for (const action of task.actionItems) {
    validateActionItem(action)
  }
  return true
}

export function validateActionItem(action: Partial<Action>): boolean {
  const keys = Object.keys(action.parameters || {})
  switch (action.type) {
    case 'CUSTOM':
      if (!keys.length) {
        throw new Error(`'CUSTOM' Action must contain at least one parameter. eg: 'id'`)
      }
      return true
    case 'LOCATION':
      if (!keys.includes('x') || !keys.includes('y')) {
        throw new Error(`'LOCATION' Action must contain 'x' and 'y' parameters`)
      }
      return true
    case 'EMOTE':
      if (!keys.includes('x') || !keys.includes('y') || !keys.includes('id')) {
        throw new Error(`'EMOTE' Action must contain 'x', 'y', and 'id' parameters`)
      }
      return true
    case 'JUMP':
      if (!keys.includes('x') || !keys.includes('y')) {
        throw new Error(`'JUMP' Action must contain 'x' and 'y' parameters`)
      }
      return true
    default:
      throw new Error(`Invalid Action: ${JSON.stringify(action)}`)
  }
}

export function validateStepsAndConnections(quest: Partial<Pick<CreateQuest, 'definition'>>): boolean {
  if (!quest.definition) {
    throw new Error('Quest must have a definition')
  }
  validateSteps(quest.definition.steps || [])
  if (quest.definition.steps.length > 1) {
    validateConnections(quest.definition.connections || [])
  } else {
    // a single-step quest still carries its (empty) connections
    return Array.isArray(quest.definition.connections)
  }
  return true
}

export function validateConnections(connections: Partial<Connection>[]): boolean {
  if (!connections?.length || !Array.isArray(connections)) {
    throw new Error("Quest's definition must have its connections defined")
  }
  if (!connections.every((connection) => connection.stepFrom?.length && connection.stepTo?.length)) {
    throw new Error("Quest's definition must have valid connections")
  }
  if (!connections.every((connection) => connection.stepFrom !== connection.stepTo)) {
    throw new Error("Quest's connections are invalid. A Connection cannot go from and to the same step")
  }
  return true
}

export function validateSteps(steps: Partial<Step>[]): boolean {
  if (!steps?.length || !Array.isArray(steps)) {
    throw new Error("Quest's definition must have its steps defined")
  }
  const ids = new Set<string>()
  for (const step of steps) {
    validateStep(step)
    if (ids.has(step.id!)) {
      throw new Error(`${step.id} is repeated. Each Step's id must be unique`)
    }
    ids.add(step.id!)
  }
  return true
}

export function validateCreateQuest(quest: Partial<CreateQuest>): boolean {
  if (!quest.name || quest.name.length < 5) {
    throw new Error("Quest's name must be at least 5 chars")
  }
  if (!quest.description || quest.description.length < 5) {
    throw new Error("Quest's description must be at least 5 chars")
  }
  if (!quest.imageUrl?.length || !URL_PATTERN.test(quest.imageUrl)) {
    throw new Error("Quest's image URL must be a valid URL")
  }
  validateStepsAndConnections(quest)
  if (quest.reward) {
    if (!quest.reward.hook) {
      throw new Error("Quest's reward must have its webhook defined")
    }
    if (!quest.reward.hook.webhookUrl?.length || !URL_PATTERN.test(quest.reward.hook.webhookUrl)) {
      throw new Error("Quest's reward must have a valid Webhook URL")
    }
    if (!quest.reward.items?.length || !Array.isArray(quest.reward.items)) {
      throw new Error("Quest's reward must have its items defined")
    }
    if (!quest.reward.items.every((item) => URL_PATTERN.test(item.imageLink || '') && item?.name?.length >= 3)) {
      throw new Error("Quest's reward must have valid items")
    }
  }
  return true
}
