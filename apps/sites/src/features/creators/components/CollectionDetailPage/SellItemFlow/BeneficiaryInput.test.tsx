import { describe, it, expect, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { type Friend } from '~/lib/friends'
import { BeneficiaryInput } from './BeneficiaryInput'
import { ADDRESS, FRIEND, Providers } from './testUtils'

// A fresh Response per call: a body can only be read once.
vi.stubGlobal(
  'fetch',
  vi.fn(() =>
    Promise.resolve(
      new Response(
        JSON.stringify({ avatars: [{ name: 'Resolved', avatar: { snapshots: { face256: 'face.png' } } }] }),
        {
          status: 200
        }
      )
    )
  )
)

const friends: Friend[] = [
  { address: FRIEND, name: 'Blackbeard', hasClaimedName: true, avatarUrl: 'bb.png' },
  { address: '0x00000000000000000000000000000000000000dd', name: 'Anne Bonny', hasClaimedName: false, avatarUrl: '' }
]

type Props = Partial<Parameters<typeof BeneficiaryInput>[0]>

function renderInput(props: Props = {}) {
  const onChange = vi.fn()
  render(<BeneficiaryInput value="" onChange={onChange} friends={friends} isLoadingFriends={false} {...props} />, {
    wrapper: Providers
  })
  return { onChange }
}

describe('BeneficiaryInput', () => {
  it('lists friends from the chevron, not on focus, narrows them while typing, and picks one on click', async () => {
    const { onChange } = renderInput()
    await userEvent.click(screen.getByTestId('beneficiary-input'))
    expect(screen.queryByTestId('beneficiary-options')).not.toBeInTheDocument()
    await userEvent.click(screen.getByTestId('beneficiary-toggle'))
    expect(screen.getAllByTestId('beneficiary-option')).toHaveLength(2)

    await userEvent.type(screen.getByTestId('beneficiary-input'), 'black')
    const options = screen.getAllByTestId('beneficiary-option')
    expect(options).toHaveLength(1)
    expect(options[0]).toHaveTextContent('Blackbeard')

    await userEvent.click(options[0])
    expect(onChange).toHaveBeenCalledWith(FRIEND)
  })

  it('picks the highlighted friend with the keyboard and keeps Escape from reaching the dialog', async () => {
    const { onChange } = renderInput()
    const onKeyDown = vi.fn()
    document.addEventListener('keydown', onKeyDown)
    const input = screen.getByTestId('beneficiary-input')
    await userEvent.click(input)
    await userEvent.keyboard('{ArrowDown}{Enter}')
    expect(onChange).toHaveBeenCalledWith(friends[1].address)

    await userEvent.keyboard('{Escape}')
    expect(screen.queryByTestId('beneficiary-options')).not.toBeInTheDocument()
    expect(onKeyDown).not.toHaveBeenCalledWith(expect.objectContaining({ key: 'Escape' }))
    document.removeEventListener('keydown', onKeyDown)
  })

  it('says when nothing matches and while friends load', async () => {
    renderInput({ friends: undefined, isLoadingFriends: true })
    await userEvent.click(screen.getByTestId('beneficiary-toggle'))
    expect(screen.getByTestId('beneficiary-options')).toHaveTextContent(/loading/i)
  })

  it('rejects a pasted list unless the field takes lists, which then clear it or keep it with their error', async () => {
    const list = `${FRIEND}\n0x00000000000000000000000000000000000000dd`
    const { onChange } = renderInput()
    await userEvent.click(screen.getByTestId('beneficiary-input'))
    await userEvent.paste(list)
    await userEvent.tab()
    expect(onChange).not.toHaveBeenCalled()
    expect(screen.getByTestId('beneficiary-error')).toHaveTextContent(/valid wallet address/i)
    cleanup()

    const onChangeMany = vi
      .fn<(text: string) => string | null>()
      .mockReturnValueOnce('Nobody new')
      .mockReturnValue(null)
    renderInput({ onChangeMany })
    await userEvent.click(screen.getByTestId('beneficiary-input'))
    await userEvent.paste(list)
    expect(onChangeMany).toHaveBeenLastCalledWith(list)
    expect(screen.getByTestId('beneficiary-input')).toHaveValue(list.replace('\n', ' '))
    expect(screen.getByTestId('beneficiary-error')).toHaveTextContent('Nobody new')

    await userEvent.clear(screen.getByTestId('beneficiary-input'))
    await userEvent.paste(list)
    expect(screen.getByTestId('beneficiary-input')).toHaveValue('')
    expect(screen.queryByTestId('beneficiary-error')).not.toBeInTheDocument()
  })

  it('offers the signed-in wallet first as "You", even without friends, and finds it by typing "you"', async () => {
    const { onChange } = renderInput({ self: ADDRESS, friends: [] })
    await userEvent.click(screen.getByTestId('beneficiary-toggle'))
    const options = screen.getAllByTestId('beneficiary-option')
    expect(options).toHaveLength(1)
    expect(options[0]).toHaveTextContent('You')
    expect(await screen.findByText('Resolved')).toBeInTheDocument()

    await userEvent.type(screen.getByTestId('beneficiary-input'), 'yo')
    expect(screen.getAllByTestId('beneficiary-option')).toHaveLength(1)
    await userEvent.click(screen.getByTestId('beneficiary-option'))
    expect(onChange).toHaveBeenCalledWith(ADDRESS)
  })

  it('drops "You" from the list once chosen, and the chevron with it when nobody else is left', async () => {
    renderInput({ self: ADDRESS, exclude: [ADDRESS] })
    await userEvent.click(screen.getByTestId('beneficiary-toggle'))
    const rest = screen.getAllByTestId('beneficiary-option')
    expect(rest).toHaveLength(2)
    expect(rest[0]).toHaveTextContent('Blackbeard')

    cleanup()
    renderInput({ self: ADDRESS, friends: [], exclude: [ADDRESS] })
    expect(screen.queryByTestId('beneficiary-toggle')).not.toBeInTheDocument()
    expect(screen.getByTestId('beneficiary-input')).not.toHaveAttribute('role')
  })

  it('shows "You" at once while friends load, and once without a profile name', async () => {
    vi.mocked(fetch).mockResolvedValueOnce(new Response(null, { status: 404 }))
    const { rerender } = render(
      <BeneficiaryInput value="" onChange={vi.fn()} friends={undefined} isLoadingFriends self={ADDRESS} />,
      { wrapper: Providers }
    )
    await userEvent.click(screen.getByTestId('beneficiary-toggle'))
    const options = screen.getByTestId('beneficiary-options')
    expect(screen.getAllByTestId('beneficiary-option')).toHaveLength(1)
    expect(options).toHaveTextContent(/loading/i)
    expect(options.textContent?.match(/0x0000…00aa/g)).toHaveLength(1)

    rerender(
      <BeneficiaryInput value={ADDRESS} onChange={vi.fn()} friends={[]} isLoadingFriends={false} self={ADDRESS} />
    )
    const chip = screen.getByTestId('beneficiary-selected')
    expect(chip).toHaveTextContent('You')
    expect(chip.textContent?.match(/0x0000…00aa/g)).toHaveLength(1)
  })

  it('leaves excluded addresses out of the list and drops one typed in without an error', async () => {
    const onExcluded = vi.fn()
    const { onChange } = renderInput({ exclude: [FRIEND], onExcluded })
    await userEvent.click(screen.getByTestId('beneficiary-toggle'))
    expect(screen.getAllByTestId('beneficiary-option')).toHaveLength(1)

    await userEvent.type(screen.getByTestId('beneficiary-input'), FRIEND.toUpperCase().replace('0X', '0x'))
    expect(onExcluded).toHaveBeenCalledTimes(1)
    expect(onChange).not.toHaveBeenCalled()
    expect(screen.queryByTestId('beneficiary-error')).not.toBeInTheDocument()
    expect(screen.getByTestId('beneficiary-input')).toHaveValue('')
  })

  it('shows a chosen friend with their name and avatar, and a pasted address with its profile name', async () => {
    const { rerender } = render(
      <BeneficiaryInput value={FRIEND} onChange={vi.fn()} friends={friends} isLoadingFriends={false} />,
      { wrapper: Providers }
    )
    expect(screen.getByTestId('beneficiary-name')).toHaveTextContent('Blackbeard')
    expect(screen.getByTestId('beneficiary-selected').querySelector('img')).toHaveAttribute('src', 'bb.png')

    const stranger = '0x00000000000000000000000000000000000000ee'
    rerender(<BeneficiaryInput value={stranger} onChange={vi.fn()} friends={friends} isLoadingFriends={false} />)
    expect(screen.getByTestId('beneficiary-name')).toHaveTextContent('0x0000…00ee')
    expect(await screen.findByText('Resolved')).toBeInTheDocument()
  })
})
