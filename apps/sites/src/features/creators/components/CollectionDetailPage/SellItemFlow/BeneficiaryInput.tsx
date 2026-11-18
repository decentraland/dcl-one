import { shortenAddress } from '@/data/lib/creators/address'
import {
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
  type CSSProperties,
  type KeyboardEvent
} from 'react'
import { createPortal } from 'react-dom'
import { CloseIcon, ExpandMoreIcon as ChevronIcon } from '../../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { useProfile } from '../../../hooks/useProfile'
import type { Friend } from '@/data/lib/creators/friends'
import { isValidAddress } from '@/data/lib/creators/sales'
import { holdsAddresses } from '@/data/lib/creators/mint'
import styles from './SellItemModal.module.css'
import shared from '../../../styles/shared.module.css'

const LIST_GAP = 6
const VIEWPORT_MARGIN = 8

type Props = {
  /** The chosen address (lowercased), or '' while none is chosen. */
  value: string
  onChange: (address: string) => void
  /** The creator's friends; `undefined` while they load or when they couldn't be loaded. */
  friends: Friend[] | undefined
  isLoadingFriends: boolean
  disabled?: boolean
  placeholder?: string
  /** Rejects an address already used, with this copy under the field. */
  duplicateError?: (address: string) => string | null
  /**
   * Takes a pasted list of addresses (the text holds addresses but is not one): the field clears when it
   * returns null, or keeps the text and shows the returned error under it.
   */
  onChangeMany?: (text: string) => string | null
  /** The signed-in wallet (lowercased): listed first as "You". */
  self?: string
  /** Addresses already chosen (lowercased): left out of the list; one typed in is dropped and `onExcluded` told. */
  exclude?: string[]
  onExcluded?: () => void
  /** Accessible name of the chip's ✕; "Clear beneficiary" unless the chip stands for something else. */
  clearLabel?: string
  /** `compact`: the chosen address as a dark pill with a shortened address (address lists), not a form field. */
  variant?: 'field' | 'compact'
  testId?: string
}

/**
 * Pick a payout address: a friend from the list, or any wallet address pasted in. A valid address is
 * taken as soon as it is typed; the chosen one shows as a chip with its avatar and name, if known.
 */
