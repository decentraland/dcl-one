import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { DownloadSuccess } from './DownloadSuccess'

const steps = [
  { title: 'Open the file', text: 'Find it in your downloads.' },
  { title: 'Install', text: 'Follow the installer.', image: '/step2.webp' },
  { title: 'Jump in', text: 'Sign in and play.' }
]

function renderPage(props: Partial<Parameters<typeof DownloadSuccess>[0]> = {}) {
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
  describe('when rendered', () => {
    it('should expose one main landmark with the title as the page heading', () => {
      renderPage()

      expect(screen.getByRole('main')).toBeInTheDocument()
      expect(
        screen.getByRole('heading', { level: 1, name: 'Your download has started' })
      ).toBeInTheDocument()
    })

    it('should render the steps as a named ordered list, each with its label and heading', () => {
      renderPage()

      const items = within(screen.getByRole('list', { name: 'Install steps' })).getAllByRole(
        'listitem'
      )
      expect(items).toHaveLength(3)
      expect(within(items[1]!).getByText('Step 2')).toBeInTheDocument()
      expect(
        within(items[1]!).getByRole('heading', { level: 2, name: 'Install' })
      ).toBeInTheDocument()
    })

    it('should put the step label, heading and text before the step image, as source does', () => {
      renderPage()

      const item = within(screen.getByRole('list', { name: 'Install steps' })).getAllByRole(
        'listitem'
      )[1]!
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

    it('should describe a step image when alt text is given', () => {
      renderPage({
        steps: [{ title: 'Find', text: 'x', image: '/s1.webp', imageAlt: 'Browser downloads' }]
      })

      expect(screen.getByRole('img', { name: 'Browser downloads' })).toHaveAttribute(
        'src',
        '/s1.webp'
      )
    })

    it('should render step images as decorative', () => {
      const { container } = renderPage()

      expect(container.querySelectorAll('img')).toHaveLength(1)
      expect(container.querySelector('img')).toHaveAttribute('alt', '')
    })
  })

  describe('when footer lines are given', () => {
    it('should render each line around a named link it styles itself, reachable and activated from the keyboard', async () => {
      const onClick = vi.fn((event: { preventDefault: () => void }) => event.preventDefault())
      const user = userEvent.setup()
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

      const retry = screen.getByRole('link', { name: 'Try again' })
      expect(retry).toHaveAttribute('href', '/installer.exe')
      expect(retry.closest('p')).toHaveTextContent("Download didn't start? Try again")
      expect(screen.getByRole('link', { name: 'Support' })).toHaveAttribute('href', '/docs/')
      await user.tab()
      expect(retry).toHaveFocus()
      await user.keyboard('{Enter}')
      expect(onClick).toHaveBeenCalledTimes(1)
    })
  })

  describe('when the installer is transferring', () => {
    it('should announce it with a determinate progress bar when progress is known', () => {
      renderPage({ downloading: { label: 'Downloading', progress: 40 } })

      expect(screen.getByRole('status')).toHaveTextContent('Downloading')
      expect(screen.getByRole('progressbar', { name: 'Downloading' })).toHaveAttribute(
        'value',
        '40'
      )
    })

    it('should render an indeterminate progress bar when progress is unknown', () => {
      renderPage({ downloading: { label: 'Downloading', progress: null } })

      expect(screen.getByRole('progressbar', { name: 'Downloading' })).not.toHaveAttribute('value')
    })
  })

  describe('when no transfer is running', () => {
    it('should not render the overlay', () => {
      renderPage({ downloading: null })

      expect(screen.queryByRole('status')).not.toBeInTheDocument()
    })
  })
})
