import type { BuilderApiAssetPack } from './types.ts';

export class Api {
  url: string;

  constructor(url: string) {
    this.url = url;
  }

  async getAssetPacks() {
    const url = `${this.url}/v1/assetPacks`;
    const resp = await fetch(url);
    if (!resp.ok) {
      const error = await resp.text();
      throw new Error(error);
    }
    const json = (await resp.json()) as { data: BuilderApiAssetPack[] };
    return json.data;
  }

  async getThumbnail(assetPackId: string) {
    const url = `${this.url}/v1/storage/assetPacks/${assetPackId}.png`;
    const resp = await fetch(url);
    const arrayBuffer = await resp.arrayBuffer();
    return Buffer.from(arrayBuffer);
  }

  async getContent(hash: string) {
    const url = `${this.url}/v1/storage/contents/${hash}`;
    const resp = await fetch(url);
    const arrayBuffer = await resp.arrayBuffer();
    return Buffer.from(arrayBuffer);
  }
}
