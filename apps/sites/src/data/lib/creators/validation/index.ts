// Single entry point for item validation (see README.md). The backend is swappable: today the local
// rule book, tomorrow a remote service behind the same interface.
import { localValidator } from './localValidator'
import { type ItemValidator } from './types'

export type {
  ItemCheck,
  ItemValidator,
  ModelFile,
  ThumbnailSource,
  ValidateOptions,
  ValidationContext,
  ValidationIssue,
  ValidationResult,
  ValidationSource
} from './types'
export { ValidationSeverity } from './types'
export { distinctModels, itemModels, itemValidationContext, type DistinctModel } from './models'
export { cacheKey, cachedRun, deleteCached } from './cache'
export { countIssues, getValidationStatus, hasErrors, type ValidationStatus } from './status'

let validator: ItemValidator = localValidator

export function getValidator(): ItemValidator {
  return validator
}

/** Swap point for another backend (a remote service, a test double). */
export function setValidator(next: ItemValidator): void {
  validator = next
}
