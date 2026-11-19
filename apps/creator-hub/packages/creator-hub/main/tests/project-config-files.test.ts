import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import {
  addDependencyOverrides,
  ensureEditorFilesNotDeployed,
  ensureGitignored,
  upsertMcpServer,
} from '../src/modules/project-config-files';
import { DEFAULT_DCLIGNORE } from '../src/modules/optimizer/dclignore';

let dir: string;
const gitignore = () => fs.readFileSync(path.join(dir, '.gitignore'), 'utf8');

beforeEach(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ch-project-config-'));
});
afterEach(() => {
  fs.rmSync(dir, { recursive: true, force: true });
});

describe('ensureGitignored', () => {
  describe('when an existing rule already covers the entry', () => {
    it.each(['skills', '/skills', 'skills/'])('should leave "%s" as the only rule', rule => {
      fs.writeFileSync(path.join(dir, '.gitignore'), `${rule}\r\n`);

      ensureGitignored(dir, 'skills');

      expect(gitignore()).toBe(`${rule}\r\n`);
    });

    it('should accept a broader rule the caller names', () => {
      fs.writeFileSync(path.join(dir, '.gitignore'), '.cursor/\n');

      ensureGitignored(dir, '.cursor/mcp.json', { coveredBy: ['.cursor', '.cursor/'] });

      expect(gitignore()).toBe('.cursor/\n');
    });
  });

  describe('when nothing covers the entry', () => {
    it('should append it on its own line, under the comment when one is given', () => {
      fs.writeFileSync(path.join(dir, '.gitignore'), 'node_modules');

      ensureGitignored(dir, '.mcp.json', { comment: 'local token' });

      expect(gitignore()).toBe('node_modules\n\n# local token\n.mcp.json\n');
    });
  });
});

describe('upsertMcpServer', () => {
  const file = () => path.join(dir, 'sub', 'mcp.json');
  const read = () => JSON.parse(fs.readFileSync(file(), 'utf8'));

  it('should add ours beside the user’s servers', () => {
    fs.mkdirSync(path.dirname(file()));
    fs.writeFileSync(file(), JSON.stringify({ mcpServers: { mine: { command: 'x' } } }));

    upsertMcpServer(file(), 'creator-hub', { url: 'u' });

    expect(read().mcpServers).toEqual({ mine: { command: 'x' }, 'creator-hub': { url: 'u' } });
  });

  it('should create the file with the given mode', () => {
    upsertMcpServer(file(), 'creator-hub', { url: 'u' }, { mode: 0o600 });

    expect(fs.statSync(file()).mode & 0o777).toBe(0o600);
  });
});

describe('ensureEditorFilesNotDeployed', () => {
  const dclignore = () => fs.readFileSync(path.join(dir, '.dclignore'), 'utf8');
  const block = '# Editor and agent files, never part of the deployed scene\n';

  describe('when the scene has a .dclignore that names neither editor file', () => {
    it('should append both under the comment, keeping the creator’s lines', () => {
      fs.writeFileSync(path.join(dir, '.dclignore'), '.*\n*.md');

      ensureEditorFilesNotDeployed(dir);

      expect(dclignore()).toBe(`.*\n*.md\n${block}cursor.config\ncursorignore\n`);
    });
  });

  describe('when the .dclignore already names one of them', () => {
    it.each(['cursor.config', '/cursor.config', '!cursor.config'])(
      'should leave "%s" and add only the other',
      line => {
        fs.writeFileSync(path.join(dir, '.dclignore'), `${line}\r\n`);

        ensureEditorFilesNotDeployed(dir);

        expect(dclignore()).toBe(`${line}\r\n${block}cursorignore\n`);
      },
    );
  });

  describe('when the .dclignore already names both', () => {
    it('should not touch the file', () => {
      fs.writeFileSync(path.join(dir, '.dclignore'), 'cursorignore\ncursor.config\n');

      ensureEditorFilesNotDeployed(dir);

      expect(dclignore()).toBe('cursorignore\ncursor.config\n');
    });
  });

  describe('when the scene has no .dclignore', () => {
    it('should create one with the deploy defaults and the editor files', () => {
      ensureEditorFilesNotDeployed(dir);

      expect(dclignore()).toBe(
        `${DEFAULT_DCLIGNORE.join('\n')}\n${block}cursor.config\ncursorignore\n`,
      );
    });
  });
});

describe('addDependencyOverrides', () => {
  const pkgFile = () => path.join(dir, 'package.json');
  const read = () => JSON.parse(fs.readFileSync(pkgFile(), 'utf8'));
  const template = { name: 'dcl-project', devDependencies: { '@dcl/sdk': 'latest' } };

  describe('when the template sets no overrides', () => {
    beforeEach(() => {
      fs.writeFileSync(pkgFile(), JSON.stringify(template, null, 2));
      addDependencyOverrides(dir);
    });

    it('should pin the vulnerable protobufjs releases to a fixed one, keeping the rest', () => {
      expect(read()).toEqual({ ...template, overrides: { 'protobufjs@<7.6.3': '^7.6.6' } });
    });

    it('should end the file with a newline', () => {
      expect(fs.readFileSync(pkgFile(), 'utf8').endsWith('}\n')).toBe(true);
    });
  });

  describe('when the template already overrides protobufjs', () => {
    it.each([{ protobufjs: '7.6.6' }, { 'protobufjs@7': '^7.6.7' }])(
      'should keep %j as it is',
      overrides => {
        fs.writeFileSync(pkgFile(), JSON.stringify({ ...template, overrides }));

        addDependencyOverrides(dir);

        expect(read().overrides).toEqual(overrides);
      },
    );
  });

  describe('when the template overrides another package', () => {
    it('should add ours beside it', () => {
      fs.writeFileSync(
        pkgFile(),
        JSON.stringify({ ...template, overrides: { picomatch: '2.3.2' } }),
      );

      addDependencyOverrides(dir);

      expect(read().overrides).toEqual({ picomatch: '2.3.2', 'protobufjs@<7.6.3': '^7.6.6' });
    });
  });

  describe('when the package.json is missing or not JSON', () => {
    it('should not create one', () => {
      addDependencyOverrides(dir);

      expect(fs.existsSync(pkgFile())).toBe(false);
    });

    it('should leave an unreadable one as it is', () => {
      fs.writeFileSync(pkgFile(), '{ not json');

      addDependencyOverrides(dir);

      expect(fs.readFileSync(pkgFile(), 'utf8')).toBe('{ not json');
    });
  });
});
