/* oxlint-disable no-useless-escape */

import fs from 'fs/promises';
import path from 'path';
import extract from '@dcl/zip-extract';
import log from 'electron-log/main';
import { fetch } from '/shared/fetch';

// Function to parse GitHub URL for root or subfolder
function parseGitHubUrl(githubUrl: string) {
  const rootRegex = /https:\/\/github\.com\/([^\/]+)\/([^\/]+)$/;
  const subfolderRegex = /https:\/\/github\.com\/([^\/]+)\/([^\/]+)\/tree\/([^\/]+)\/?(.*)/;
  let match = githubUrl.match(subfolderRegex);
  if (match) {
    return {
      owner: match[1],
      repo: match[2],
      branch: match[3],
      path: match[4],
    };
  } else {
    match = githubUrl.match(rootRegex);
    if (match) {
      return {
        owner: match[1],
        repo: match[2],
        branch: 'main', // Default to main if no branch is specified
        path: '', // No specific path for root
      };
    }
  }
  throw new Error("URL doesn't match the expected GitHub format.");
}

// GitHub's zipball answers in under a second when it's well, but a slow answer is common enough
// that the shared 5 s request timeout failed scene creation from a template. The limit covers the
// whole download, body included: the shared fetch's timeout stops at the headers.
const ZIPBALL_TIMEOUT_MS = 30_000;
const ZIPBALL_ATTEMPTS = 2;

async function downloadZipball(apiUrl: string): Promise<ArrayBuffer> {
  const response = await fetch(apiUrl, undefined, ZIPBALL_TIMEOUT_MS);
  if (!response.ok) {
    await response.body?.cancel().catch(() => undefined);
    throw Object.assign(new Error(`GitHub API request failed with status ${response.status}`), {
      retryable: response.status >= 500,
    });
  }
  let timer: ReturnType<typeof setTimeout> | undefined;
  const stalled = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      void response.body?.cancel().catch(() => undefined);
      reject(Object.assign(new Error(`Download of ${apiUrl} stalled`), { retryable: true }));
    }, ZIPBALL_TIMEOUT_MS);
  });
  try {
    return await Promise.race([response.arrayBuffer(), stalled]);
  } finally {
    clearTimeout(timer);
  }
}

async function fetchZipball(apiUrl: string): Promise<ArrayBuffer> {
  for (let attempt = 1; ; attempt++) {
    try {
      return await downloadZipball(apiUrl);
    } catch (error) {
      const retryable = (error as { retryable?: boolean }).retryable !== false;
      if (!retryable || attempt >= ZIPBALL_ATTEMPTS) throw error;
      log.warn(`[download-github-folder] ${apiUrl} failed, retrying`, error);
    }
  }
}

export async function downloadGithubRepo(githubUrl: string, destination: string) {
  const { owner, repo, branch, path: subfolderPath } = parseGitHubUrl(githubUrl);
  const apiUrl = `https://api.github.com/repos/${owner}/${repo}/zipball/${branch}`;

  try {
    // Download and extract the zip file
    const tempZipPath = path.join(destination, `${repo}.zip`);
    const zipContent = await fetchZipball(apiUrl);
    await fs.writeFile(tempZipPath, new Uint8Array(zipContent));
    await extract(tempZipPath, {
      dir: destination,
      onSkip: (entry, reason) =>
        log.warn(
          `[download-github-folder] skipped "${entry.fileName}" from ${githubUrl}: ${reason}`,
        ),
    });
    await fs.rm(tempZipPath);

    // Determine the extracted folder name.
    // GitHub zips contain a root folder with a name like owner-repo-commitHash
    const files = await fs.readdir(destination);
    const extractedFolderName = files.length === 1 ? files[0] : null;
    if (!extractedFolderName) {
      throw new Error('Unable to determine the extracted folder name.');
    }

    // If a subfolder path is specified, navigate into it
    const extractedFolderPath = path.join(destination, extractedFolderName);
    const targetFolderPath = subfolderPath
      ? path.join(extractedFolderPath, subfolderPath)
      : extractedFolderPath;

    // Copy contents of the extracted folder (or subfolder) to the destination directory
    const items = await fs.readdir(targetFolderPath);
    for (const item of items) {
      const srcPath = path.join(targetFolderPath, item);
      const destPath = path.join(destination, item);
      await fs.rename(srcPath, destPath);
    }

    // Cleanup the extracted folder root
    await fs.rm(extractedFolderPath, { recursive: true, force: true });
  } catch (error: any) {
    throw new Error(`Error downloading GitHub repository: ${apiUrl} \n ${error.message}`);
  }
}
