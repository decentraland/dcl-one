import { z } from 'zod'

const inventoryEntrySchema = z.object({
  pageId: z.string().min(1),
  componentIds: z.array(z.string().min(1)).optional()
})
const generationInventorySchema = z
  .object({
    schema: z.literal(1),
    review: z.array(inventoryEntrySchema),
    promotion: z.array(inventoryEntrySchema)
  })
  .superRefine((inventory, context) => {
    for (const scope of ['review', 'promotion'] as const) {
      const ids = inventory[scope].map((entry) => entry.pageId)
      if (new Set(ids).size !== ids.length)
        context.addIssue({ code: 'custom', message: `Duplicate ${scope} inventory page` })
    }
    for (const entry of inventory.promotion) {
      const review = inventory.review.find((item) => item.pageId === entry.pageId)
      if (
        !review ||
        (review.componentIds &&
          (!entry.componentIds ||
            entry.componentIds.some((id) => !review.componentIds!.includes(id))))
      )
        context.addIssue({
          code: 'custom',
          message: `Promotion scope exceeds Review scope: ${entry.pageId}`
        })
    }
  })
type GenerationInventory = z.infer<typeof generationInventorySchema>
type InventoryScope = 'review' | 'promotion'

function addInventoryPages(
  inventory: GenerationInventory,
  readyPageIds: string[],
  capturedComponents: Record<string, string[]> = {}
): GenerationInventory {
  // Readiness alone cannot expand publication scope; a completed ready capture must supply identities.
  const captured = new Set(readyPageIds.filter((id) => Object.hasOwn(capturedComponents, id)))
  const review = inventory.review.map((entry) =>
    captured.has(entry.pageId) ? { pageId: entry.pageId } : entry
  )
  const promotion = inventory.promotion.map((entry) =>
    captured.has(entry.pageId) ? { pageId: entry.pageId } : entry
  )
  for (const pageId of captured) {
    if (!review.some((entry) => entry.pageId === pageId)) review.push({ pageId })
    if (!promotion.some((entry) => entry.pageId === pageId)) promotion.push({ pageId })
  }
  return generationInventorySchema.parse({ ...inventory, review, promotion })
}

export { generationInventorySchema, addInventoryPages }
export type { GenerationInventory, InventoryScope }
