import { MaterialIcon, type MaterialIconProps } from './MaterialIcon'

export function GridViewIcon(props: MaterialIconProps) {
  return (
    <MaterialIcon {...props}>
      <path
        fillRule="evenodd"
        d="M3 3v8h8V3zm6 6H5V5h4zm-6 4v8h8v-8zm6 6H5v-4h4zm4-16v8h8V3zm6 6h-4V5h4zm-6 4v8h8v-8zm6 6h-4v-4h4z"
      />
    </MaterialIcon>
  )
}
