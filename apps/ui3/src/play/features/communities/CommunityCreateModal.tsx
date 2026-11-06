// Create-a-Community form, as unity-explorer's CommunityCreationEdition panel: a NAME gate, then
// profile picture, name*, membership, discoverability and description*, in that order. Submits
// through the bridge (signed multipart POST, the picture as a file part) and waits for the answer:
// success opens the new community's card, a refusal from the content check shows its findings, and
// any other failure is a toast with the form left as it was.

import { useEffect, useRef, useState } from 'react'
import { CharCounter, Select, Spinner, TextArea, openPopup, showConfirm, showToast } from '../../design'
import type { CommunitySave, CreateCommunityInput } from '../session/useEngineSession'
import { useSession } from '../session/SessionContext'
import { siteUrl } from '../../lib/baseDomain'
import styles from './CommunityCreateModal.module.css'

const MEMBERSHIP = [
  { value: 'public', label: 'Public', note: 'Anyone can become a member, view Community details, and join your Voice Streams' },
  { value: 'private', label: 'Private', note: 'Membership by request/invite; only Community description visible to non-members' }
] as const

const VISIBILITY = [
  { value: 'all', label: 'Discoverable', note: "Listed in 'Browse Communities' section and search results" },
  { value: 'unlisted', label: 'Unlisted', note: 'Hidden from non-members, invite-only' }
] as const

const NAMES_URL = siteUrl('/marketplace/names/claim')
const CONTENT_POLICY_URL = siteUrl('/content')
const CODE_OF_ETHICS_URL = siteUrl('/ethics')

// unity-explorer's limits (CreateOrEditCommunityPanel.prefab, CommunityCreationEditionController.cs),
// which are also the social service's.
const NAME_MAX = 30
const DESCRIPTION_MAX = 500
const MAX_IMAGE_BYTES = 512000
const MAX_IMAGE_PX = 512

const CREATE_ERROR = 'There was an error creating community. Please try again.'
const IMAGE_GENERAL_ERROR = 'Invalid image file selected. Please check file type and size.'
const IMAGE_WEIGHT_ERROR = 'Selected image exceeds 500KB size limit.'
const IMAGE_RESOLUTION_ERROR = 'Selected image exceeds 512x512 px size limit.'

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
 *  to the picture as a data URL, or to the message to show. */
