import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { REJECTION_REASON_MAX_LENGTH, rejectionNoteLimit, rejectionReason } from './admin-copy'
import { RejectEventModal } from './RejectEventModal'

afterEach(cleanup)

function mount() {
  const onSubmit = vi.fn()
  render(<RejectEventModal busy={false} onClose={vi.fn()} onSubmit={onSubmit} />)
  const note = screen.getByRole<HTMLTextAreaElement>('textbox', { name: 'Other (optional)' })
  return { onSubmit, note }
}

describe('when rejecting an event with a note', () => {
  it('should count the note against the whole stored reason when no reason is chosen', async () => {
    const { note } = mount()
    await userEvent.type(note, 'Wrong parcel')
    expect(note.maxLength).toBe(REJECTION_REASON_MAX_LENGTH)
    expect(screen.getByText(`12/${REJECTION_REASON_MAX_LENGTH}`)).toBeTruthy()
    expect(note.getAttribute('aria-describedby')).toBe(
      screen.getByText(`12/${REJECTION_REASON_MAX_LENGTH}`).id
    )
  })

  it('should leave room for the chosen reasons and the separator', async () => {
    const { note } = mount()
    await userEvent.click(screen.getByRole('checkbox', { name: /Invalid image/ }))
    // "Invalid image" and ". " come before the note.
    expect(note.maxLength).toBe(REJECTION_REASON_MAX_LENGTH - 'Invalid image. '.length)
    await userEvent.click(screen.getByRole('checkbox', { name: /Invalid location/ }))
    const limit = REJECTION_REASON_MAX_LENGTH - 'Invalid image, Invalid location. '.length
    expect(note.maxLength).toBe(limit)
    expect(screen.getByText(`0/${limit}`)).toBeTruthy()
  })

  it('should submit a note that fills the limit whole, and store it as typed', async () => {
    const { note, onSubmit } = mount()
    await userEvent.click(screen.getByRole('checkbox', { name: /Invalid image/ }))
    const typed = 'x'.repeat(rejectionNoteLimit(['invalid_image']))
    fireEvent.change(note, { target: { value: typed } })
    await userEvent.click(screen.getByRole('button', { name: 'Submit' }))
    expect(onSubmit).toHaveBeenCalledWith({ reasons: ['invalid_image'], notes: typed })
    const stored = rejectionReason(['invalid_image'], typed)
    expect(stored).toBe(`Invalid image. ${typed}`)
    expect(stored).toHaveLength(REJECTION_REASON_MAX_LENGTH)
  })

  describe('and choosing another reason makes the note too long', () => {
    it('should say so and refuse to submit instead of cutting the note', async () => {
      const { note, onSubmit } = mount()
      await userEvent.click(screen.getByRole('checkbox', { name: /Invalid image/ }))
      const typed = 'x'.repeat(rejectionNoteLimit(['invalid_image']))
      fireEvent.change(note, { target: { value: typed } })
      await userEvent.click(screen.getByRole('checkbox', { name: /Invalid location/ }))
      const limit = rejectionNoteLimit(['invalid_image', 'invalid_location'])
      expect(screen.getByRole('alert').textContent).toBe(
        `The note is too long for the chosen reasons. Keep it to ${limit} characters.`
      )
      expect(note.getAttribute('aria-invalid')).toBe('true')
      expect(note.value).toBe(typed)
      const submit = screen.getByRole<HTMLButtonElement>('button', { name: 'Submit' })
      expect(submit.disabled).toBe(true)
      await userEvent.click(submit)
      expect(onSubmit).not.toHaveBeenCalled()
    })
  })
})
