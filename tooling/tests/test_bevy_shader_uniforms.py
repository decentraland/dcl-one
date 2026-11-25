"""Embedded shaders bind no uniform WebGL2 refuses."""
from pathlib import Path
import re
import unittest

REPO = Path(__file__).resolve().parents[2]


class ShaderUniformTests(unittest.TestCase):
    # The preview embeds every shader of the assets crate and ships a WebGL2 module, where wgpu
    # refuses a uniform binding smaller than 16 bytes (no BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED) and
    # the module panics at startup. These shaders are the imposters crate's, which the preview does
    # not link; the Explorer builds them for WebGPU only.
    WEBGPU_ONLY = {'floor_bake.wgsl', 'floor_fragment.wgsl'}

    def test_no_shader_binds_a_uniform_smaller_than_16_bytes(self):
        shaders = REPO / 'rendering/bevy-explorer/source/crates/assets/src/assets/shaders'
        small = re.compile(r'var<uniform>\s*\w+\s*:\s*(f32|u32|i32|bool|vec[23]<\w+>)\s*;')
        found = {path.name: match[1] for path in sorted(shaders.rglob('*.wgsl'))
                 for match in small.finditer(path.read_text())}
        self.assertTrue(set(found) >= self.WEBGPU_ONLY, 'the allowance names a shader that no longer needs it')
        self.assertEqual({name: kind for name, kind in found.items() if name not in self.WEBGPU_ONLY}, {})


if __name__ == '__main__':
    unittest.main()
