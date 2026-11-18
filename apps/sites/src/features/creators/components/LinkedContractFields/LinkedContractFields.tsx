import { useEffect, useId, useMemo, useRef } from 'react'
import { InfoOutlinedIcon } from '~/components/Icons'
import { type ContractNetwork } from '@dcl/schemas'
import { config } from '~/config'
import { useTranslation } from '~/intl'
import {
  checkLinkedContract,
  isAddressBeingTyped,
  isContractAddress,
  type LinkedContractCheck
} from '~/lib/linkedContract'
import { getLinkedContractNetworks } from '~/lib/thirdParty'
import { Select } from '~/components/Select'
import nameInput from '~/components/CollectionNameInput/CollectionNameInput.module.css'
import { cx } from '~/lib/cx'
import shared from '~/styles/shared.module.css'
import styles from './LinkedContractFields.module.css'

const CONTRACT_CHECK_DEBOUNCE_MS = 1000

export type LinkedContractFieldsValue = {
  network: ContractNetwork | null
  address: string
  /** Null until a full address sits on a picked network; 'ok' is what a submit needs. */
  check: LinkedContractCheck | 'checking' | null
}

export const EMPTY_LINKED_CONTRACT: LinkedContractFieldsValue = { network: null, address: '', check: null }

type Props = {
  value: LinkedContractFieldsValue
  onChange: (value: LinkedContractFieldsValue) => void
}

/** The NFT contract's network and address, checked on chain after a pause in typing (the legacy create modal's fields). */
export function LinkedContractFields({ value, onChange }: Props) {
  const { t } = useTranslation()
  const { network, address, check } = value
  const checkTimer = useRef<ReturnType<typeof setTimeout>>()
  const checkRun = useRef(0)
  const onChangeRef = useRef(onChange)
  onChangeRef.current = onChange
  const valueRef = useRef(value)
  valueRef.current = value

  const isProduction = config.get('ENVIRONMENT') === 'production'
  const networks = useMemo(() => getLinkedContractNetworks(isProduction), [isProduction])
  const networkOptions = useMemo(
    () => networks.map(option => ({ value: option, label: t(`linked_collection.networks.${option}`) })),
    [networks, t]
  )

  // Only the fields re-run the check; the parent's onChange is read through a ref so it never does.
  useEffect(() => {
    clearTimeout(checkTimer.current)
    const run = ++checkRun.current
    if (!network || !isContractAddress(address)) {
      onChangeRef.current({ network, address, check: null })
      return
    }
    onChangeRef.current({ network, address, check: 'checking' })
    checkTimer.current = setTimeout(() => {
      void checkLinkedContract(network, address).then(result => {
        // A keystroke that landed while the check ran is kept; its own effect run re-checks.
        const latest = valueRef.current
        if (run === checkRun.current && latest.network === network && latest.address === address) {
          onChangeRef.current({ network, address, check: result })
        }
      })
    }, CONTRACT_CHECK_DEBOUNCE_MS)
    return () => clearTimeout(checkTimer.current)
  }, [network, address])

  const contractErrorId = useId()
  const contractError =
    check === 'not_nft'
      ? t('create_linked_collection_modal.error_not_nft')
      : check === 'error'
        ? t('create_linked_collection_modal.error_contract_check')
        : null

  return (
    <>
      <div className={nameInput.field}>
        <div className={nameInput.label}>
          {t('create_linked_collection_modal.network')}
          <Select
            value={network}
            options={networkOptions}
            placeholder={t('create_linked_collection_modal.network_placeholder')}
            ariaLabel={t('create_linked_collection_modal.network')}
            testId="linked-network"
            onChange={next => onChange({ network: next, address, check })}
          />
        </div>
      </div>
      <div className={nameInput.field}>
        <label className={nameInput.label}>
          {t('create_linked_collection_modal.contract')}
          <div className={nameInput.box} data-invalid={contractError ? true : undefined}>
            <input
              value={address}
              placeholder={t('create_linked_collection_modal.contract_placeholder')}
              spellCheck={false}
              autoComplete="off"
              aria-invalid={contractError ? true : undefined}
              aria-describedby={contractError ? contractErrorId : undefined}
              data-testid="linked-contract-address-input"
              onChange={event => {
                const next = event.target.value.trim()
                if (!isAddressBeingTyped(next)) return
                onChange({ network, address: next, check })
              }}
            />
            {check === 'checking' && (
              <span className={cx(shared.spinner, styles.spinner)} data-testid="linked-contract-checking" aria-hidden />
            )}
          </div>
        </label>
        {contractError ? (
          <p className={nameInput.errorText} id={contractErrorId} data-testid="linked-contract-error">
            {contractError}
          </p>
        ) : (
          <p className={nameInput.hintText} data-testid="linked-contract-hint">
            <InfoOutlinedIcon fontSize="inherit" aria-hidden />
            {t('create_linked_collection_modal.contract_hint')}
          </p>
        )}
      </div>
    </>
  )
}
