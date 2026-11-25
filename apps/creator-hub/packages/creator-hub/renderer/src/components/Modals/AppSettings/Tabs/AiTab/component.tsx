import { useCallback, useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import Button from '@ui/atoms/Button';
import Checkbox from '@ui/atoms/Checkbox';
import IconButton from '@ui/atoms/IconButton';
import Spinner from '@ui/atoms/Spinner';
import Text from '@ui/atoms/Text';
import TextField from '@ui/atoms/TextField';
import { Tooltip } from '@dcl/one-ui';

import { ai, misc } from '#preload';

import { cliSetup, type AiProviderInfo } from '/shared/types/ai';
import { t } from '/@/modules/store/translation/utils';
import { ContentCopyIcon, KeyboardArrowDownIcon } from '@ui/atoms/icons-material';
import { useCliSignIn } from '/@/hooks/useCliSignIn';
import { SignInCodeField } from '/@/components/SignInCodeField';
import { ToggleRow } from '../../ToggleRow';
import type { BaseTabProps } from '../../types';

import './styles.css';

/** A lightweight expand/collapse section built from a chevron and a title. */
function Accordion({
  title,
  expanded,
  onToggle,
  children,
}: {
  title: ReactNode;
  expanded: boolean;
  onToggle: () => void;
  children: ReactNode;
}) {
  return (
    <div className="SettingsAccordion">
      <button
        type="button"
        className="SettingsAccordionHeader"
        onClick={onToggle}
      >
        <KeyboardArrowDownIcon
          fontSize="small"
          className={`SettingsAccordionChevron${expanded ? ' expanded' : ''}`}
        />
        <span className="SettingsAccordionTitle">{title}</span>
      </button>
      {expanded && <div className="SettingsAccordionBody">{children}</div>}
    </div>
  );
}

/** A read-only command line with a copy button and an optional label above it. */
function CommandField({ label, value }: { label?: string; value: string }) {
  const [copied, setCopied] = useState(false);
  const copy = useCallback(() => {
    void misc.copyToClipboard(value).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }, [value]);
  return (
    <div className="CommandFieldWrapper">
      {label !== undefined && (
        <Text
          variant="caption"
          tone="secondary"
        >
          {label}
        </Text>
      )}
      <div className="CommandField">
        <code>{value}</code>
        <IconButton
          size="small"
          aria-label={t('modal.app_settings.fields.ai_connect.copy')}
          onClick={copy}
        >
          <ContentCopyIcon
            fontSize="small"
            tone={copied ? 'primary' : 'inherit'}
          />
        </IconButton>
      </div>
    </div>
  );
}

/** One agent in the Connect section: status, in-app sign in/out and a Via Terminal fallback. */
function AgentConnect({
  info,
  onRecheck,
}: {
  info: AiProviderInfo;
  onRecheck: () => Promise<void>;
}) {
  const [pending, setPending] = useState(false);
  const track = useCallback((work: Promise<unknown>) => {
    setPending(true);
    void work.finally(() => setPending(false));
  }, []);
  const recheck = useCallback(() => track(onRecheck()), [track, onRecheck]);
  const { signIn, start, cancel, submitCode } = useCliSignIn(info.id, recheck);
  const reloginInTerminal = info.needsLogin === true && !info.managedSignIn;
  const [terminalOpen, setTerminalOpen] = useState(
    reloginInTerminal || (info.desktopInstalled === true && !info.managedSignIn),
  );
  const cmds = cliSetup(info.id, navigator.userAgent.includes('Windows'));

  const [signOutError, setSignOutError] = useState<string | null>(null);
  const handleSignOut = useCallback(() => {
    setSignOutError(null);
    track(
      ai
        .signOutCli(info.id)
        .catch(e => setSignOutError(e instanceof Error ? e.message : String(e)))
        .then(onRecheck),
    );
  }, [track, info.id, onRecheck]);
  const spinner = (signIn.busy || pending) && (
    <Spinner
      ring="wide"
      size={10}
      color="inherit"
      role="progressbar"
    />
  );

  return (
    <div className="AgentConnect">
      <div className="AgentConnectHeader">
        <span className="AgentName">{info.label}</span>
        {info.available && info.needsLogin === true && (
          <span className="AgentStatus">
            <span className="AgentStatusDot warning" />
            {t('editor.ai.setup.needs_login_status')}
          </span>
        )}
        {info.available && info.needsLogin !== true && (
          <span className="AgentStatus">
            <span className="AgentStatusDot" />
            {t('modal.app_settings.fields.ai_connect.connected')}
          </span>
        )}
        {info.available ? (
          info.managedSignIn === true && (
            <Tooltip
              label={t('modal.app_settings.fields.ai_connect.sign_out_shared', {
                agent: info.label,
              })}
              disabled={pending}
              enterDelay={150}
              side="top"
              portal
              className="AgentSignTip"
            >
              <button
                type="button"
                className="AgentSignLink"
                disabled={pending}
                onClick={handleSignOut}
              >
                {spinner}
                {t('modal.app_settings.fields.ai_connect.sign_out')}
              </button>
            </Tooltip>
          )
        ) : signIn.busy ? (
          <button
            type="button"
            className="AgentSignLink"
            onClick={cancel}
          >
            {spinner}
            {t('modal.app_settings.fields.ai_connect.cancel')}
          </button>
        ) : info.managedSignIn === true ? (
          <button
            type="button"
            className="AgentSignLink primary"
            disabled={pending}
            onClick={() => void start()}
          >
            {spinner}
            {t('modal.app_settings.fields.ai_connect.sign_in')}
          </button>
        ) : null}
      </div>
      {signIn.busy && signIn.message !== '' && (
        <Text
          variant="caption"
          tone="secondary"
        >
          {signIn.message}
        </Text>
      )}
      {signIn.url !== null && <CommandField value={signIn.url} />}
      {signIn.busy && signIn.url !== null && signIn.acceptsCode && (
        <SignInCodeField onSubmit={submitCode} />
      )}
      {signOutError !== null && (
        <Text
          variant="caption"
          tone="error"
        >
          {signOutError}
        </Text>
      )}
      {signIn.error !== null && (
        <Text
          variant="caption"
          tone="error"
        >
          {signIn.error}
        </Text>
      )}
      {info.needsLogin === true && (
        <Text
          variant="caption"
          tone="secondary"
        >
          {t(
            info.managedSignIn
              ? 'editor.ai.setup.needs_login_managed_help'
              : 'editor.ai.setup.needs_login_help',
          )}
        </Text>
      )}
      {info.desktopInstalled === true && (
        <Text
          variant="caption"
          tone="secondary"
        >
          {t('editor.ai.setup.desktop_detected')}
        </Text>
      )}
      {info.id === 'gemini' && (
        <Text
          variant="caption"
          tone="secondary"
        >
          {t('editor.ai.setup.gemini_plan_note')}
        </Text>
      )}
      <Accordion
        title={t('modal.app_settings.fields.ai_connect.via_terminal')}
        expanded={terminalOpen}
        onToggle={() => setTerminalOpen(o => !o)}
      >
        <CommandField
          label={t('modal.app_settings.fields.ai_connect.install')}
          value={cmds.install}
        />
        {(reloginInTerminal ? cmds.relogin : [cmds.signin]).map((cmd, i) => (
          <CommandField
            key={cmd}
            label={i === 0 ? t('modal.app_settings.fields.ai_connect.sign_in_label') : undefined}
            value={cmd}
          />
        ))}
        <button
          type="button"
          className="AgentGuideLink"
          onClick={() => void misc.openExternal(cmds.guideUrl)}
        >
          {t('editor.ai.setup.install_guide', { provider: info.label })}
        </button>
      </Accordion>
      {info.needsLogin === true && (
        <Text
          variant="caption"
          tone="secondary"
        >
          {t('editor.ai.setup.api_key_alternative', {
            envVar: cmds.apiKeyEnv,
            setting: t('modal.app_settings.fields.ai_api_key.label'),
          })}
        </Text>
      )}
    </div>
  );
}

// `children` renders at the end of the tab, inside its layout: the settings modal passes the
// store-backed sharing section this way, since this tab itself takes only settings props.
const AiTab: React.FC<BaseTabProps & { children?: ReactNode }> = ({
  settings,
  updateSettings,
  children,
}) => {
  const handleAiAssistantChange = useCallback(
    (checked: boolean) => {
      updateSettings({
        ...settings,
        aiAssistant: checked,
        useApiKeyFromEnv: checked ? settings.useApiKeyFromEnv : false,
      });
    },
    [settings, updateSettings],
  );

  const handleUseApiKeyFromEnvChange = useCallback(
    (checked: boolean) => {
      updateSettings({ ...settings, useApiKeyFromEnv: checked });
    },
    [settings, updateSettings],
  );

  const handleExposeMcpChange = useCallback(
    (checked: boolean) => {
      updateSettings({ ...settings, exposeMcpServer: checked });
    },
    [settings, updateSettings],
  );

  const [providers, setProviders] = useState<AiProviderInfo[]>([]);
  const [connectOpen, setConnectOpen] = useState(true);
  const recheckProviders = useCallback(
    () =>
      ai.detectProviders(settings.useApiKeyFromEnv).then(
        list => setProviders(list),
        () => setProviders([]),
      ),
    [settings.useApiKeyFromEnv],
  );
  useEffect(() => {
    if (!settings.aiAssistant) {
      setProviders([]);
      return;
    }
    void recheckProviders();
    // Signing in or out in a terminal happens outside the app, so check again on return.
    const onFocus = () => void recheckProviders();
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, [settings.aiAssistant, recheckProviders]);

  const [mcpInfo, setMcpInfo] = useState<{ url: string; token: string } | null>(null);
  const [mcpCopied, setMcpCopied] = useState(false);
  useEffect(() => {
    if (!settings.exposeMcpServer) {
      setMcpInfo(null);
      return;
    }
    let cancelled = false;
    const fetchInfo = () =>
      void ai.getMcpServerInfo().then(
        info => !cancelled && setMcpInfo(info),
        () => !cancelled && setMcpInfo(null),
      );
    fetchInfo();
    // The switch flips here before main has granted access (the save is async), so the first
    // fetch can come back empty; main says when access actually changed.
    const { cleanup } = ai.onExternalAccessChanged(fetchInfo);
    return () => {
      cancelled = true;
      cleanup();
    };
  }, [settings.exposeMcpServer]);

  const mcpConfigSnippet =
    mcpInfo &&
    JSON.stringify(
      {
        mcpServers: {
          'creator-hub': {
            type: 'http',
            url: mcpInfo.url,
            headers: { Authorization: `Bearer ${mcpInfo.token}` },
          },
        },
      },
      null,
      2,
    );

  const handleCopyMcpConfig = useCallback(() => {
    if (mcpConfigSnippet === null || mcpConfigSnippet === undefined) return;
    void misc.copyToClipboard(mcpConfigSnippet).then(() => {
      setMcpCopied(true);
      setTimeout(() => setMcpCopied(false), 2000);
    });
  }, [mcpConfigSnippet]);

  return (
    <div className="FormContainer AiTab">
      <div className="AiField">
        <ToggleRow
          checked={!!settings.aiAssistant}
          label={t('modal.app_settings.fields.ai_assistant.label')}
          onChange={handleAiAssistantChange}
        />
        <Text
          variant="body2"
          tone="secondary"
          className="AiHint"
        >
          {t('modal.app_settings.fields.ai_assistant.help')}
        </Text>

        {settings.aiAssistant && (
          <div className="AiNestedField">
            <Checkbox
              className="SettingsCheckbox"
              checked={!!settings.useApiKeyFromEnv}
              onChange={handleUseApiKeyFromEnvChange}
            >
              {t('modal.app_settings.fields.ai_api_key.label')}
            </Checkbox>
            <Text
              variant="caption"
              tone="secondary"
              className="AiHint"
            >
              {t('modal.app_settings.fields.ai_api_key.help')}
            </Text>

            <Accordion
              title={t('modal.app_settings.fields.ai_connect.title')}
              expanded={connectOpen}
              onToggle={() => setConnectOpen(o => !o)}
            >
              {providers.map(info => (
                <AgentConnect
                  key={info.id}
                  info={info}
                  onRecheck={recheckProviders}
                />
              ))}
            </Accordion>
          </div>
        )}
      </div>

      {/* Not nested under the assistant: outside agents are a separate grant, so the switch
          that revokes it must stay visible whatever the assistant setting is. */}
      <hr className="AiDivider" />
      <div className="AiField">
        <ToggleRow
          checked={!!settings.exposeMcpServer}
          label={t('modal.app_settings.fields.mcp_server.label')}
          onChange={handleExposeMcpChange}
        />
        <Text
          variant="body2"
          tone="secondary"
          className="AiHint"
        >
          {t('modal.app_settings.fields.mcp_server.help')}
        </Text>
        {settings.exposeMcpServer && (
          <div className="AiNestedField">
            {mcpConfigSnippet !== null && mcpConfigSnippet !== undefined ? (
              <>
                <TextField
                  aria-label={t('modal.app_settings.fields.mcp_server.label')}
                  value={mcpConfigSnippet}
                  multiline
                  fullWidth
                  minRows={7}
                  readOnly
                />
                <Button
                  className="AiCopyButton"
                  variant="text"
                  tone="neutral"
                  size="sm"
                  startIcon={<ContentCopyIcon fontSize="small" />}
                  onClick={handleCopyMcpConfig}
                >
                  {mcpCopied
                    ? t('modal.app_settings.fields.mcp_server.copied')
                    : t('modal.app_settings.fields.mcp_server.copy')}
                </Button>
                <Text
                  variant="caption"
                  tone="secondary"
                  className="AiHint"
                >
                  {t('modal.app_settings.fields.mcp_server.note')}
                </Text>
              </>
            ) : (
              <Spinner
                ring="wide"
                size={20}
                color="primary"
                role="progressbar"
              />
            )}
          </div>
        )}
      </div>
      {children}
    </div>
  );
};

export default AiTab;
