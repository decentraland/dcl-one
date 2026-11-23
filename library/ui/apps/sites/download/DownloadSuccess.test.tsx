import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { DownloadSuccess } from './DownloadSuccess'

type DownloadSuccessProps = Parameters<typeof DownloadSuccess>[0]

const steps = [
  { title: 'Open the file', text: 'Find it in your downloads.' },
  { title: 'Install', text: 'Follow the installer.', image: '/step2.webp' },
  { title: 'Jump in', text: 'Sign in and play.' }
]

function renderPage(props: Partial<DownloadSuccessProps> = {}) {
  return render(
    <DownloadSuccess
      title="Your download has started"
      subtitle="Follow these steps"
      stepsLabel="Install steps"
      stepLabel={(n) => `Step ${n}`}
      steps={steps}
      {...props}
    />
  )
}

describe('DownloadSuccess', () => {
  let container: HTMLElement

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('when rendered', () => {
    let item: HTMLElement

    beforeEach(() => {
      ;({ container } = renderPage())
      item = within(screen.getByRole('list', { name: 'Install steps' })).getAllByRole(
        'listitem'
      )[1]!
    })

    it('should expose one main landmark', () => {
      expect(screen.getByRole('main')).toBeInTheDocument()
    })

    it('should use the title as the page heading', () => {
      expect(
        screen.getByRole('heading', { level: 1, name: 'Your download has started' })
      ).toBeInTheDocument()
    })

    it('should render the steps as a named ordered list', () => {
      expect(
        within(screen.getByRole('list', { name: 'Install steps' })).getAllByRole('listitem')
      ).toHaveLength(3)
    })

    it('should render each step with its label and heading', () => {
      expect(within(item).getByText('Step 2')).toBeInTheDocument()
      expect(within(item).getByRole('heading', { level: 2, name: 'Install' })).toBeInTheDocument()
    })

    it('should put the step label, heading and text before the step image, as source does', () => {
      const order = [
        within(item).getByText('Step 2'),
        within(item).getByRole('heading', { level: 2, name: 'Install' }),
        within(item).getByText('Follow the installer.'),
        within(item).getByRole('presentation')
      ]
      for (let i = 1; i < order.length; i++) {
        expect(
          order[i - 1]!.compareDocumentPosition(order[i]!) & Node.DOCUMENT_POSITION_FOLLOWING
        ).toBeTruthy()
      }
    })

    it('should render the logo and step images as decorative', () => {
      const images = container.querySelectorAll('img')
      expect(images).toHaveLength(2)
      for (const image of images) expect(image).toHaveAttribute('alt', '')
    })
  })

  describe('when a step image has alt text', () => {
    beforeEach(() => {
      renderPage({
        steps: [{ title: 'Find', text: 'x', image: '/s1.webp', imageAlt: 'Browser downloads' }]
      })
    })

    it('should describe the step image', () => {
      expect(screen.getByRole('img', { name: 'Browser downloads' })).toHaveAttribute(
        'src',
        '/s1.webp'
      )
    })
  })

  describe('when footer lines are given', () => {
    let onClick: Mock<(event: { preventDefault: () => void }) => void>
    let user: ReturnType<typeof userEvent.setup>
    let retry: HTMLElement

    beforeEach(() => {
      onClick = vi.fn((event: { preventDefault: () => void }) => event.preventDefault())
      user = userEvent.setup()
      renderPage({
        footerLines: [
          {
            text: (link) => <>Download didn&apos;t start? {link}</>,
            link: { label: 'Try again', href: '/installer.exe', onClick }
          },
          {
            text: (link) => <>Need help? Go to {link}</>,
            link: { label: 'Support', href: '/docs/' }
          }
        ]
      })
      retry = screen.getByRole('link', { name: 'Try again' })
    })

    it('should render the first line around its named link', () => {
      expect(retry).toHaveAttribute('href', '/installer.exe')
      expect(retry.closest('p')).toHaveTextContent("Download didn't start? Try again")
    })

    it('should render the second line with its named link', () => {
      expect(screen.getByRole('link', { name: 'Support' })).toHaveAttribute('href', '/docs/')
    })

    describe('and the first link is reached and activated from the keyboard', () => {
      beforeEach(async () => {
        await user.tab()
      })

      it('should focus the link', () => {
        expect(retry).toHaveFocus()
      })

      it('should activate the link on Enter', async () => {
        await user.keyboard('{Enter}')
        expect(onClick).toHaveBeenCalledTimes(1)
      })
    })
  })

  describe('when the installer is transferring', () => {
    describe('and the progress is known', () => {
      beforeEach(() => {
        renderPage({ downloading: { label: 'Downloading', progress: 40 } })
      })

      it('should announce the transfer', () => {
        expect(screen.getByRole('status')).toHaveTextContent('Downloading')
      })

      it('should render a determinate progress bar', () => {
        expect(screen.getByRole('progressbar', { name: 'Downloading' })).toHaveAttribute(
          'value',
          '40'
        )
      })
    })

    describe('and the progress is unknown', () => {
      beforeEach(() => {
        renderPage({ downloading: { label: 'Downloading', progress: null } })
      })

      it('should render an indeterminate progress bar', () => {
        expect(screen.getByRole('progressbar', { name: 'Downloading' })).not.toHaveAttribute(
          'value'
        )
      })
    })
  })

  describe('when no transfer is running', () => {
    beforeEach(() => {
      renderPage({ downloading: null })
    })

    it('should not render the overlay', () => {
      expect(screen.queryByRole('status')).not.toBeInTheDocument()
    })
  })
})
