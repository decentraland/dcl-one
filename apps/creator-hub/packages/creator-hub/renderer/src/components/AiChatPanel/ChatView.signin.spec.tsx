import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ai } from '#preload';

import { TestProviders } from '../../../tests/utils/TestProviders';
import { ChatView, type ChatViewProps } from './ChatView';

vi.mock('/@/modules/store/translation/utils', async importOriginal => ({
  ...((await importOriginal()) as object),
  t: (id: string) => id,
}));

window.HTMLElement.prototype.scrollIntoView = vi.fn();

function renderWith(provider: Partial<ChatViewProps['providers'][number]>) {
  const props: ChatViewProps = {
    providers: [
      { id: 'claude', label: 'Claude', available: true, managedSignIn: false, ...provider },
    ],
    provider: provider.id ?? 'claude',
    messages: [],
    busy: false,
    detecting: false,
    selection: [],
    assetMentions: [],
    recording: null,
    billingDismissed: true,
    sessions: [],
    currentSessionId: '',
    onSend: vi.fn(),
    onStop: vi.fn(),
    onNewChat: vi.fn(),
    onProviderChange: vi.fn(),
    onRevertTurn: vi.fn(),
    onAnswerPrompt: vi.fn(),
    onRecheck: vi.fn(),
    onDismissBilling: vi.fn(),
    onSwitchSession: vi.fn(),
    onDeleteSession: vi.fn(),
    onClearSelection: vi.fn(),
    onRemoveAssetMention: vi.fn(),
    onRemoveRecording: vi.fn(),
    onClose: vi.fn(),
  };
  render(
    <TestProviders>
      <ChatView {...props} />
    </TestProviders>,
  );
  return props;
}

function type(command: string) {
  const composer = screen.getByRole('textbox');
  fireEvent.change(composer, { target: { value: command } });
  fireEvent.keyDown(composer, { key: 'Enter' });
}

describe('AI chat panel', () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  describe('when the agent asked the user to sign in', () => {
    it('should keep the composer enabled and show the sign-out and sign-in steps with the install guide', () => {
      renderWith({ needsLogin: true });
      expect(screen.getByText('editor.ai.setup.needs_login_title')).toBeTruthy();
      expect(screen.getAllByText(/^claude auth/).map(e => e.textContent)).toEqual([
        'claude auth logout',
        'claude auth login',
      ]);
      expect(screen.getByText('editor.ai.setup.install_guide')).toBeTruthy();
      expect(screen.getByText('editor.ai.setup.api_key_alternative')).toBeTruthy();
      expect(screen.getByRole('textbox').hasAttribute('disabled')).toBe(false);
    });
  });

  describe('when an agent the app signed in to asks to sign in again', () => {
    it('should point at /login instead of terminal commands', () => {
      renderWith({ id: 'codex', label: 'Codex', managedSignIn: true, needsLogin: true });
      expect(screen.getByText('editor.ai.setup.needs_login_managed_help')).toBeTruthy();
      expect(screen.queryByText('editor.ai.setup.needs_login_help')).toBeNull();
      expect(screen.queryByText('codex logout')).toBeNull();
    });
  });

  describe('when Claude Code is missing and Claude Desktop is installed', () => {
    it('should show the Desktop note and the terminal steps instead of an in-app sign-in', () => {
      vi.spyOn(ai, 'getMcpServerInfo').mockRejectedValue(new Error('no MCP in tests'));
      renderWith({ available: false, desktopInstalled: true });
      expect(screen.getByText('editor.ai.setup.desktop_detected')).toBeTruthy();
      expect(screen.getByText('curl -fsSL https://claude.ai/install.sh | bash')).toBeTruthy();
      expect(screen.getByText('claude auth login')).toBeTruthy();
      expect(screen.getByText('editor.ai.setup.install_guide')).toBeTruthy();
      expect(screen.queryByText('editor.ai.setup.signin_button')).toBeNull();
    });
  });

  describe('when the agent is signed in', () => {
    it('should not show the sign-in hint', () => {
      renderWith({});
      expect(screen.queryByText('editor.ai.setup.needs_login_title')).toBeNull();
    });
  });
});