export async function readCommunityPicture(file: File): Promise<{ dataUrl: string } | { error: string }> {
  if (file.type !== 'image/png' && file.type !== 'image/jpeg') return { error: IMAGE_GENERAL_ERROR }
  const dataUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(typeof reader.result === 'string' ? reader.result : '')
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(file)
  }).catch(() => null)
  if (dataUrl == null) return { error: IMAGE_GENERAL_ERROR }
  const size = await new Promise<{ w: number; h: number } | null>((resolve) => {
    const img = new Image()
    img.onload = () => resolve({ w: img.naturalWidth, h: img.naturalHeight })
    img.onerror = () => resolve(null)
    img.src = dataUrl
  })
  if (size == null) return { error: IMAGE_GENERAL_ERROR }
  const tooHeavy = file.size > MAX_IMAGE_BYTES
  const tooBig = size.w > MAX_IMAGE_PX || size.h > MAX_IMAGE_PX
  if (tooHeavy && tooBig) return { error: IMAGE_GENERAL_ERROR }
  if (tooHeavy) return { error: IMAGE_WEIGHT_ERROR }
  if (tooBig) return { error: IMAGE_RESOLUTION_ERROR }
  return { dataUrl }
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
  save,
  onCreate,
  onClearSave,
  onCreated,
  onClose,
  onDirtyChange
}: {
  /** The user has a claimed NAME (community creation is gated behind one). */
  canCreate: boolean
  /** The create in flight, or how it ended. */
  save: CommunitySave | null
  onCreate: (input: CreateCommunityInput) => void
  /** Forget the outcome: back to the form. */
  onClearSave: () => void
  /** The community was created: close the form without asking and open the community. */
  onCreated: (id: string) => void
  onClose: () => void
  /** Announce entered details, so a stray backdrop click can't bin them. */
  onDirtyChange?: (dirty: boolean) => void
}): React.JSX.Element {
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [privacy, setPrivacy] = useState<'public' | 'private'>('public')
  const [visibility, setVisibility] = useState<'all' | 'unlisted'>('all')
  const [pfp, setPfp] = useState<string | null>(null)
  const [focused, setFocused] = useState<'name' | 'description' | null>(null)
  const fileRef = useRef<HTMLInputElement>(null)
  const valid = name.trim() !== '' && description.trim() !== ''
  const saving = save?.status === 'saving'
  const dirty = name.trim() !== '' || description.trim() !== '' || pfp != null || privacy !== 'public' || visibility !== 'all'
  const dirtyCb = useRef(onDirtyChange)
  dirtyCb.current = onDirtyChange
  useEffect(() => {
    dirtyCb.current?.(dirty)
    return () => dirtyCb.current?.(false)
  }, [dirty])

  // How the create ended: success opens the community; a plain failure is a toast and the form
  // stays; a refusal from the content check stays in `save` and is shown below.
  const result = save?.status === 'done' ? save.result : null
  const created = useRef(onCreated)
  created.current = onCreated
  useEffect(() => {
    if (result == null || result.moderation != null) return
    onClearSave()
    if (result.error != null) showToast(CREATE_ERROR, { tone: 'error' })
    else created.current(result.id ?? '')
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
  const submit = (): void => {
    if (!valid || saving) return
    onCreate({
      name: name.trim(),
      description: description.trim(),
      privacy,
      discoverable: visibility === 'all',
      ...(pfp != null ? { thumbnail: pfp } : {})
    })
  }

  if (!canCreate) {
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
    return (
      <div className={styles.modal} role="dialog" aria-modal="true" aria-label="Your Community Couldn't Be Created" onClick={(e) => e.stopPropagation()}>
        <h2 className={styles.title}>Your Community Couldn&apos;t Be Created</h2>
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

  // Just the card — the popup layer (PopupHost .dim/.pop) draws the scrim, DPI-scales it and animates
  // it in. stopPropagation keeps a click on the card from reaching the scrim.
  return (
    <div className={styles.modal} role="dialog" aria-modal="true" aria-label="Create a Community" aria-busy={saving} onClick={(e) => e.stopPropagation()}>
      <h2 className={styles.title}>Create a Community</h2>

      <div className={styles.scroll}>
        <div className={styles.group}>
          <span className={styles.label}>PROFILE PICTURE</span>
          <span className={styles.hint}>PNG or JPG | 512x512 px | 500KB max</span>
          <div className={styles.pfp}>
            <span className={styles.pfpImg} style={pfp ? { backgroundImage: `url(${pfp})` } : undefined}>
              {!pfp && <ImageIcon />}
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
            disabled={saving}
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
            disabled={saving}
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
            disabled={saving}
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
      </div>

      {saving && (
        <div className={styles.checking} role="status">
          <Spinner size={20} />
          <span>Running Content Policy check. This may take a moment.</span>
        </div>
      )}

      <div className={styles.actions}>
        <button type="button" className={`${styles.ghost} ${styles.cancel}`} disabled={saving} onClick={onClose}>CANCEL</button>
        <button type="button" className={`${styles.primary} ${styles.create}`} disabled={!valid || saving} onClick={submit}>CREATE</button>
      </div>

      <p className={styles.policy}>
        Please ensure Community content follows Decentraland&apos;s{' '}
        <button type="button" className={styles.link} onClick={() => openLink(CONTENT_POLICY_URL)}>Content Policy.</button>
      </p>
    </div>
  )
}

/** The form with the session's create state: the popup content re-renders as the outcome arrives. */
function CommunityCreateContainer({
  canCreate,
  onCreated,
  onClose,
  onDirtyChange
}: {
  canCreate: boolean
  onCreated: (id: string) => void
  onClose: () => void
  onDirtyChange: (dirty: boolean) => void
}): React.JSX.Element {
  const { communities } = useSession()
  const { clearSave } = communities
  // a create's outcome belongs to this form only: forget it when the form goes away
  useEffect(() => clearSave, [clearSave])
  return (
    <CommunityCreateModal
      canCreate={canCreate}
      save={communities.save}
      onCreate={communities.create}
      onClearSave={clearSave}
      onCreated={onCreated}
      onClose={onClose}
      onDirtyChange={onDirtyChange}
    />
  )
}

/** Open the create-a-community flow (NAME gate or form) as a popup; returns the close handle.
 *  `canCreate` is a snapshot taken when the button is clicked — matches the other fire-once popups
 *  (WorldVisitModal): the profile's claimed-NAME state isn't expected to change mid-flow.
 *  `onCreated` gets the new community's id (empty if the service sent none) once the form closed. */
export function openCommunityCreateModal(canCreate: boolean, onCreated: (id: string) => void): () => void {
  // Same contract as the passport: the scrim refuses while there is something to lose, and the
  // deliberate closes (CANCEL, ×, the Cancel/Escape action) ask. A successful create closes through
  // the handle, which does not ask.
  const dirty = { current: false }
  const handle = openPopup(
    (close) => (
      <CommunityCreateContainer
        canCreate={canCreate}
        onCreated={(id) => {
          handle()
          onCreated(id)
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
          title: 'Discard this community?',
          body: 'The details you have entered will be lost.',
          confirmLabel: 'Discard',
          cancelLabel: 'Keep editing'
        })
    }
  )
  return handle
}
