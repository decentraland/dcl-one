import { describe, expect, it, vi } from 'vitest';

// The same stubs as scene-mcp.test.ts: listing the tools needs no window, preview or log.
vi.mock('electron-log/main', () => ({ default: { info: vi.fn(), warn: vi.fn(), error: vi.fn() } }));
vi.mock('../src/modules/window', () => ({ getWindow: () => undefined }));
vi.mock('../src/mainWindow', () => ({ MAIN_WINDOW_ID: 'main' }));
vi.mock('../src/modules/explorer-gateway', () => ({
  callExplorerTool: vi.fn(),
  explorerTools: () => [{ name: 'move_player', description: 'From the preview' }],
  onExplorerToolsChanged: () => () => {},
  gatewayProject: () => null,
  launchPreview: vi.fn(),
  previewStatus: vi.fn(),
  stopExplorerGateway: vi.fn(),
  stopPreview: vi.fn(),
}));

import { DCL_SYSTEM_PROMPT } from '../src/modules/ai-prompt';
import { hashTools, sha256, stableJson } from '../src/modules/ai-sharing/hashes';
import { assistantToolsHash, buildAssistantManifest } from '../src/modules/ai-sharing/manifest';
import { buildTurnRecord, type TurnCapture } from '../src/modules/ai-sharing/record';

const COMMIT = '0123456789abcdef0123456789abcdef01234567';

// Hashed independently (Python: json.dumps(sorted by name, sort_keys=True,
// separators=(',', ':'), ensure_ascii=False)), so a change to the serialization fails here
// before it splits the app's hashes from a manifest built by an older script.
const FIXTURE = [
  {
    name: 'b',
    title: null,
    inputSchema: { type: 'object', properties: { y: { type: 'string' }, x: { type: 'number' } } },
  },
  { name: 'a', description: 'Does "a" — fast' },
];
const FIXTURE_HASH = '90b0e46dce9b79af8f7cfb30a8225010525e49f1588a6d74682529309e189f66';

describe('when hashing the tool definitions', () => {
  it('should match the fixture hashed outside this code', () => {
    expect(hashTools(FIXTURE)).toBe(FIXTURE_HASH);
  });

  it('should not depend on the order of the tools or of their keys', () => {
    const reordered = [
      { description: 'Does "a" — fast', name: 'a' },
      {
        inputSchema: {
          properties: { x: { type: 'number' }, y: { type: 'string' } },
          type: 'object',
        },
        title: null,
        name: 'b',
      },
    ];

    expect(hashTools(reordered)).toBe(FIXTURE_HASH);
  });

  it('should hash no tools as an empty list', () => {
    expect(hashTools([])).toBe(sha256('[]'));
  });

  it('should leave undefined fields out, as JSON does', () => {
    expect(stableJson({ a: 1, b: undefined })).toBe('{"a":1}');
  });
});

describe('when building the release manifest', () => {
  const release = { appVersion: '1.2.3', dclOneCommit: COMMIT };

  it('should carry the prompt, the tools and the skills denylist with their hashes', async () => {
    const manifest = await buildAssistantManifest(release);

    expect(manifest).toMatchObject({
      appVersion: '1.2.3',
      dclOneCommit: COMMIT,
      systemPrompt: DCL_SYSTEM_PROMPT,
      systemPromptHash: sha256(DCL_SYSTEM_PROMPT),
      skillsDenylist: ['create-scene', 'deploy-scene', 'deploy-worlds', 'migrate-sdk6-to-sdk7'],
    });
    expect(manifest.toolsHash).toBe(hashTools(manifest.tools));
    expect(manifest.tools.map(t => t.name)).toContain('scene_state');
    expect(manifest.tools[0]).toEqual(
      expect.objectContaining({ name: 'ask_user', inputSchema: expect.any(Object) }),
    );
  });

  it("should leave out the preview's explorer tools, which Creator Hub doesn't define", async () => {
    const manifest = await buildAssistantManifest(release);

    expect(manifest.tools.map(t => t.name)).not.toContain('explorer_move_player');
  });

  it('should name the same assistant a shared turn names', async () => {
    const manifest = await buildAssistantManifest(release);
    const capture: TurnCapture = {
      address: '0x1234567890abcdef1234567890abcdef12345678',
      conversationId: 'c1',
      turnId: 't1',
      share: { wallet: 'x', turnIndex: 0, selectedEntityCount: 0, mentionedAssetCount: 0 },
      startedAt: 0,
      appVersion: '1.2.3',
      os: 'darwin',
      provider: 'claude',
      requestedModel: 'default',
      systemPrompt: DCL_SYSTEM_PROMPT,
      skillsCommit: null,
      toolsHash: await assistantToolsHash(),
      prompt: 'hi',
      attachments: [],
      projectId: null,
      info: {},
      responseParts: ['hello'],
      tools: [],
    };
    const record = buildTurnRecord(capture, {
      outcome: 'ok',
      errorKind: null,
      mutations: 0,
      endedAt: 1,
    });

    expect(record?.assistant.systemPromptHash).toBe(manifest.systemPromptHash);
    expect(record?.assistant.toolsHash).toBe(manifest.toolsHash);
  });
});
