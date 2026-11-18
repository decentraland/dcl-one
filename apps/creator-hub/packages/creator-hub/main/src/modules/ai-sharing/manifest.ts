import { DCL_SYSTEM_PROMPT } from '../ai-prompt';
import { listAssistantTools, type AssistantToolDefinition } from '../scene-mcp';
import { SKILL_DENYLIST } from '../skills';
import { hashTools, sha256 } from './hashes';

/** What a release's assistant is: attached to each release as `assistant-manifest.json`. */
export interface AssistantManifest {
  appVersion: string;
  dclOneCommit: string;
  systemPrompt: string;
  systemPromptHash: string;
  tools: AssistantToolDefinition[];
  toolsHash: string;
  skillsDenylist: string[];
}

export async function buildAssistantManifest(release: {
  appVersion: string;
  dclOneCommit: string;
}): Promise<AssistantManifest> {
  const tools = await listAssistantTools();
  return {
    appVersion: release.appVersion,
    dclOneCommit: release.dclOneCommit,
    systemPrompt: DCL_SYSTEM_PROMPT,
    systemPromptHash: sha256(DCL_SYSTEM_PROMPT),
    tools,
    toolsHash: hashTools(tools),
    skillsDenylist: [...SKILL_DENYLIST].sort(),
  };
}

let toolsHashPromise: Promise<string> | null = null;

/** The hash of the tools every turn gets; the list is fixed for a build, so it is listed once. */
export function assistantToolsHash(): Promise<string> {
  toolsHashPromise ??= Promise.resolve()
    .then(listAssistantTools)
    .then(hashTools)
    .catch((e: unknown) => {
      toolsHashPromise = null;
      throw e;
    });
  return toolsHashPromise;
}

/** A turn that ran without Creator Hub's MCP server got no tools from it. */
export const NO_TOOLS_HASH = hashTools([]);
