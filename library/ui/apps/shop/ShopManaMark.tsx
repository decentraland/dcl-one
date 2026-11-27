import { GeneralMana, GeneralPolygonMana } from '../../foundations/icons/general'

type ShopManaMarkProps = {
  network?: 'ethereum' | 'polygon' | 'matic'
  size?: number
  className?: string
}

export function ShopManaMark({ network = 'ethereum', size = 16, className }: ShopManaMarkProps) {
  const Icon = network === 'polygon' || network === 'matic' ? GeneralPolygonMana : GeneralMana
  return <Icon size={size} className={className} aria-hidden="true" />
}
