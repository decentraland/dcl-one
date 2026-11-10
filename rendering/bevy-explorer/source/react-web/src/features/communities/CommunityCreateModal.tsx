// Create-a-Community and Edit Community form, as unity-explorer's CommunityCreationEdition panel: a
// NAME gate (create only), then profile picture, name*, membership, discoverability, description*
// and places, in that order. The form waits for the places (the user's own LANDs and Worlds, and
// the community's linked ones when editing) before it can be filled, as unity-explorer's does. Submits through the bridge (signed multipart POST or PUT, the
// picture as a file part) and waits for the answer: success opens the new community (create) or
// closes the form (edit), a refusal from the content check shows its findings, and any other
// failure is a toast with the form left as it was. An edit sends only what changed; only the owner
// can change the name, membership and discoverability.

import { useEffect, useRef, useState } from 'react'
import { CharCounter, MaskIcon, Select, Spinner, TextArea, openPopup, showConfirm, showToast } from '@hud/design'
import type { Community, CommunityPlaceOption, CommunityPlaceOptionsMessage } from '@hud/engine/protocol'
import closeIcon from '@hud/assets/backpack/icon-close.webp'
import type { CommunitySave, CreateCommunityInput, UpdateCommunityInput } from '../session/useEngineSession'
import { useSession } from '../session/SessionContext'
import styles from './CommunityCreateModal.module.css'

const MEMBERSHIP = [
  { value: 'public', label: 'Public', note: 'Anyone can become a member, view Community details, and join your Voice Streams' },
  { value: 'private', label: 'Private', note: 'Membership by request/invite; only Community description visible to non-members' }
] as const

const VISIBILITY = [
  { value: 'all', label: 'Discoverable', note: "Listed in 'Browse Communities' section and search results" },
  { value: 'unlisted', label: 'Unlisted', note: 'Hidden from non-members, invite-only' }
] as const

const NAMES_URL = 'https://decentraland.org/marketplace/names/claim'
const CONTENT_POLICY_URL = 'https://decentraland.org/content'
const CODE_OF_ETHICS_URL = 'https://decentraland.org/ethics'

// unity-explorer's limits (CreateOrEditCommunityPanel.prefab, CommunityCreationEditionController.cs),
// which are also the social service's.
const NAME_MAX = 30
const DESCRIPTION_MAX = 500
const MAX_IMAGE_BYTES = 512000
const MAX_IMAGE_PX = 512

const CREATE_ERROR = 'There was an error creating community. Please try again.'
const UPDATE_ERROR = 'There was an error updating community. Please try again.'
const IMAGE_GENERAL_ERROR = 'Invalid image file selected. Please check file type and size.'
const IMAGE_WEIGHT_ERROR = 'Selected image exceeds 500KB size limit.'
const IMAGE_RESOLUTION_ERROR = 'Selected image exceeds 512x512 px size limit.'
const PLACES_DROPDOWN_TITLE = 'Select LAND or World'

/** The ids to send, as unity-explorer's form collects them: lands first, then worlds. */
function placeIdsOf(places: CommunityPlaceOption[]): string[] {
  return [...places.filter((p) => !p.isWorld), ...places.filter((p) => p.isWorld)].map((p) => p.id)
}
const sameIds = (a: string[], b: string[]): boolean => a.length === b.length && a.every((id) => b.includes(id))

export type { CreateCommunityInput }

function LockArt(): React.JSX.Element {
  return (
    <svg viewBox="0 0 48 48" width="48" height="48" aria-hidden="true">
      <path d="M14 20v-4a10 10 0 0 1 20 0v4M10 20h28v18H10z" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}
function ImageIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="34" height="34" aria-hidden="true">
      <rect x="3" y="4" width="18" height="16" rx="2" fill="none" stroke="currentColor" strokeWidth="1.6" />
      <circle cx="8.5" cy="9.5" r="1.8" fill="currentColor" />
      <path d="M5 18l4.5-5 3.5 4 2.5-2.5L21 17" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}
function PencilIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
      <path d="M14.06 6.19l3.75 3.75M3 17.25V21h3.75L17.81 9.94a1.5 1.5 0 0 0 0-2.12l-1.63-1.63a1.5 1.5 0 0 0-2.12 0L3 17.25z" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}

const openLink = (url: string): void => {
  window.open(url, '_blank', 'noopener')
}

/** unity-explorer's checks on a chosen picture: PNG or JPEG, at most 500 KB and 512×512 px. Resolves
 *  to the picture as a data URL, or to the message to show. The dimensions are read from the file
 *  itself, and only a picture that passes is read into a data URL, so a large photo is refused
 *  without being copied as text first. */
export async function readCommunityPicture(file: File): Promise<{ dataUrl: string } | { error: string }> {
  if (file.type !== 'image/png' && file.type !== 'image/jpeg') return { error: IMAGE_GENERAL_ERROR }
  const url = URL.createObjectURL(file)
  const size = await new Promise<{ w: number; h: number } | null>((resolve) => {
    const img = new Image()
    img.onload = () => resolve({ w: img.naturalWidth, h: img.naturalHeight })
    img.onerror = () => resolve(null)
    img.src = url
  }).finally(() => URL.revokeObjectURL(url))
  if (size == null) return { error: IMAGE_GENERAL_ERROR }
  const tooHeavy = file.size > MAX_IMAGE_BYTES
  const tooBig = size.w > MAX_IMAGE_PX || size.h > MAX_IMAGE_PX
  if (tooHeavy && tooBig) return { error: IMAGE_GENERAL_ERROR }
  if (tooHeavy) return { error: IMAGE_WEIGHT_ERROR }
  if (tooBig) return { error: IMAGE_RESOLUTION_ERROR }
  const dataUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(typeof reader.result === 'string' ? reader.result : '')
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(file)
  }).catch(() => null)
  return dataUrl == null ? { error: IMAGE_GENERAL_ERROR } : { dataUrl }
}

/** unity-explorer's text for the content check's findings: one line per field that has any. */
function issueLines(issues: { name: string[]; description: string[]; image: string[] }): string[] {
  const line = (label: string, tags: string[]): string | null => (tags.length > 0 ? `${label}: ${tags.map((t) => `[${t}]`).join(', ')}` : null)
  return [line('COMMUNITY NAME', issues.name), line('COMMUNITY IMAGE', issues.image), line('COMMUNITY DESCRIPTION', issues.description)].filter(
    (l): l is string => l != null
  )
}

