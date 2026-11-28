import { useId, type ComponentProps, type ReactNode } from 'react'

import { FieldChrome, fieldMessageIds } from './InputField'
import styles from './InputField.module.css'

/**
 * A visible label, with a description, error and hint, laid out as InputWebCreatorTools lays out its own. The
 * messages need the label, so whether the field has one decides its markup once: without it the control renders
 * alone in its frame, and the caller names it.
 */
type WebFieldLabel =
  | {
      /** The visible name above the control. A node lets a form put its own content in it, such as a required mark. */
      label: ReactNode
      labelHidden?: boolean
      description?: string
      error?: string
      hint?: string
    }
  | { label?: undefined; labelHidden?: never; description?: never; error?: never; hint?: never }

type WebFieldProps = WebFieldLabel & {
  /** Placement on the outermost element: the field with its label, or the frame alone. `className` styles the control. */
  rootClassName?: string
}

export type WebSelectProps = ComponentProps<'select'> & WebFieldProps
export type WebTextareaProps = ComponentProps<'textarea'> & WebFieldProps

const frameClass = `${styles.field} ${styles.webField} ${styles.nativeField}`

function isInvalid(value: WebSelectProps['aria-invalid']) {
  return Boolean(value && value !== 'false')
}

function WebField({
  id,
  label,
  labelHidden = false,
  description,
  error,
  hint,
  rootClassName,
  describedBy,
  invalid,
  control
}: {
  label: ReactNode
  labelHidden: boolean | undefined
  description: string | undefined
  error: string | undefined
  hint: string | undefined
  rootClassName: string | undefined
  id: string | undefined
  describedBy: string | undefined
  invalid: WebSelectProps['aria-invalid']
  control: (aria: { id?: string; 'aria-describedby'?: string; 'aria-invalid'?: WebSelectProps['aria-invalid'] }) => ReactNode
}) {
  const generatedId = useId()
  if (label === undefined) {
    return (
      <div
        className={rootClassName ? `${frameClass} ${rootClassName}` : frameClass}
        data-invalid={isInvalid(invalid) || undefined}
      >
        {control({})}
      </div>
    )
  }
  const controlId = id ?? generatedId
  const { descriptionId, errorId, hintId } = fieldMessageIds(controlId, { description, error, hint })
  const ids = [describedBy, descriptionId, errorId, hintId].filter(Boolean).join(' ') || undefined
  return (
    <FieldChrome
      surface="web"
      inputId={controlId}
      label={label}
      labelHidden={labelHidden}
      description={description}
      error={error}
      hint={hint}
      className={rootClassName}
    >
      <div className={frameClass} data-invalid={!!errorId || isInvalid(invalid) || undefined}>
        {control({ id: controlId, 'aria-describedby': ids, 'aria-invalid': errorId ? true : invalid })}
      </div>
    </FieldChrome>
  )
}

export function WebSelect({
  className,
  label,
  labelHidden,
  description,
  error,
  hint,
  rootClassName,
  ...props
}: WebSelectProps) {
  return (
    <WebField
      id={props.id}
      label={label}
      labelHidden={labelHidden}
      description={description}
      error={error}
      hint={hint}
      rootClassName={rootClassName}
      describedBy={props['aria-describedby']}
      invalid={props['aria-invalid']}
      control={(aria) => (
        <select
          {...props}
          {...aria}
          className={[styles.input, styles.nativeSelect, className].filter(Boolean).join(' ')}
        />
      )}
    />
  )
}

export function WebTextarea({
  className,
  label,
  labelHidden,
  description,
  error,
  hint,
  rootClassName,
  ...props
}: WebTextareaProps) {
  return (
    <WebField
      id={props.id}
      label={label}
      labelHidden={labelHidden}
      description={description}
      error={error}
      hint={hint}
      rootClassName={rootClassName}
      describedBy={props['aria-describedby']}
      invalid={props['aria-invalid']}
      control={(aria) => (
        <textarea
          {...props}
          {...aria}
          className={[styles.input, styles.nativeTextarea, className].filter(Boolean).join(' ')}
        />
      )}
    />
  )
}
