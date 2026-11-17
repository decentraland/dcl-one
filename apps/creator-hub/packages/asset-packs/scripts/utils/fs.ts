import fs from 'fs';
import { resolve, dirname } from 'path';

export async function exists(path: string) {
  try {
    await fs.promises.stat(path);
    return true;
  } catch (error) {
    const { code } = error as NodeJS.ErrnoException;
    if (code === 'ENOENT') return false;
    throw new Error(code);
  }
}

export async function createDirectory(path: string) {
  if (await exists(path)) {
    return;
  }
  await fs.promises.mkdir(path, { recursive: true });
}

export async function writeFile(path: string, data: string | Buffer) {
  await createDirectory(dirname(path));
  await fs.promises.writeFile(path, data, 'utf8');
}

export async function readJson(path: string) {
  return JSON.parse(await fs.promises.readFile(path, 'utf8'));
}

export function readBuffer(path: string) {
  return fs.promises.readFile(path);
}

export async function isDirectory(path: string) {
  return (await fs.promises.lstat(path)).isDirectory();
}

export function list(path: string) {
  return fs.promises.readdir(path);
}

export async function getSubfolders(path: string) {
  const paths = await list(path);
  const results = await Promise.all(
    paths.map(async file => {
      return {
        file,
        isDirectory: await isDirectory(resolve(path, file)),
      };
    }),
  );
  return results.filter(result => result.isDirectory).map(result => resolve(path, result.file));
}

export async function getFiles(path: string) {
  const paths = await list(path);
  const results = await Promise.all(
    paths.map(async file => {
      return {
        file,
        isDirectory: await isDirectory(resolve(path, file)),
      };
    }),
  );
  const files: string[] = [];
  for (const result of results) {
    if (result.isDirectory) {
      const subfiles = await getFiles(resolve(path, result.file));
      for (const subfile of subfiles) {
        files.push(subfile);
      }
    } else {
      files.push(resolve(path, result.file));
    }
  }
  return files;
}

export function move(from: string, to: string) {
  return fs.promises.rename(from, to);
}
