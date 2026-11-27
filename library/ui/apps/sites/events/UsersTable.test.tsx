import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { UsersTable, type AdminUserRow } from './UsersTable'

afterEach(cleanup)

const row = (index: number): AdminUserRow => ({
  address: `0x${String(index).padStart(40, '0')}`,
  permissions: index === 1 ? ['approve_any_event'] : []
})

function mount(count: number) {
  const onEdit = vi.fn()
  render(
    <UsersTable
      rows={Array.from({ length: count }, (_, index) => row(index + 1))}
      onAdd={vi.fn()}
      onEdit={onEdit}
    />
  )
  return {
    onEdit,
    previous: screen.getByRole<HTMLButtonElement>('button', { name: 'Previous page' }),
    next: screen.getByRole<HTMLButtonElement>('button', { name: 'Next page' }),
    rowsPerPage: screen.getByRole<HTMLSelectElement>('combobox', { name: 'Rows per page:' })
  }
}

describe('when paging through the accounts', () => {
  it('should keep Previous disabled on the first page and show the next page', async () => {
    const { previous, next } = mount(12)
    expect(previous.disabled).toBe(true)
    expect(previous.getAttribute('data-icon-only')).toBe('true')
    await userEvent.click(previous)
    expect(screen.getByText('1–10 of 12')).toBeTruthy()
    await userEvent.click(next)
    expect(screen.getByText('11–12 of 12')).toBeTruthy()
  })

  it('should keep Next disabled on the last page and go back from there', async () => {
    const { previous, next } = mount(12)
    await userEvent.click(next)
    expect(next.disabled).toBe(true)
    await userEvent.click(next)
    expect(screen.getByText('11–12 of 12')).toBeTruthy()
    await userEvent.click(previous)
    expect(screen.getByText('1–10 of 12')).toBeTruthy()
  })

  it('should disable both arrows when there is no account to page through', () => {
    const { previous, next } = mount(0)
    expect(previous.disabled).toBe(true)
    expect(next.disabled).toBe(true)
    expect(screen.getByText('0–0 of 0')).toBeTruthy()
    expect(screen.getByText('No admins configured')).toBeTruthy()
  })
})

describe('when choosing how many accounts a page shows', () => {
  it('should list more accounts per page and return to the first page', async () => {
    const { next, rowsPerPage } = mount(30)
    await userEvent.click(next)
    expect(screen.getByText('11–20 of 30')).toBeTruthy()
    await userEvent.selectOptions(rowsPerPage, '25')
    expect(rowsPerPage.value).toBe('25')
    expect(screen.getByText('1–25 of 30')).toBeTruthy()
    const table = screen.getByRole('table', { name: 'Users' })
    expect(within(table).getAllByRole('row')).toHaveLength(26)
  })

  it('should offer only the page sizes the table supports', () => {
    const { rowsPerPage } = mount(3)
    expect(
      within(rowsPerPage)
        .getAllByRole('option')
        .map((option) => option.textContent)
    ).toEqual(['10', '25', '50'])
  })
})

describe('when opening an account from its row', () => {
  it('should open the editor once from the row activator', async () => {
    const { onEdit } = mount(2)
    await userEvent.click(screen.getByRole('button', { name: row(1).address }))
    expect(onEdit).toHaveBeenCalledTimes(1)
    expect(onEdit).toHaveBeenCalledWith(row(1))
  })
})