export function CommunityCreateModal({
  canCreate,
  editing,
  save,
  placeOptions,
  onCreate,
  onUpdate,
  onClearSave,
  onSaved,
  onClose,
  onDirtyChange
}: {
  /** The user has a claimed NAME (community creation is gated behind one). Ignored when editing. */
  canCreate: boolean
  /** The community being edited; absent to create one. */
  editing?: Community
  /** The create or update in flight, or how it ended. */
  save: CommunitySave | null
  /** The places for the picker and, when editing, the linked ones; null while they load. */
  placeOptions: CommunityPlaceOptionsMessage | null
  onCreate: (input: CreateCommunityInput) => void
  onUpdate: (input: UpdateCommunityInput) => void
  /** Forget the outcome: back to the form. */
  onClearSave: () => void
  /** Saved (or nothing to save): close the form without asking. Gets the community's id. */
  onSaved: (id: string) => void
  onClose: () => void
  /** Announce entered details, so a stray backdrop click can't bin them. */
  onDirtyChange?: (dirty: boolean) => void
}): React.JSX.Element {
  // the values the form opens with: the community's when editing, unity-explorer's defaults otherwise
  const initial = useRef({
    name: editing?.name ?? '',
    description: editing?.description ?? '',
    privacy: (editing?.privacy === 'private' ? 'private' : 'public') as 'public' | 'private',
    visibility: (editing?.visibility === 'unlisted' ? 'unlisted' : 'all') as 'all' | 'unlisted'
  }).current
  const [name, setName] = useState(initial.name)
  const [description, setDescription] = useState(initial.description)
  const [privacy, setPrivacy] = useState(initial.privacy)
  const [visibility, setVisibility] = useState(initial.visibility)
  // a newly chosen picture (data URL); the current one shows until then
  const [pfp, setPfp] = useState<string | null>(null)
  const [focused, setFocused] = useState<'name' | 'description' | null>(null)
  // the linked places (tags): the community's once its places arrive, none on create
  const [places, setPlaces] = useState<CommunityPlaceOption[]>([])
  const initialPlaceIds = useRef<string[]>([])
  const loadingPlaces = placeOptions == null
  const owned = placeOptions?.owned ?? []
  const linkedFailed = placeOptions?.linkedFailed === true
  useEffect(() => {
    if (placeOptions == null) return
    // unity-explorer's notice for each read that failed
    placeOptions.errors.forEach((e) => showToast(e, { tone: 'error' }))
    setPlaces(placeOptions.linked)
    initialPlaceIds.current = placeOptions.linked.map((p) => p.id)
  }, [placeOptions])
  const fileRef = useRef<HTMLInputElement>(null)
  const isEdit = editing != null
  // as unity-explorer: on edit only the owner may change the name, membership and discoverability
  const ownerFields = !isEdit || editing.role === 'owner'
  const shownPfp = pfp ?? editing?.thumbnail ?? null
  const valid = name.trim() !== '' && description.trim() !== ''
  const saving = save?.status === 'saving'
  const dirty =
    name.trim() !== initial.name.trim() ||
    description.trim() !== initial.description.trim() ||
    pfp != null ||
    privacy !== initial.privacy ||
    visibility !== initial.visibility ||
    !sameIds(places.map((p) => p.id), initialPlaceIds.current)
  const dirtyCb = useRef(onDirtyChange)
  dirtyCb.current = onDirtyChange
  useEffect(() => {
    dirtyCb.current?.(dirty)
    return () => dirtyCb.current?.(false)
  }, [dirty])

  // How the save ended: success closes the form; a plain failure is a toast and the form stays; a
  // refusal from the content check stays in `save` and is shown below.
  const result = save?.status === 'done' ? save.result : null
  const saved = useRef(onSaved)
  saved.current = onSaved
  useEffect(() => {
    if (result == null || result.moderation != null) return
    onClearSave()
    if (result.error != null) showToast(result.mode === 'update' ? UPDATE_ERROR : CREATE_ERROR, { tone: 'error' })
    else saved.current(result.id ?? '')
  }, [result, onClearSave])

  const pickPfp = (e: React.ChangeEvent<HTMLInputElement>): void => {
    const file = e.target.files?.[0]
    e.target.value = ''
    if (!file) return
    void readCommunityPicture(file).then((picked) => {
      if ('error' in picked) showToast(picked.error, { tone: 'error' })
      else setPfp(picked.dataUrl)
    })
  }
  // as unity-explorer: on create every tag can go; on edit the owner may unlink any place, a
  // moderator only the places they own (which includes every place they just added)
  const removable = (p: CommunityPlaceOption): boolean => !isEdit || editing.role === 'owner' || owned.some((o) => o.id === p.id)
  const addPlace = (id: string): void => {
    const place = owned.find((o) => o.id === id)
    if (place != null && !places.some((p) => p.id === id)) setPlaces([...places, place])
  }
  const removePlace = (id: string): void => setPlaces(places.filter((p) => p.id !== id))

  const submit = (): void => {
    if (!valid || saving || loadingPlaces) return
    const placeIds = placeIdsOf(places)
    // what is sent: the places shown, and the linked ones the places service did not return
    const hidden = (placeOptions?.hiddenLinkedIds ?? []).filter((id) => !placeIds.includes(id))
    const sentIds = [...placeIds, ...hidden]
    if (isEdit) {
      // only what changed, as unity-explorer; with nothing changed the form just closes
      const changes: UpdateCommunityInput = { id: editing.id }
      if (name.trim() !== initial.name.trim()) changes.name = name.trim()
      if (description.trim() !== initial.description.trim()) changes.description = description.trim()
      if (privacy !== initial.privacy) changes.privacy = privacy
      if (visibility !== initial.visibility) changes.discoverable = visibility === 'all'
      if (pfp != null) changes.thumbnail = pfp
      // the whole set, only when it changed (unity-explorer sends it on every save)
      if (!linkedFailed && !sameIds(placeIds, initialPlaceIds.current)) changes.placeIds = sentIds
      if (Object.keys(changes).length === 1) saved.current(editing.id)
      else onUpdate(changes)
      return
    }
    onCreate({
      name: name.trim(),
      description: description.trim(),
      privacy,
      discoverable: visibility === 'all',
      ...(pfp != null ? { thumbnail: pfp } : {}),
      placeIds: sentIds
    })
  }

  if (!isEdit && !canCreate) {
    return (
      <div className={styles.gate} onClick={(e) => e.stopPropagation()} role="dialog" aria-label="Get a NAME">
        <div className={styles.gateArt}><LockArt /></div>
        <h2 className={styles.gateTitle}>Get a NAME to Unlock Community Creation</h2>
        <p className={styles.gateBody}>
          NAMEs are unique Decentraland usernames that come with a World, and unlock community creation.
        </p>
        <div className={styles.gateBtns}>
          <button type="button" className={styles.primary} onClick={() => openLink(NAMES_URL)}>Get a NAME</button>
          <button type="button" className={styles.ghost} onClick={onClose}>Maybe later</button>
        </div>
      </div>
    )
  }

  const moderation = result?.moderation
  if (moderation != null) {
    // unity-explorer shows "Created" for both; an edit says "Updated" here (decisions guide)
    const refused = isEdit ? "Your Community Couldn't Be Updated" : "Your Community Couldn't Be Created"
    return (
      <div className={styles.modal} role="dialog" aria-modal="true" aria-label={refused} onClick={(e) => e.stopPropagation()}>
        <h2 className={styles.title}>{refused}</h2>
        {moderation.unavailable ? (
          <p className={styles.notice}>Content verification is temporarily unavailable. Please try again later.</p>
        ) : (
          <>
            <p className={styles.notice}>
              The following content has been flagged as violating{' '}
              <button type="button" className={styles.link} onClick={() => openLink(CODE_OF_ETHICS_URL)}>Decentraland&apos;s Guidelines</button>:
            </p>
            <ul className={styles.issues}>
              {issueLines(moderation.issues).map((l) => (
                <li key={l}>{l}</li>
              ))}
            </ul>
            <p className={styles.notice}>Please remove any rule-breaking content to continue. If you believe this was a mistake, please reach out to Support.</p>
          </>
        )}
        <div className={styles.actions}>
          {moderation.unavailable ? (
            <>
              <button type="button" className={`${styles.ghost} ${styles.cancel}`} onClick={onClearSave}>CANCEL</button>
              <button type="button" className={`${styles.primary} ${styles.create}`} onClick={submit}>RETRY</button>
            </>
          ) : (
            <>
              <button type="button" className={`${styles.ghost} ${styles.cancel}`} onClick={onClose}>CANCEL</button>
              <button type="button" className={`${styles.primary} ${styles.create}`} onClick={onClearSave}>EDIT COMMUNITY</button>
            </>
          )}
        </div>
      </div>
    )
  }

  const title = isEdit ? 'Edit Community' : 'Create a Community'
  // Just the card — the popup layer (PopupHost .dim/.pop) draws the scrim, DPI-scales it and animates
  // it in. stopPropagation keeps a click on the card from reaching the scrim.
  return (
    <div className={styles.modal} role="dialog" aria-modal="true" aria-label={title} aria-busy={saving} onClick={(e) => e.stopPropagation()}>
      <h2 className={styles.title}>{title}</h2>

      {loadingPlaces ? (
        <div className={styles.loading}>
          <Spinner size={28} />
        </div>
      ) : (
      <div className={styles.scroll}>
        <div className={styles.group}>
          <span className={styles.label}>PROFILE PICTURE</span>
          <span className={styles.hint}>PNG or JPG | 512x512 px | 500KB max</span>
          <div className={styles.pfp}>
            <span className={styles.pfpImg} style={shownPfp ? { backgroundImage: `url(${shownPfp})` } : undefined}>
              {!shownPfp && <ImageIcon />}
            </span>
            <button type="button" className={styles.pfpEdit} aria-label="Edit profile picture" disabled={saving} onClick={() => fileRef.current?.click()}>
              <PencilIcon />
            </button>
            <input ref={fileRef} type="file" accept="image/png,image/jpeg" hidden onChange={pickPfp} />
          </div>
        </div>

        <div className={styles.group}>
          <div className={styles.labelRow}>
            <label className={styles.label} htmlFor="cc-name">COMMUNITY NAME <span className={styles.req}>*</span></label>
            {focused === 'name' && <CharCounter current={name.length} max={NAME_MAX} />}
          </div>
          <input
            id="cc-name"
            className={styles.input}
            maxLength={NAME_MAX}
            placeholder="Write here"
            value={name}
            disabled={saving || !ownerFields}
            onChange={(e) => setName(e.target.value)}
            onFocus={() => setFocused('name')}
            onBlur={() => setFocused(null)}
            // eslint-disable-next-line jsx-a11y/no-autofocus
            autoFocus
          />
        </div>

        <div className={styles.group}>
          <span className={styles.label}>MEMBERSHIP</span>
          {/* Our own listbox, not a native <select>: the HUD renders offscreen and CEF paints a
              native dropdown as a popup surface the engine does not composite, which covers the
              whole HUD (see DateField). The note rides along in the option's label. */}
          <Select
            aria-label="Membership"
            variant="light"
            value={privacy}
            disabled={saving || !ownerFields}
            options={MEMBERSHIP.map((o) => ({ value: o.value, label: `${o.label}: ${o.note}` }))}
            onChange={(value) => setPrivacy(value as 'public' | 'private')}
          />
        </div>

        <div className={styles.group}>
          <span className={styles.label}>DISCOVERABILITY</span>
          <Select
            aria-label="Discoverability"
            variant="light"
            value={visibility}
            disabled={saving || !ownerFields}
            options={VISIBILITY.map((o) => ({ value: o.value, label: `${o.label}: ${o.note}` }))}
            onChange={(value) => setVisibility(value as 'all' | 'unlisted')}
          />
        </div>

        <div className={styles.group}>
          <div className={styles.labelRow}>
            <label className={styles.label} htmlFor="cc-description">DESCRIPTION <span className={styles.req}>*</span></label>
            {focused === 'description' && <CharCounter current={description.length} max={DESCRIPTION_MAX} />}
          </div>
          <span className={styles.hint}>
            Tell everyone what your Community is about! The first 80 characters of your description will be visible when browsing Communities.
          </span>
          <TextArea
            id="cc-description"
            variant="light"
            maxLength={DESCRIPTION_MAX}
            placeholder="Write here"
            value={description}
            disabled={saving}
            onChange={setDescription}
            onFocus={() => setFocused('description')}
            onBlur={() => setFocused(null)}
          />
        </div>

        <div className={styles.group}>
          <span className={styles.label}>PLACES</span>
          <span className={styles.hint}>
            Community owners and moderators may link Places they own to a Community. <b>Note:</b> Public photos taken at linked Places will
            appear in your Community&apos;s profile.
          </span>
          <Select
            aria-label="Add a place"
            variant="light"
            value=""
            placeholder={PLACES_DROPDOWN_TITLE}
            disabled={saving || linkedFailed}
            options={owned.filter((o) => !places.some((p) => p.id === o.id)).map((o) => ({ value: o.id, label: o.label }))}
            onChange={addPlace}
          />
          {places.length > 0 && (
            <ul className={styles.placeTags} aria-label="Linked places">
              {places.map((p) => (
                <li key={p.id} className={styles.placeTag}>
                  <span className={styles.placeText}>
                    <span className={styles.placeName}>{p.label}</span>
                    {p.ownerName !== '' && <span className={styles.placeOwner}>{p.ownerName}</span>}
                  </span>
                  {removable(p) && (
                    <button
                      type="button"
                      className={styles.placeRemove}
                      aria-label={`Remove ${p.label}`}
                      disabled={saving || linkedFailed}
                      onClick={() => removePlace(p.id)}
                    >
                      <MaskIcon src={closeIcon} size={10} />
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
      )}

      {saving && (
        <div className={styles.checking} role="status">
          <Spinner size={20} />
          <span>Running Content Policy check. This may take a moment.</span>
        </div>
      )}

      <div className={styles.actions}>
        <button type="button" className={`${styles.ghost} ${styles.cancel}`} disabled={saving} onClick={onClose}>CANCEL</button>
        <button type="button" className={`${styles.primary} ${styles.create}`} disabled={!valid || saving || loadingPlaces} onClick={submit}>{isEdit ? 'SAVE' : 'CREATE'}</button>
      </div>

      <p className={styles.policy}>
        Please ensure Community content follows Decentraland&apos;s{' '}
        <button type="button" className={styles.link} onClick={() => openLink(CONTENT_POLICY_URL)}>Content Policy.</button>
      </p>
    </div>
  )
}

/** The form with the session's save state: the popup content re-renders as the outcome arrives. */
function CommunityFormContainer({
  canCreate,
  editing,
  onSaved,
  onClose,
  onDirtyChange
}: {
  canCreate: boolean
  editing?: Community
  onSaved: (id: string) => void
  onClose: () => void
  onDirtyChange: (dirty: boolean) => void
}): React.JSX.Element {
  const { communities } = useSession()
  const { clearSave, loadPlaceOptions, clearPlaceOptions } = communities
  // a save's outcome belongs to this form only: forget it when the form goes away
  useEffect(() => clearSave, [clearSave])
  // the places, read once as the form opens and forgotten when it closes; not behind the NAME gate,
  // which shows no places (a failed read would toast over it)
  const editingId = editing?.id
  const gated = editingId == null && !canCreate
  useEffect(() => {
    if (gated) return
    loadPlaceOptions(editingId)
    return clearPlaceOptions
  }, [gated, editingId, loadPlaceOptions, clearPlaceOptions])
  return (
    <CommunityCreateModal
      canCreate={canCreate}
      editing={editing}
      save={communities.save}
      placeOptions={communities.placeOptions}
      onCreate={communities.create}
      onUpdate={communities.update}
      onClearSave={clearSave}
      onSaved={onSaved}
      onClose={onClose}
      onDirtyChange={onDirtyChange}
    />
  )
}

/** Open the form as a popup; returns the close handle. Same contract as the passport: the scrim
 *  refuses while there is something to lose, and the deliberate closes (CANCEL, the Cancel/Escape
 *  action) ask. A successful save closes through the handle, which does not ask. */
function openCommunityForm(canCreate: boolean, editing: Community | undefined, onSaved: (id: string) => void): () => void {
  const dirty = { current: false }
  const handle = openPopup(
    (close) => (
      <CommunityFormContainer
        canCreate={canCreate}
        editing={editing}
        onSaved={(id) => {
          handle()
          onSaved(id)
        }}
        onClose={close}
        onDirtyChange={(d) => {
          dirty.current = d
        }}
      />
    ),
    {
      backdropClickCloses: () => !dirty.current,
      confirmClose: () =>
        !dirty.current ||
        showConfirm({
          title: editing != null ? 'Discard your changes?' : 'Discard this community?',
          body: editing != null ? 'The changes you have made will be lost.' : 'The details you have entered will be lost.',
          confirmLabel: 'Discard',
          cancelLabel: 'Keep editing'
        })
    }
  )
  return handle
}

/** Open the create-a-community flow (NAME gate or form) as a popup; returns the close handle.
 *  `canCreate` is a snapshot taken when the button is clicked — matches the other fire-once popups
 *  (WorldVisitModal): the profile's claimed-NAME state isn't expected to change mid-flow.
 *  `onCreated` gets the new community's id (empty if the service sent none) once the form closed. */
export function openCommunityCreateModal(canCreate: boolean, onCreated: (id: string) => void): () => void {
  return openCommunityForm(canCreate, undefined, onCreated)
}

/** Open the Edit Community form for a community the user owns or moderates; returns the close handle. */
export function openCommunityEditModal(community: Community): () => void {
  return openCommunityForm(true, community, () => undefined)
}
