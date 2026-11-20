import { ValidationSeverity, type ValidationIssue } from './types'

export type ValidationStatus = 'idle' | 'loading' | 'pass' | 'warnings' | 'errors'

export function hasErrors(issues: ValidationIssue[]): boolean {
  return issues.some(issue => issue.severity === ValidationSeverity.ERROR)
}

export function getValidationStatus(issues: ValidationIssue[] | undefined, isLoading: boolean): ValidationStatus {
  if (isLoading) return 'loading'
  if (!issues) return 'idle'
  if (hasErrors(issues)) return 'errors'
  return issues.length > 0 ? 'warnings' : 'pass'
}

export function countIssues(issues: ValidationIssue[]): { errors: number; warnings: number } {
  const errors = issues.filter(issue => issue.severity === ValidationSeverity.ERROR).length
  return { errors, warnings: issues.length - errors }
}
