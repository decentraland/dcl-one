import { type Config } from '/shared/types/config';

import { invoke } from './ipc';
import { produce, type WritableDraft } from 'immer';

export async function getConfig(): Promise<Config> {
  return invoke('config.getConfig');
}

// Each write reads the file, changes it and writes it whole: two at once would each drop the
// other's change, so they run one after another.
let writing: Promise<unknown> = Promise.resolve();

export function setConfig(drafter: (c: WritableDraft<Config>) => void): Promise<void> {
  const write = writing.then(async () => {
    const config = await getConfig();
    const update = produce(config, draft => {
      drafter(draft);
    });
    await invoke('config.writeConfig', update);
  });
  writing = write.catch(() => {});
  return write;
}
