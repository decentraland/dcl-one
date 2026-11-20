import { describe, expect, it } from 'vitest'
import { render } from '@testing-library/react'
import { MessageText } from '../features/chat/chatText'

const STYLES = { url: 'url', mention: 'mention', location: 'location', world: 'world', bold: 'bold', italic: 'italic' }

function runs(text: string): Array<[string, string]> {
  const { container } = render(<MessageText text={text} members={[]} styles={STYLES} onMention={() => undefined} onLocation={() => undefined} onWorld={() => undefined} />)
  return [...container.children].map((el) => [el.textContent ?? '', el.className])
}

describe('bold and italic in a chat message', () => {
  it('styles the words between the tags and shows no tag', () => {
    expect(runs('a <b>bold</b> and <i>slanted</i> word')).toEqual([
      ['a ', ''],
      ['bold', 'bold'],
      [' and ', ''],
      ['slanted', 'italic'],
      [' word', '']
    ])
  })

  it('nests, and styles a link inside', () => {
    expect(runs('<b>go <i>to 10,20</i> now</b>')).toEqual([
      ['go ', 'bold'],
      ['to ', 'bold italic'],
      ['10,20', 'location bold italic'],
      [' now', 'bold']
    ])
  })

  it('an unclosed tag runs to the end; a closing tag alone does nothing', () => {
    expect(runs('plain</b> then <i>slanted')).toEqual([
      ['plain', ''],
      [' then ', ''],
      ['slanted', 'italic']
    ])
    // opened twice, closed twice
    expect(runs('<b><b>a</b>b</b>c')).toEqual([
      ['a', 'bold'],
      ['b', 'bold'],
      ['c', '']
    ])
  })

  it('shows any other tag as it was typed', () => {
    expect(runs('<B>x</B> <br> <color=red>')).toEqual([['<B>x</B> <br> <color=red>', '']])
  })
})
