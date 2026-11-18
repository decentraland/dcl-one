import { useCallback, useEffect, useMemo, useReducer, useState } from 'react'
import { useSpringBones } from '~/hooks/useSpringBones'
import { createItemDraft, isItemDraftDirty, itemDraftReducer, type ItemDraft } from '~/lib/itemDraft'
import { type Item } from '~/lib/items'
import { sameSpringBoneParams, type SpringBoneParamsByHash } from '~/lib/springBones'

const EMPTY_DRAFT: ItemDraft = {
  name: '',
  description: '',
  utility: '',
  category: null,
  rarity: null,
  hides: [],
  tags: [],
  loop: false,
  blockVrmExport: false,
  outlineCompatible: true,
  thumbnail: null,
  video: null,
  mappings: null
}

/** The properties form state for the selected item: field draft plus spring bone params, reset on selection. */
export function useItemForm(item: Item | null) {
  const [draft, dispatch] = useReducer(itemDraftReducer, item, initial =>
    initial ? createItemDraft(initial) : EMPTY_DRAFT
  )
  const [springBoneParams, setSpringBoneParams] = useState<SpringBoneParamsByHash>({})
  const springBones = useSpringBones(item)
  const itemId = item?.id ?? null

  useEffect(() => {
    if (item) dispatch({ type: 'reset', item })
    // Only a different item resets the draft; a refetch of the same item keeps the edits.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [itemId])

  useEffect(() => {
    setSpringBoneParams(springBones.initialParams)
  }, [springBones.initialParams])

  const isDraftDirty = useMemo(() => (item ? isItemDraftDirty(draft, item) : false), [draft, item])
  const isSpringBonesDirty = useMemo(
    () => !sameSpringBoneParams(springBoneParams, springBones.initialParams),
    [springBoneParams, springBones.initialParams]
  )

  const reset = useCallback(
    (from: Item) => {
      dispatch({ type: 'reset', item: from })
      setSpringBoneParams(springBones.initialParams)
    },
    [springBones.initialParams]
  )

  return {
    draft,
    springBones,
    dispatch,
    springBoneParams,
    setSpringBoneParams,
    isDraftDirty,
    isSpringBonesDirty,
    isDirty: isDraftDirty || isSpringBonesDirty,
    reset
  }
}