export function BeneficiaryInput({
  value,
  onChange,
  friends,
  isLoadingFriends,
  disabled,
  placeholder,
  duplicateError,
  onChangeMany,
  self,
  exclude,
  onExcluded,
  clearLabel,
  variant = 'field',
  testId = 'beneficiary'
}: Props) {
  const { t } = useTranslation()
  const listId = useId()
  const [query, setQuery] = useState('')
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const [touched, setTouched] = useState(false)
  const [listError, setListError] = useState<string | null>(null)
  const boxRef = useRef<HTMLDivElement>(null)
  const listRef = useRef<HTMLUListElement>(null)
  const [listStyle, setListStyle] = useState<CSSProperties>()

  const selfProfile = useProfile(self)
  const youLabel = t('sell_item_modal.beneficiary.you')
  const entries = useMemo<Friend[]>(() => {
    const list = friends ?? []
    if (!self) return list
    const me: Friend = {
      address: self,
      // No name: the "You" tag and the address identify the row, so the address isn't repeated as a name.
      name: selfProfile.data?.name ?? '',
      hasClaimedName: selfProfile.data?.hasClaimedName ?? false,
      avatarUrl: selfProfile.data?.avatar?.snapshots?.face256 ?? ''
    }
    return [me, ...list.filter(friend => friend.address !== self)]
  }, [friends, self, selfProfile.data])

  const selectedFriend = useMemo(() => entries.find(friend => friend.address === value), [entries, value])
  // A pasted address that isn't a friend still gets a name and avatar from its profile, when it has one.
  const profile = useProfile(value && !selectedFriend ? value : undefined)

  const available = useMemo(
    () => (exclude?.length ? entries.filter(friend => !exclude.includes(friend.address)) : entries),
    [entries, exclude]
  )
  const matches = useMemo(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return available
    return available.filter(
      friend =>
        friend.name.toLowerCase().includes(needle) ||
        friend.address.includes(needle) ||
        (friend.address === self && youLabel.toLowerCase().includes(needle))
    )
  }, [available, query, self, youLabel])

  const trimmed = query.trim()
  const duplicate = isValidAddress(trimmed) ? duplicateError?.(trimmed.toLowerCase()) : null
  const invalid = touched && trimmed !== '' && !isValidAddress(trimmed)
  // Without anyone left to pick from the field is a plain address input.
  const withFriends = isLoadingFriends || available.length > 0
  const showList = withFriends && open

  // The list is portaled so the modal's scrolling body can't clip it: anchor it to the field, upward when there's no room below.
  useLayoutEffect(() => {
    if (!showList) return
    function position() {
      const box = boxRef.current?.getBoundingClientRect()
      if (!box) return
      const height = listRef.current?.offsetHeight ?? 0
      const fitsBelow = box.bottom + LIST_GAP + height <= window.innerHeight - VIEWPORT_MARGIN
      setListStyle({
        left: box.left,
        width: box.width,
        top: fitsBelow ? box.bottom + LIST_GAP : Math.max(VIEWPORT_MARGIN, box.top - LIST_GAP - height)
      })
    }
    position()
    window.addEventListener('resize', position)
    window.addEventListener('scroll', position, true)
    return () => {
      window.removeEventListener('resize', position)
      window.removeEventListener('scroll', position, true)
    }
  }, [showList, matches.length, isLoadingFriends])

  function reset() {
    setQuery('')
    setOpen(false)
    setTouched(false)
    setListError(null)
  }

  function commit(address: string) {
    const normalized = address.toLowerCase()
    if (exclude?.includes(normalized)) {
      onExcluded?.()
      reset()
      return
    }
    if (duplicateError?.(normalized)) return
    onChange(normalized)
    reset()
  }

  function handleInput(next: string) {
    setQuery(next)
    setActive(0)
    setOpen(true)
    setListError(null)
    if (isValidAddress(next.trim())) commit(next.trim())
  }

  // Read from the clipboard: the input itself would drop the line breaks between the addresses.
  function handlePaste(event: ClipboardEvent<HTMLInputElement>) {
    if (!onChangeMany) return
    const text = event.clipboardData.getData('text').trim()
    if (isValidAddress(text) || !holdsAddresses(text)) return
    event.preventDefault()
    const error = onChangeMany(text)
    if (error === null) {
      reset()
      return
    }
    setQuery(text.replace(/\s+/g, ' '))
    setOpen(false)
    setListError(error)
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === 'Escape') {
      // Keep the dialog open; only the list closes.
      event.stopPropagation()
      setOpen(false)
      return
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      setOpen(true)
      if (matches.length === 0) return
      const delta = event.key === 'ArrowDown' ? 1 : -1
      setActive(current => (current + delta + matches.length) % matches.length)
      return
    }
    if (event.key === 'Enter') {
      event.preventDefault()
      const match = open ? matches[active] : undefined
      if (match) commit(match.address)
      else setTouched(true)
    }
  }

  if (value) {
    const known = selectedFriend?.name || profile.data?.name
    const avatar = selectedFriend?.avatarUrl ?? profile.data?.avatar?.snapshots?.face256
    const compact = variant === 'compact'
    return (
      <div
        className={styles.box}
        data-testid={`${testId}-selected`}
        data-variant={variant}
        data-disabled={disabled || undefined}
      >
        {avatar ? (
          <img className={styles.avatar} src={avatar} alt="" />
        ) : (
          <span className={styles.avatarFallback} aria-hidden />
        )}
        <span className={styles.chipName} data-testid={`${testId}-name`}>
          {known ?? shortenAddress(value)}
        </span>
        {value === self && (
          <span className={styles.youTag} data-testid={`${testId}-you`}>
            {youLabel}
          </span>
        )}
        {/* Without a name the compact pill would repeat the shortened address, so it shows it once. */}
        {(known || !compact) && (
          <span className={styles.chipAddress} title={value}>
            ({compact ? shortenAddress(value) : value})
          </span>
        )}
        <button
          className={styles.chipClear}
          type="button"
          aria-label={clearLabel ?? t('sell_item_modal.beneficiary.clear')}
          disabled={disabled}
          data-testid={`${testId}-clear`}
          onClick={() => onChange('')}
        >
          <CloseIcon fontSize="small" />
        </button>
      </div>
    )
  }

  return (
    <div className={styles.combo}>
      <div
        className={styles.box}
        ref={boxRef}
        data-invalid={invalid || !!duplicate || undefined}
        data-disabled={disabled || undefined}
      >
        <input
          type="text"
          role={withFriends ? 'combobox' : undefined}
          aria-expanded={withFriends ? open : undefined}
          aria-controls={withFriends ? listId : undefined}
          aria-autocomplete={withFriends ? 'list' : undefined}
          autoComplete="off"
          spellCheck={false}
          placeholder={placeholder ?? t('sell_item_modal.beneficiary.placeholder')}
          value={query}
          disabled={disabled}
          data-testid={`${testId}-input`}
          onChange={event => handleInput(event.target.value)}
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          onBlur={() => {
            setOpen(false)
            setTouched(true)
          }}
        />
        {withFriends && (
          <button
            className={styles.comboToggle}
            type="button"
            aria-label={t('sell_item_modal.beneficiary.open')}
            aria-expanded={open}
            aria-controls={listId}
            disabled={disabled}
            data-open={open || undefined}
            data-testid={`${testId}-toggle`}
            // Keep the input's focus so the blur doesn't close the list we're opening.
            onMouseDown={event => event.preventDefault()}
            onClick={() => setOpen(current => !current)}
          >
            <ChevronIcon />
          </button>
        )}
        {showList &&
          createPortal(
            <ul
              className={styles.options}
              ref={listRef}
              role="listbox"
              id={listId}
              data-testid={`${testId}-options`}
              style={listStyle}
            >
              {matches.length === 0 && !isLoadingFriends ? (
                <li className={styles.optionNote} data-testid={`${testId}-no-matches`}>
                  {t('sell_item_modal.beneficiary.no_matches')}
                </li>
              ) : (
                matches.map((friend, index) => (
                  <li
                    className={styles.option}
                    key={friend.address}
                    role="option"
                    aria-selected={index === active}
                    data-active={index === active || undefined}
                    data-testid={`${testId}-option`}
                    onMouseDown={event => event.preventDefault()}
                    onMouseEnter={() => setActive(index)}
                    onClick={() => commit(friend.address)}
                  >
                    {friend.avatarUrl ? (
                      <img className={styles.avatar} src={friend.avatarUrl} alt="" />
                    ) : (
                      <span className={styles.avatarFallback} aria-hidden />
                    )}
                    {friend.name && <span>{friend.name}</span>}
                    {friend.address === self && <span className={styles.youTag}>{youLabel}</span>}
                    <span className={styles.optionAddress}>{shortenAddress(friend.address)}</span>
                  </li>
                ))
              )}
              {/* Self is known at once; the friends join the list under the note when they arrive. */}
              {isLoadingFriends && (
                <li className={styles.optionNote}>
                  <span className={shared.spinner} aria-hidden />
                  {t('sell_item_modal.beneficiary.friends_loading')}
                </li>
              )}
            </ul>,
            document.body
          )}
      </div>
      {listError ? (
        <p className={styles.errorText} data-testid={`${testId}-error`}>
          {listError}
        </p>
      ) : invalid ? (
        <p className={styles.errorText} data-testid={`${testId}-error`}>
          {t('sell_item_modal.beneficiary.invalid')}
        </p>
      ) : (
        duplicate && (
          <p className={styles.errorText} data-testid={`${testId}-error`}>
            {duplicate}
          </p>
        )
      )}
    </div>
  )
}