describe('AI chat panel account actions', () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  describe('when the app signed in to the agent', () => {
    it('should sign out from the chat menu and detect the agents again', async () => {
      const signOut = vi.spyOn(ai, 'signOutCli').mockResolvedValue();
      const props = renderWith({ managedSignIn: true });

      fireEvent.click(screen.getByRole('button', { name: 'editor.ai.new_chat' }));
      fireEvent.click(screen.getByRole('menuitem', { name: /editor\.ai\.account\.sign_out/ }));

      expect(signOut).toHaveBeenCalledWith('claude');
      await waitFor(() => expect(props.onRecheck).toHaveBeenCalled());
    });

    it('should sign out on /logout without sending it to the agent', async () => {
      const signOut = vi.spyOn(ai, 'signOutCli').mockResolvedValue();
      const props = renderWith({ managedSignIn: true });

      type('/logout');

      expect(signOut).toHaveBeenCalledWith('claude');
      expect(props.onSend).not.toHaveBeenCalled();
      await waitFor(() => expect(props.onRecheck).toHaveBeenCalled());
    });

    it('should sign out and start a new sign-in on /login', async () => {
      const signOut = vi.spyOn(ai, 'signOutCli').mockResolvedValue();
      const signIn = vi.spyOn(ai, 'signInCli').mockReturnValue(new Promise(() => {}));
      const props = renderWith({ managedSignIn: true });

      type('/login');

      expect(signOut).toHaveBeenCalledWith('claude');
      await waitFor(() => expect(signIn).toHaveBeenCalledWith('claude', expect.any(Function)));
      expect(props.onSend).not.toHaveBeenCalled();
    });

    it('should show why the sign-out failed', async () => {
      vi.spyOn(ai, 'signOutCli').mockRejectedValue(new Error('claude sign-out timed out'));
      renderWith({ managedSignIn: true });

      type('/logout');

      expect(await screen.findByText('claude sign-out timed out')).toBeTruthy();
    });
  });

  describe('when the agent signs in from its own terminal', () => {
    it('should offer no sign-out in the chat menu', () => {
      renderWith({});

      fireEvent.click(screen.getByRole('button', { name: 'editor.ai.new_chat' }));

      expect(screen.queryByRole('menuitem', { name: /editor\.ai\.account\.sign_out/ })).toBeNull();
    });

    it('should show the terminal steps on /logout instead of sending it to the agent', () => {
      const signOut = vi.spyOn(ai, 'signOutCli');
      const props = renderWith({});

      type('/logout');

      expect(screen.getByText('editor.ai.account.terminal')).toBeTruthy();
      expect(screen.getAllByText(/^claude auth/).map(e => e.textContent)).toEqual([
        'claude auth logout',
        'claude auth login',
      ]);
      expect(signOut).not.toHaveBeenCalled();
      expect(props.onSend).not.toHaveBeenCalled();
    });
  });

  describe('when a message only mentions /login', () => {
    it('should send it to the agent', () => {
      const props = renderWith({ managedSignIn: true });

      type('how does /login work?');

      expect(props.onSend).toHaveBeenCalledWith('how does /login work?', undefined);
    });
  });
});

describe('AI chat panel with no CLI installed', () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  describe('when access for outside agents is off', () => {
    it('should say how to turn it on instead of showing a connection', async () => {
      vi.spyOn(ai, 'getMcpServerInfo').mockResolvedValue(null);

      renderWith({ available: false });

      expect(await screen.findByText('editor.ai.setup.alt_access_off')).toBeTruthy();
      expect(screen.queryByText('editor.ai.setup.alt_copy')).toBeNull();
    });
  });

  describe('when access for outside agents is on', () => {
    it('should show the connection to copy', async () => {
      vi.spyOn(ai, 'getMcpServerInfo').mockResolvedValue({ url: 'http://x/mcp', token: 'tok' });

      renderWith({ available: false });

      expect(await screen.findByText('editor.ai.setup.alt_copy')).toBeTruthy();
      expect(screen.queryByText('editor.ai.setup.alt_access_off')).toBeNull();
    });
  });
});
