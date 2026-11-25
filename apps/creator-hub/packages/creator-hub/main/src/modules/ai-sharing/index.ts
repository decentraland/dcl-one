import path from 'path';
import log from 'electron-log/main';

import type { AiSendParams } from '/shared/types/ai';
import { consentFor, hasSharingConsent, type AiTurnRecord } from '/shared/types/ai-sharing';

import { getAppVersion, getUserDataPath } from '../electron';
import { Outbox } from './outbox';
import {
  buildTurnRecord,
  readTurnInfo,
  type TurnCapture,
  type TurnEnd,
  type TurnInfo,
} from './record';

export { readTurnInfo, type TurnInfo };

let outbox: Outbox | null = null;

export function getOutbox(): Outbox {
  outbox ??= new Outbox(
    path.join(getUserDataPath(), 'ai-sharing', 'outbox'),
    ({ expired, overLimit }) =>
      log.warn(
        `[AI sharing] dropped queued turns that were never shared: ${expired} older than 7 days, ${overLimit} past the queue's limits`,
      ),
  );
  return outbox;
}

// The wallet the renderer is signed in with, lowercase, or null when signed out. The renderer
// sets it on every sign-in, switch and sign-out. Analytics' user id is not used: it is only set
// when Segment is configured, is never cleared on sign-out, and lags a wallet switch.
let signedInWallet: string | null = null;

export function setSignedInWallet(address: string | null): void {
  signedInWallet =
    address !== null && /^0x[0-9a-fA-F]{40}$/.test(address) ? address.toLowerCase() : null;
}

/**
 * Whether this turn may be recorded, and for which wallet. The renderer asks (`params.share`,
 * naming the wallet it means), but main checks that wallet is the one signed in and has saved
 * consent, so a renderer bug or a wallet switch can't record a turn under the wrong account.
 */
export async function sharingAddress(params: AiSendParams): Promise<string | null> {
  if (params.share === undefined) return null;
  const address = signedInWallet;
  if (address === null || params.share.wallet.toLowerCase() !== address) return null;
  try {
    // Loaded lazily, like ai.ts's session store: config pulls in the Electron app, and ai.ts
    // (which imports this module) must stay importable in unit tests.
    const { getConfigStorage } = await import('../config');
    const settings = await (await getConfigStorage()).get('settings');
    return hasSharingConsent(consentFor(settings?.aiDataSharing, address)) ? address : null;
  } catch (e) {
    log.warn('[AI sharing] could not read consent; not recording this turn', e);
    return null;
  }
}

// Conversations the creator stopped sharing. A turn already running in one of them must not be
// queued when it ends; the renderer persists the choice and repeats it here on every change.
const excludedConversations = new Set<string>();

export function setConversationShared(conversationId: string, shared: boolean): void {
  if (shared) {
    excludedConversations.delete(conversationId);
    return;
  }
  excludedConversations.add(conversationId);
  try {
    getOutbox().purgeConversation(conversationId);
  } catch (e) {
    // A file that can't be removed now is skipped by every upload (see `peek`) and expires.
    log.warn('[AI sharing] could not drop every queued turn of an excluded conversation', e);
  }
}

export const isConversationExcluded = (conversationId: string): boolean =>
  excludedConversations.has(conversationId);

export type CaptureStart = Omit<
  TurnCapture,
  'appVersion' | 'os' | 'info' | 'responseParts' | 'tools'
>;

/** Collects one turn while it runs. `finish` builds the record and queues it, once. */
export class TurnRecorder {
  private readonly capture: TurnCapture;
  private finished = false;

  constructor(start: CaptureStart, appVersion: string) {
    this.capture = {
      ...start,
      appVersion,
      os: process.platform,
      info: {},
      responseParts: [],
      tools: [],
    };
  }

  static async start(start: CaptureStart): Promise<TurnRecorder> {
    return new TurnRecorder(start, await getAppVersion());
  }

  onText(text: string): void {
    this.capture.responseParts.push(text);
  }

  onTool(name: string, detail: string): void {
    this.capture.tools.push([name, detail]);
  }

  /**
   * Builds and queues the record, once, and returns its id (null when nothing was queued).
   * `stillShared` runs first: consent, the account or the conversation's exclusion can change
   * while a turn runs, and the answer at the end is the one that counts.
   */
  async finish(
    end: TurnEnd,
    info: TurnInfo,
    stillShared: () => Promise<boolean>,
    queue: (record: AiTurnRecord) => void = r => getOutbox().enqueue(r),
  ): Promise<string | null> {
    if (this.finished) return null;
    this.finished = true;
    const excluded = () => isConversationExcluded(this.capture.conversationId);
    try {
      if (excluded() || !(await stillShared())) return null;
      const record = buildTurnRecord({ ...this.capture, info }, end);
      if (record === null) {
        log.warn(`[AI sharing] turn ${this.capture.turnId} is too large to share; not queued`);
        return null;
      }
      // Again right before queuing: excluding the conversation or withdrawing consent meanwhile
      // purged a queue this record wasn't in yet.
      if (excluded() || !(await stillShared()) || excluded()) return null;
      queue(record);
      return record.recordId;
    } catch (e) {
      // Never let sharing break a turn: the record is lost, the conversation isn't.
      log.warn('[AI sharing] could not queue the turn record', e);
      return null;
    }
  }
}
