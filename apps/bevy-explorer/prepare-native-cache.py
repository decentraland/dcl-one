#!/usr/bin/env python3
"""Keep only the pinned native toolchain on a disposable GitHub-hosted runner."""
import os
import re
import subprocess


def output(*args):
    return subprocess.check_output(args, text=True).strip()


def prepare():
    # rust-cache hashes every installed compiler, including unused runner defaults.
    # Never remove a developer's toolchains or mutate a shared self-hosted runner.
    if os.environ.get('RUNNER_ENVIRONMENT') != 'github-hosted':
        raise RuntimeError('Toolchain cleanup requires a disposable GitHub-hosted runner')
    active = output('rustup', 'show', 'active-toolchain').split()[0]
    compiler = output('rustc', '-vV')
    fields = dict(line.split(': ', 1) for line in compiler.splitlines() if ': ' in line)
    release = fields.get('release', '')
    if not re.fullmatch(r'\d+\.\d+\.\d+', release) or active != f"{release}-{fields.get('host', '')}":
        raise RuntimeError('Install and select a numbered native Rust toolchain before cache preparation')
    installed = output('rustup', 'toolchain', 'list', '--quiet').splitlines()
    if active not in installed:
        raise RuntimeError('Active toolchain is absent from the installed toolchain list')
    for toolchain in installed:
        if toolchain != active:
            print(f'Removing unused toolchain: {toolchain}', flush=True)
            subprocess.run(['rustup', 'toolchain', 'uninstall', toolchain], check=True)
    # The active compiler, its components and cross-compilation targets stay intact.
    print(f'Native cache toolchain: {active}\n{compiler}', flush=True)


if __name__ == '__main__':
    prepare()
