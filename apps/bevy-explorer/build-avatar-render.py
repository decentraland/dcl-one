#!/usr/bin/env python3
"""Build the engine's headless avatar renderer and leave it, wrapped, at dist/avatar_render.

The node's profile images (social::profile_images, PROFILE_IMAGES_AVATAR_RENDER_BIN) run it to render
the face and body of profiles whose snapshot files the node does not hold. The node runs with a clean
environment, so the wrapper carries the library path and Vulkan settings of the shell this ran in: the
GPU driver's, on NixOS under /run/opengl-driver as well.
"""
import os
import shlex
import stat
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parents[1] / 'rendering/bevy-explorer/source'
DIST = HERE / 'dist'
# The batch tools' features: media decoding, the scene inspector and the social service client, no HUD.
FEATURES = 'ffmpeg,inspect,social'
CARRIED = ('LD_LIBRARY_PATH', 'VK_ICD_FILENAMES', 'VK_DRIVER_FILES', 'XDG_DATA_DIRS', 'WGPU_BACKEND')
DRIVER_LIBS = Path('/run/opengl-driver/lib')
# where NixOS keeps the Vulkan ICD manifests the loader looks up through XDG_DATA_DIRS
DRIVER_SHARE = Path('/run/opengl-driver/share')


def environment():
    env = {name: os.environ[name] for name in CARRIED if os.environ.get(name)}
    for name, directory in (('LD_LIBRARY_PATH', DRIVER_LIBS), ('XDG_DATA_DIRS', DRIVER_SHARE)):
        if directory.is_dir():
            env[name] = ':'.join(p for p in (env.get(name, ''), str(directory)) if p)
    env.setdefault('WGPU_BACKEND', 'vulkan')
    return env


def main():
    subprocess.run(['cargo', 'build', '--release', '--locked', '--no-default-features', f'--features={FEATURES}',
                    '--bin', 'avatar_render'], cwd=SOURCE, check=True)
    binary = SOURCE / 'target/release/avatar_render'
    DIST.mkdir(parents=True, exist_ok=True)
    wrapper = DIST / 'avatar_render'
    exports = ''.join(f'export {name}={shlex.quote(value)}\n' for name, value in environment().items())
    wrapper.write_text(f'#!/usr/bin/env bash\n{exports}exec {shlex.quote(str(binary))} "$@"\n')
    wrapper.chmod(wrapper.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    print(wrapper)


if __name__ == '__main__':
    main()
