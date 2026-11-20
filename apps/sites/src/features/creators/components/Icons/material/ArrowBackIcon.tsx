import { MaterialIcon, type MaterialIconProps } from './MaterialIcon'

export function ArrowBackIcon(props: MaterialIconProps) {
  return (
    <MaterialIcon {...props}>
      <path d="M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20z" />
    </MaterialIcon>
  )
}
