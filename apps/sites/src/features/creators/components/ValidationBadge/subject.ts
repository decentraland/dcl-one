import type { Item } from '@/data/lib/creators/items'
import type { ValidationSubject } from './ItemValidationCard'

/** A saved item as the validation results show it. */
export function toValidationSubject(item: Item): ValidationSubject {
  const thumbnail = item.contents[item.thumbnail]
  return {
    name: item.name,
    type: item.type,
    category: item.data.category,
    rarity: item.rarity,
    thumbnail: thumbnail ? { item, hash: thumbnail } : null
  }
}
