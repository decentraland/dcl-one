import { type BodyShape } from '@dcl/schemas'
import { type Item, type ItemType } from '../items'

export enum ValidationSeverity {
  ERROR = 'error',
  WARNING = 'warning'
}

/** An item with the issues found on it, as the publish gate and the review findings list them. */
export type ItemCheck = { item: Item; issues: ValidationIssue[] }

/** One problem found in an item, with the rule book's creator-facing English message. */
export type ValidationIssue = {
  code: string
  severity: ValidationSeverity
  message: string
  /** The rule book's name for the check, e.g. "Triangle count". */
  title?: string
  /** File, mesh or bone the issue points at. */
  where?: string
  /** Set when the item has a separate model per body shape and only these shapes' model has the issue. */
  bodyShapes?: BodyShape[]
}

/** One model to check and the body shapes that wear it; `contents` scopes its audio files (default: all of them). */
export type ModelFile = { mainFile: string; bodyShapes: BodyShape[]; contents?: string[] }

/**
 * What to validate: in-memory files (imports, live preview) or a saved item (storage URLs). Every distinct
 * model is checked: a blob's `representations` when given (else `mainFile` for both body shapes), or all of
 * a saved item's representations.
 */
export type ValidationSource =
  | { kind: 'blob'; contents: Record<string, Blob>; mainFile: string; representations?: ModelFile[] }
  | { kind: 'item'; item: Item }

/** A thumbnail to check: an in-memory PNG (imports, unsaved edits) or a saved item's stored one. */
export type ThumbnailSource = { kind: 'blob'; blob: Blob } | { kind: 'item'; item: Item }

export type ValidationContext = {
  type: ItemType
  category?: string
  hides?: string[]
  /** Emotes: whether it plays in a loop, which the loop-seam check needs. */
  loop?: boolean
}

export type ValidationResult = { issues: ValidationIssue[] }

export type ValidateOptions = { signal?: AbortSignal }

export interface ItemValidator {
  validate(source: ValidationSource, ctx: ValidationContext, opts?: ValidateOptions): Promise<ValidationResult>
  /** The thumbnail rule alone: format, size and background transparency. */
  validateThumbnail(source: ThumbnailSource, opts?: ValidateOptions): Promise<ValidationResult>
}
