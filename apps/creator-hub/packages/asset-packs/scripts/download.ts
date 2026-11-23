import path from 'path';
import { rm } from 'fs/promises';
import { Api } from './utils/builder/api.ts';
import { createDirectory, exists, writeFile } from './utils/fs.ts';
import { LocalFileSystem } from './utils/local.ts';

async function main() {
  console.log('Creating dist directory...');
  const packs = path.resolve(import.meta.dirname, '..', 'packs');
  await createDirectory(packs);

  const builder = new Api('https://builder-api.decentraland.org');
  const local = new LocalFileSystem(packs);

  console.log('Downloading asset packs...');
  const assetPacks = await builder.getAssetPacks();
  for (const assetPack of assetPacks) {
    console.log(`Writing asset pack "${assetPack.title}"...`);
    await local.writeAssetPack(assetPack);

    console.log('Downloading thumbnail...');
    const assetPackPath = local.getAssetPackPath(assetPack.title);
    const thumbnailPath = `${assetPackPath}/thumbnail.png`;
    if (!(await exists(thumbnailPath))) {
      const thumbnail = await builder.getThumbnail(assetPack.id);
      console.log('Saving thumbnail...');
      await writeFile(`${assetPackPath}/thumbnail.png`, thumbnail);
    }
    for (const asset of assetPack.assets) {
      // only download non-smart items
      if (asset.script === null) {
        console.log(`Writing asset "${asset.name}"...`);
        const assetPath = await local.writeAsset(assetPack, asset);
        for (const path in asset.contents) {
          const contentPath = `${assetPath}/${path}`;
          if (!(await exists(contentPath))) {
            const hash = asset.contents[path];
            const content = await builder.getContent(hash);
            console.log(`Writing file "${path}"...`);
            await writeFile(contentPath, content);
          }

          const thumbnailPath = `${assetPath}/thumbnail.png`;
          if (!(await exists(thumbnailPath))) {
            const thumbnail = await builder.getContent(asset.thumbnail);
            console.log(`Writing thumbnail for "${asset.name}"...`);
            await writeFile(thumbnailPath, thumbnail);
          }
        }
      }
    }

    const isValid = await exists(local.getAssetsPath(assetPack.title));
    if (!isValid) {
      console.log(`Removing empty asset pack "${assetPack.title}"`);
      await rm(assetPackPath, { recursive: true, force: true });
    }
  }
}

main().catch(error => {
  console.error(error);
  process.exit(1);
});
