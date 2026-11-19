import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { writeCmdShim } from '../src/modules/cmd-shim';

describe('writeCmdShim', () => {
  let folder: string;
  let program: string;
  let shim: string;

  beforeEach(() => {
    folder = fs.mkdtempSync(path.join(os.tmpdir(), 'cmd-shim-'));
    program = path.join(folder, 'app', 'Creator Hub.exe');
    shim = path.join(folder, 'app', 'resources', 'unpacked', 'node');
    fs.mkdirSync(path.dirname(program), { recursive: true });
    fs.writeFileSync(program, 'MZ');
  });

  afterEach(() => {
    fs.rmSync(folder, { recursive: true, force: true });
  });

  describe('when the program is there', () => {
    beforeEach(async () => {
      await writeCmdShim(program, shim);
    });

    it('should write a cmd launcher that starts the program by its relative path', () => {
      expect(fs.readFileSync(`${shim}.cmd`, 'utf8')).toBe(
        '@ECHO off\r\n' +
          'GOTO start\r\n' +
          ':find_dp0\r\n' +
          'SET dp0=%~dp0\r\n' +
          'EXIT /b\r\n' +
          ':start\r\n' +
          'SETLOCAL\r\n' +
          'CALL :find_dp0\r\n' +
          '"%dp0%\\..\\..\\Creator Hub.exe"   %*\r\n',
      );
    });

    it('should write a shell launcher', () => {
      const script = fs.readFileSync(shim, 'utf8');

      expect(script.startsWith('#!/bin/sh\n')).toBe(true);
      expect(script.endsWith('\nexec "$basedir/../../Creator Hub.exe"   "$@"\n')).toBe(true);
    });

    it('should write a PowerShell launcher', () => {
      const script = fs.readFileSync(`${shim}.ps1`, 'utf8');

      expect(script.startsWith('#!/usr/bin/env pwsh\n')).toBe(true);
      expect(script).toContain('  $input | & "$basedir/../../Creator Hub.exe"   $args\n');
      expect(script).toContain('  & "$basedir/../../Creator Hub.exe"   $args\n');
      expect(script.endsWith('exit $LASTEXITCODE\n')).toBe(true);
    });

    it('should replace the launchers that are there', async () => {
      fs.writeFileSync(`${shim}.cmd`, 'old');

      await writeCmdShim(program, shim);

      expect(fs.readFileSync(`${shim}.cmd`, 'utf8')).toContain('Creator Hub.exe');
    });
  });

  describe('when the launcher and the program are on different volumes', () => {
    beforeEach(async () => {
      // path.relative has no relative answer across Windows drives and returns the target as is.
      vi.spyOn(path, 'relative').mockReturnValue('/other-volume/Creator Hub.exe');
      await writeCmdShim(program, shim);
    });

    afterEach(() => {
      vi.restoreAllMocks();
    });

    it('should start the program by its absolute path', () => {
      const cmd = fs.readFileSync(`${shim}.cmd`, 'utf8');
      expect(cmd).toContain('"\\other-volume\\Creator Hub.exe"');
      expect(cmd).not.toContain('%dp0%\\\\other-volume');
      expect(fs.readFileSync(shim, 'utf8')).toContain('exec "/other-volume/Creator Hub.exe"');
    });
  });

  describe('when the program is not there', () => {
    it('should fail and write no launcher', async () => {
      await expect(writeCmdShim(path.join(folder, 'missing.exe'), shim)).rejects.toThrow('ENOENT');

      expect(fs.existsSync(`${shim}.cmd`)).toBe(false);
    });
  });
});
