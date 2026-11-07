/**
 * The module ecs is exposed by the sdk via `@dcl/sdk/ecs`
 *
 * It defines the engine, components & systems for the SDK 7.

* @example
 * ```tsx
 * import { engine, Transform } from '@dcl/sdk/ecs'
 * const entity = engine.addEntity()
 * Transform.create(entity, defaultPosition)
 * ```
 *
 * @module ECS
 *
 */

import { polyfillTextEncoder } from './text-codec'

// @dcl/ecs decodes protobuf messages with @bufbuild/protobuf, which needs the Encoding API on globalThis
polyfillTextEncoder()

export * from '@dcl/ecs'
