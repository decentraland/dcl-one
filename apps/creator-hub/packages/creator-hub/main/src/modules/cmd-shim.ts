import fs from 'node:fs/promises';
import path from 'node:path';

const CMD_HEAD =
  '@ECHO off\r\n' +
  'GOTO start\r\n' +
  ':find_dp0\r\n' +
  'SET dp0=%~dp0\r\n' +
  'EXIT /b\r\n' +
  ':start\r\n' +
  'SETLOCAL\r\n' +
  'CALL :find_dp0\r\n';

const SH_HEAD =
  '#!/bin/sh\n' +
  'basedir=$(dirname "$(echo "$0" | sed -e \'s,\\\\,/,g\')")\n' +
  '\n' +
  'case `uname` in\n' +
  '    *CYGWIN*|*MINGW*|*MSYS*)\n' +
  '        if command -v cygpath > /dev/null 2>&1; then\n' +
  '            basedir=`cygpath -w "$basedir"`\n' +
  '        fi\n' +
  '    ;;\n' +
  'esac\n' +
  '\n';

const PWSH_HEAD =
  '#!/usr/bin/env pwsh\n' +
  '$basedir=Split-Path $MyInvocation.MyCommand.Definition -Parent\n' +
  '\n' +
  '$exe=""\n' +
  'if ($PSVersionTable.PSVersion -lt "6.0" -or $IsWindows) {\n' +
  '  # Fix case when both the Windows and Linux builds of Node\n' +
  '  # are installed in the same directory\n' +
  '  $exe=".exe"\n' +
  '}\n';

/**
 * Makes the launchers that let Windows shells start the program `from` under the name `to`:
 * `to.cmd` for cmd, `to.ps1` for PowerShell and `to` for a POSIX shell. Each one starts
 * `from` by its path relative to the launcher (or its absolute path when the two are on different
 * volumes, where no relative path exists) and passes all the arguments on. The files are
 * the ones that the npm `cmd-shim` package writes for a program that is not a script.
 */
export async function writeCmdShim(from: string, to: string): Promise<void> {
  await fs.stat(from);
  const launchers = [to, `${to}.cmd`, `${to}.ps1`];
  await Promise.all(launchers.map(file => fs.unlink(file).catch(() => {})));
  await fs.mkdir(path.dirname(to), { recursive: true });

  const relative = path.relative(path.dirname(to), from);
  // Roaming AppData on another drive or a network share: `relative` is then absolute.
  const absolute = path.isAbsolute(relative);
  const windowsTarget = absolute
    ? `"${relative.split('/').join('\\')}"`
    : `"%dp0%\\${relative.split('/').join('\\')}"`;
  const posixTarget = absolute
    ? `"${relative.split('\\').join('/')}"`
    : `"$basedir/${relative.split('\\').join('/')}"`;

  await Promise.all([
    fs.writeFile(to, `${SH_HEAD}exec ${posixTarget}   "$@"\n`),
    fs.writeFile(`${to}.cmd`, `${CMD_HEAD}${windowsTarget}   %*\r\n`),
    fs.writeFile(
      `${to}.ps1`,
      `${PWSH_HEAD}# Support pipeline input\n` +
        'if ($MyInvocation.ExpectingInput) {\n' +
        `  $input | & ${posixTarget}   $args\n` +
        '} else {\n' +
        `  & ${posixTarget}   $args\n` +
        '}\n' +
        'exit $LASTEXITCODE\n',
    ),
  ]);
  await Promise.all(launchers.map(file => fs.chmod(file, 0o755)));
}
