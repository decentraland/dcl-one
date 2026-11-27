import { useId, useState } from 'react'
import { InputField } from '../../../components/InputField/InputField'
import { WebSelect } from '../../../components/InputField/InputFieldWebControls'
import {
  GeneralCheck,
  GeneralLeftArrow,
  GeneralRightArrow,
  GeneralSearch
} from '../../../foundations/icons'
import { Button } from '../../../components/Button/Button'
import { ADMIN_PERMISSIONS, adminCopy } from './admin-copy'
import styles from './UsersTable.module.css'

export type AdminUserRow = {
  address: string
  name?: string
  avatarUrl?: string
  permissions: readonly string[]
}

const ROWS_PER_PAGE = [10, 25, 50] as const

export type UsersTableProps = {
  rows: readonly AdminUserRow[]
  onAdd: () => void
  onEdit: (row: AdminUserRow) => void
}

/**
 * decentraland.org's Users page body: a wallet or name search and "+ Add User", then a table of the
 * accounts with grants (a check per grant), paged 10, 25 or 50 at a time. A row opens its editor.
 */
export function UsersTable({ rows, onAdd, onEdit }: UsersTableProps) {
  const rowsPerPageId = useId()
  const [search, setSearch] = useState('')
  const [page, setPage] = useState(0)
  const [rowsPerPage, setRowsPerPage] = useState<number>(ROWS_PER_PAGE[0])
  const query = search.trim().toLowerCase()
  const filtered = query
    ? rows.filter(
        (row) =>
          row.address.toLowerCase().includes(query) || row.name?.toLowerCase().includes(query)
      )
    : rows
  const pages = Math.max(1, Math.ceil(filtered.length / rowsPerPage))
  const current = Math.min(page, pages - 1)
  const visible = filtered.slice(current * rowsPerPage, (current + 1) * rowsPerPage)
  const from = filtered.length ? current * rowsPerPage + 1 : 0
  const to = current * rowsPerPage + visible.length
  return (
    <>
      <div className={styles.header}>
        <InputField
          surface="web"
          className={styles.search}
          type="search"
          label={adminCopy.users.searchLabel}
          placeholder={adminCopy.users.searchPlaceholder}
          leadingIcon={<GeneralSearch size={20} />}
          value={search}
          onChange={(event) => {
            setSearch(event.target.value)
            setPage(0)
          }}
        />
        <Button size="lg" onClick={onAdd}>
          {adminCopy.users.addUser}
        </Button>
      </div>
      <div className={styles.frame}>
        <div className={styles.scroller}>
          <table className={styles.table} aria-label={adminCopy.users.title}>
            <thead>
              <tr>
                <th scope="col">{adminCopy.users.columns.user}</th>
                {ADMIN_PERMISSIONS.map((permission) => (
                  <th key={permission.id} scope="col" className={styles.center}>
                    {adminCopy.users.columns[permission.id]}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {visible.map((row) => (
                <tr key={row.address} className={styles.row} onClick={() => onEdit(row)}>
                  <th scope="row" className={styles.user}>
                    {row.avatarUrl ? (
                      <img className={styles.avatar} src={row.avatarUrl} alt="" loading="lazy" />
                    ) : (
                      <span className={styles.avatar} aria-hidden="true" />
                    )}
                    <button
                      type="button"
                      className={styles.userButton}
                      onClick={(event) => {
                        event.stopPropagation()
                        onEdit(row)
                      }}
                    >
                      {row.address}
                      {row.name ? ` (${row.name})` : null}
                    </button>
                  </th>
                  {ADMIN_PERMISSIONS.map((permission) => (
                    <td key={permission.id} className={styles.center}>
                      {row.permissions.includes(permission.id) ? (
                        <GeneralCheck
                          className={styles.check}
                          size={24}
                          role="img"
                          aria-label={adminCopy.users.granted}
                        />
                      ) : null}
                    </td>
                  ))}
                </tr>
              ))}
              {visible.length === 0 ? (
                <tr>
                  <td colSpan={ADMIN_PERMISSIONS.length + 1} className={styles.empty}>
                    {rows.length ? adminCopy.users.noMatches : adminCopy.users.empty}
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </div>
        <div className={styles.pagination}>
          <label className={styles.rowsPerPage} htmlFor={rowsPerPageId}>
            {adminCopy.users.rowsPerPage}
          </label>
          <div className={styles.select}>
            <WebSelect
              id={rowsPerPageId}
              value={rowsPerPage}
              onChange={(event) => {
                setRowsPerPage(Number(event.target.value))
                setPage(0)
              }}
            >
              {ROWS_PER_PAGE.map((size) => (
                <option key={size} value={size}>
                  {size}
                </option>
              ))}
            </WebSelect>
          </div>
          <span className={styles.range} aria-live="polite">
            {adminCopy.users.range(from, to, filtered.length)}
          </span>
          <Button
            variant="text"
            size="sm"
            iconOnly
            aria-label={adminCopy.users.previousPage}
            disabled={current === 0}
            onClick={() => setPage(current - 1)}
          >
            <GeneralLeftArrow className={styles.pageIcon} size={20} aria-hidden="true" />
          </Button>
          <Button
            variant="text"
            size="sm"
            iconOnly
            aria-label={adminCopy.users.nextPage}
            disabled={current >= pages - 1}
            onClick={() => setPage(current + 1)}
          >
            <GeneralRightArrow className={styles.pageIcon} size={20} aria-hidden="true" />
          </Button>
        </div>
      </div>
    </>
  )
}
