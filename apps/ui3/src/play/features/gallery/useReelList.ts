// A camera reel read page by page, as unity-explorer's PagedCameraReelManager and
// CameraReelGalleryController do: 100 photos a page, the next page asked for when fewer than 12
// loaded photos remain past the last one on screen, and paging over once the reel's total is loaded
// or a page comes back short. The list keeps the service's order (newest first).
//
// `key` names the reel: a new key (opening the gallery, another passport) starts again from the top.
// Unity's offset advances by the page size even after a delete, which skips a photo; here the next
// page starts at the number of photos held.

import { useCallback, useEffect, useRef, useState } from 'react'
import type { GalleryPhoto } from '../../engine/protocol'
import type { ReelPage } from '../session/useEngineSession'

export const REEL_PAGE_SIZE = 100
export const REEL_LOAD_MORE_THRESHOLD = 12

export interface ReelList {
  photos: GalleryPhoto[]
  /** Photos in the reel in all, and the owner's storage limit, as the last page reported them. */
  total: number
  max: number
  /** The first page arrived (spinner until then). */
  loaded: boolean
  /** Ask for the next page if `lastVisibleIndex` is within the threshold of the loaded end. */
  loadMore: (lastVisibleIndex: number) => void
  /** Drop a photo (deleted, or made private on a public reel). */
  remove: (id: string) => void
  setPublic: (id: string, isPublic: boolean) => void
  /** Storage usage after a write that reports it. */
  setStorage: (total: number, max: number) => void
}

export function useReelList(key: string | null, fetchPage: (offset: number, limit: number) => Promise<ReelPage>): ReelList {
  const [photos, setPhotos] = useState<GalleryPhoto[]>([])
  const [storage, setStorageState] = useState({ total: 0, max: 0 })
  const [loaded, setLoaded] = useState(false)
  // Read synchronously by loadMore, which the grid calls on every scroll.
  const state = useRef({ generation: 0, loading: false, done: false, count: 0 })
  const fetchRef = useRef(fetchPage)
  fetchRef.current = fetchPage

  const load = useCallback(() => {
    const s = state.current
    if (s.loading || s.done) return
    s.loading = true
    const generation = s.generation
    void fetchRef.current(s.count, REEL_PAGE_SIZE).then((page) => {
      if (generation !== state.current.generation) return
      s.loading = false
      if (page.ok) {
        s.count += page.photos.length
        s.done = s.count >= page.total || page.photos.length < REEL_PAGE_SIZE
        setPhotos((list) => [...list, ...page.photos])
        setStorageState({ total: page.total, max: page.max })
      } else if (s.count === 0) {
        // The first page failed: show what there is (nothing) rather than spin forever.
        s.done = true
      }
      setLoaded(true)
    })
  }, [])

  useEffect(() => {
    const s = state.current
    s.generation += 1
    s.loading = false
    s.done = false
    s.count = 0
    setPhotos([])
    setStorageState({ total: 0, max: 0 })
    setLoaded(false)
    if (key != null) load()
  }, [key, load])

  const loadMore = useCallback(
    (lastVisibleIndex: number) => {
      if (state.current.count - lastVisibleIndex >= REEL_LOAD_MORE_THRESHOLD) return
      load()
    },
    [load]
  )

  const photosRef = useRef(photos)
  photosRef.current = photos
  const remove = useCallback((id: string) => {
    if (!photosRef.current.some((p) => p.id === id)) return
    state.current.count -= 1
    setPhotos((list) => list.filter((p) => p.id !== id))
    setStorageState((st) => ({ ...st, total: Math.max(0, st.total - 1) }))
  }, [])

  const setPublic = useCallback((id: string, isPublic: boolean) => {
    setPhotos((list) => list.map((p) => (p.id === id ? { ...p, isPublic } : p)))
  }, [])

  const setStorage = useCallback((total: number, max: number) => setStorageState({ total, max }), [])

  return { photos, total: storage.total, max: storage.max, loaded, loadMore, remove, setPublic, setStorage }
}
