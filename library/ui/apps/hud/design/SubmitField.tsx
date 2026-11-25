// SubmitField — the sign-in screen's email box and NEXT, drawn by the shared SubmitField. `icon` and
// `errorIcon` are white-on-transparent images, tinted here with MaskIcon.
import { SubmitField as SharedSubmitField, type SubmitFieldProps as SharedProps } from '@dcl/one-ui'
import { MaskIcon } from './MaskIcon'

type SubmitFieldProps = Omit<SharedProps, 'icon' | 'errorIcon'> & {
  /** A white-on-transparent image, tinted like the placeholder. */
  icon?: string
  errorIcon?: string
}

export function SubmitField({ icon, errorIcon, ...rest }: SubmitFieldProps): React.JSX.Element {
  return (
    <SharedSubmitField
      {...rest}
      icon={icon != null ? <MaskIcon src={icon} size={24} /> : undefined}
      errorIcon={errorIcon != null ? <MaskIcon src={errorIcon} size={15} /> : undefined}
    />
  )
}
