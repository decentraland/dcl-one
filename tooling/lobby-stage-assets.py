"""Convert the Unity explorer's lobby stage models and textures into the engine's embedded assets.

Run with Blender 4.2 or later, from the repository root:

    blender --background --factory-startup --python tooling/lobby-stage-assets.py -- \
        --unity <unity-explorer checkout> [--out rendering/bevy-explorer/source/crates/assets/src/assets/lobby_stage]

The Unity checkout is read only. Every model becomes one GLB that holds its own textures and a
glTF metallic-roughness material equivalent to the Unity material. The stage's own textures
(backdrops, floor, blob shadow) are re-encoded next to them.

Geometry stays in the model's own space as Blender imports it: the FBX node rotation and scale
that Unity shows on the model's child are what Blender's importer applies, so the GLB matches the
Unity instance. The one exception is the podium, whose Unity renderer references the mesh
directly, without its node, at the import scale times the stage's 70× scale.
"""

import argparse
import os
import sys

import bpy

# Models: (output name, FBX path under Explorer/Assets, mesh objects to keep, material builder key)
PROPS = [
    ('candle03', 'DCL/LobbyStage/Props/Halloween/Models/Candle03_Halloween.fbx', None, 'candles'),
    ('candle04', 'DCL/LobbyStage/Props/Halloween/Models/Candle04_Halloween.fbx', None, 'candles'),
    ('pumpkin1', 'DCL/LobbyStage/Props/Halloween/Models/Pumpkin1_Halloween.fbx', None, 'pumpkin1'),
    ('pumpkin2', 'DCL/LobbyStage/Props/Halloween/Models/Pumpkin2_Halloween.fbx', None, 'pumpkin2'),
    ('pumpkin3', 'DCL/LobbyStage/Props/Halloween/Models/Pumpkin3_Halloween.fbx', None, 'pumpkin3'),
    ('pumpkin4', 'DCL/LobbyStage/Props/Halloween/Models/Pumpkin4_Halloween.fbx', None, 'pumpkin4'),
    ('rock01', 'DCL/Landscape/Assets/Rocks/Rock01/Rock01.fbx', 'Rock01_LOD0', 'rock01'),
    ('rock02', 'DCL/Landscape/Assets/Rocks/Rock02/Rock02.fbx', 'Rocks02_LOD0', 'rock02'),
    ('rock03', 'DCL/Landscape/Assets/Rocks/Rock03/Rock03.fbx', 'Rock03_LOD0', 'rock03'),
]
PODIUM = ('podium', 'DCL/Character/CharacterPreview/Assets/PreviewPlatform/platform_v2.fbx')
# The podium's import scale (0.01) times its scale on the stage (70).
PODIUM_SCALE = 0.7

HALLOWEEN = 'DCL/LobbyStage/Props/Halloween/Materials/'
ROCKS = 'DCL/Landscape/Assets/Textures/'
PLATFORM = 'DCL/Character/CharacterPreview/Assets/PreviewPlatform/'
GPUI = 'DCL/Landscape/Assets/GPUI/Rocks/'

# Material inputs: the Unity material file (smoothness, metallic and the HDR linear emission colour
# are read from it) and its textures. The GLB keeps the emission's hue as the emissive factor and
# its peak as KHR_materials_emissive_strength.
MATERIALS = {
    'candles': dict(mat=HALLOWEEN + 'Candles.mat', base=HALLOWEEN + 'Candles_baseColor.png',
                    emission_map=HALLOWEEN + 'Candles_emissive.png'),
    'pumpkin1': dict(mat=HALLOWEEN + 'Pumpkin1.mat', base=HALLOWEEN + 'Pumpkin1_baseColor.jpg',
                     emission_map=HALLOWEEN + 'Pumpkin1_greenMask.png'),
    'pumpkin2': dict(mat=HALLOWEEN + 'Pumpkin2.mat', base=HALLOWEEN + 'Pumpkin2_baseColor.jpg',
                     emission_map=HALLOWEEN + 'Pumpkin2_greenMask.png'),
    'pumpkin3': dict(mat=HALLOWEEN + 'Pumpkin3.mat', base=HALLOWEEN + 'Pumpkin3_baseColor.jpg',
                     emission_map=HALLOWEEN + 'Pumpkin3_greenMask.png'),
    'pumpkin4': dict(mat=HALLOWEEN + 'Pumpkin4.mat', base=HALLOWEEN + 'Pumpkin4_baseColor.jpg',
                     emission_map=HALLOWEEN + 'Pumpkin4_greenMask.png'),
    'rock01': dict(mat=GPUI + 'Rock01/Rock01_GPUIPro.mat', base=ROCKS + 'stylizedRock01_baseColor.png',
                   normal=ROCKS + 'stylizedRock01_normal.png'),
    'rock02': dict(mat=GPUI + 'Rock02/Rock02_GPUIPro.mat', base=ROCKS + 'stylizedRock02_baseColor.png',
                   normal=ROCKS + 'stylizedRock02_normal.png'),
    'rock03': dict(mat=GPUI + 'Rock03/Rock_03_GPUIPro.mat', base=ROCKS + 'stylizedRock03_baseColor.png',
                   normal=ROCKS + 'stylizedRock03_normal.png'),
    'podium': dict(mat=PLATFORM + 'Platform.mat', base=PLATFORM + 'BaseColor.png',
                   normal=PLATFORM + 'DefaultMaterial_Normal.png',
                   metallic_map=PLATFORM + 'DefaultMaterial_Metallic.png',
                   emission_map=PLATFORM + 'DefaultMaterial_Emission.png'),
}

# Stage textures: (output file, source, longest side cap, format)
TEXTURES = [
    ('halloween_backdrop.jpg', 'DCL/Lobby/Textures/Halloween26_Background.png', 2048, 'JPEG'),
    ('game_world_backdrop.jpg', 'DCL/Lobby/Textures/LobbyBackground.png', 2048, 'JPEG'),
    ('ground.jpg', 'DCL/Lobby/Textures/GroundTexture.png', 2048, 'JPEG'),
]
# The blob shadow reads the radial glow's alpha times its (sRGB-decoded) red channel; that product
# is baked into one linear grey channel at Unity's 2048 import cap.
BLOB_SOURCE = 'Textures/Common/RadialGlow.png'
BLOB_OUT = 'blob_shadow.png'
BLOB_SIZE = 2048


def parse_args():
    argv = sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else []
    parser = argparse.ArgumentParser()
    parser.add_argument('--unity', required=True, help='unity-explorer checkout')
    parser.add_argument('--out', default='rendering/bevy-explorer/source/crates/assets/src/assets/lobby_stage')
    return parser.parse_args(argv)


def unity_material(path):
    """Smoothness, metallic and the emission colour (None when black or off) of a Unity material."""
    floats, emission, keywords = {}, None, ''
    with open(path, encoding='utf-8') as f:
        for line in f:
            line = line.strip()
            if line.startswith('m_ValidKeywords') or line.startswith('- _EMISSION'):
                keywords += line
            if line.startswith('- _Smoothness:') or line.startswith('- _Metallic:'):
                key, value = line[2:].split(': ')
                floats[key] = float(value)
            if line.startswith('- _EmissionColor:'):
                values = dict(part.split(': ') for part in line.split('{')[1].rstrip('}').split(', '))
                emission = tuple(float(values[k]) for k in ('r', 'g', 'b'))
    if emission is not None and (not any(emission) or '_EMISSION' not in keywords):
        emission = None
    return floats['_Smoothness'], floats.get('_Metallic', 0.0), emission


def load_image(root, rel, non_color=False):
    image = bpy.data.images.load(os.path.join(root, rel), check_existing=True)
    if non_color:
        image.colorspace_settings.name = 'Non-Color'
    return image


def build_material(name, spec, root):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    bsdf = nodes['Principled BSDF']

    base = nodes.new('ShaderNodeTexImage')
    base.image = load_image(root, spec['base'])
    links.new(base.outputs['Color'], bsdf.inputs['Base Color'])

    smoothness, metallic, emission = unity_material(os.path.join(root, spec['mat']))
    bsdf.inputs['Roughness'].default_value = 1.0 - smoothness
    bsdf.inputs['Metallic'].default_value = metallic

    if 'metallic_map' in spec:
        # Unity reads metallic from the map's red channel and smoothness from its alpha times
        # _Smoothness; the map has no alpha, so roughness stays 1 - _Smoothness.
        metal = nodes.new('ShaderNodeTexImage')
        metal.image = load_image(root, spec['metallic_map'], non_color=True)
        split = nodes.new('ShaderNodeSeparateColor')
        links.new(metal.outputs['Color'], split.inputs['Color'])
        links.new(split.outputs['Red'], bsdf.inputs['Metallic'])

    if 'normal' in spec:
        normal_tex = nodes.new('ShaderNodeTexImage')
        normal_tex.image = load_image(root, spec['normal'], non_color=True)
        normal_map = nodes.new('ShaderNodeNormalMap')
        links.new(normal_tex.outputs['Color'], normal_map.inputs['Color'])
        links.new(normal_map.outputs['Normal'], bsdf.inputs['Normal'])

    if emission is not None and spec.get('emission_map'):
        peak = max(emission)
        emit = nodes.new('ShaderNodeTexImage')
        emit.image = load_image(root, spec['emission_map'])
        tint = nodes.new('ShaderNodeMix')
        tint.data_type = 'RGBA'
        tint.blend_type = 'MULTIPLY'
        tint.inputs['Factor'].default_value = 1.0
        links.new(emit.outputs['Color'], tint.inputs['A'])
        tint.inputs['B'].default_value = (*(c / peak for c in emission), 1.0)
        links.new(tint.outputs['Result'], bsdf.inputs['Emission Color'])
        bsdf.inputs['Emission Strength'].default_value = peak
    return mat


def export_glb(path, objects):
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects:
        obj.select_set(True)
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format='GLB',
        use_selection=True,
        export_yup=True,
        export_apply=True,
        export_texcoords=True,
        export_normals=True,
        export_tangents=True,
        export_materials='EXPORT',
        export_image_format='AUTO',
        export_animations=False,
        export_skins=False,
        export_morph=False,
        export_cameras=False,
        export_lights=False,
    )


def import_model(root, rel):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(filepath=os.path.join(root, rel))
    return [o for o in bpy.context.scene.objects if o.type == 'MESH']


def convert_prop(root, out, name, rel, keep, material_key):
    meshes = import_model(root, rel)
    if keep is not None:
        for obj in [o for o in meshes if o.name != keep]:
            bpy.data.objects.remove(obj)
        meshes = [o for o in bpy.context.scene.objects if o.type == 'MESH']
    if len(meshes) != 1:
        raise RuntimeError(f'{rel}: expected one mesh, found {[o.name for o in meshes]}')
    mesh = meshes[0]
    mesh.data.materials.clear()
    mesh.data.materials.append(build_material(name, MATERIALS[material_key], root))
    export_glb(os.path.join(out, name + '.glb'), meshes)


def convert_podium(root, out):
    name, rel = PODIUM
    meshes = import_model(root, rel)
    if len(meshes) != 1:
        raise RuntimeError(f'{rel}: expected one mesh, found {[o.name for o in meshes]}')
    mesh = meshes[0]
    # Unity draws the mesh without its FBX node: only the mesh data, at the stage's scale.
    mesh.parent = None
    mesh.location = (0.0, 0.0, 0.0)
    mesh.rotation_euler = (0.0, 0.0, 0.0)
    mesh.scale = (PODIUM_SCALE, PODIUM_SCALE, PODIUM_SCALE)
    mesh.data.materials.clear()
    mesh.data.materials.append(build_material(name, MATERIALS['podium'], root))
    export_glb(os.path.join(out, name + '.glb'), [mesh])


def convert_textures(root, out):
    for out_name, rel, cap, fmt in TEXTURES:
        image = bpy.data.images.load(os.path.join(root, rel))
        width, height = image.size
        scale = min(1.0, cap / max(width, height))
        if scale < 1.0:
            image.scale(round(width * scale), round(height * scale))
        image.filepath_raw = os.path.join(out, out_name)
        image.file_format = fmt
        scene = bpy.context.scene
        scene.render.image_settings.file_format = fmt
        scene.render.image_settings.quality = 92
        scene.render.image_settings.color_mode = 'RGB'
        image.save_render(os.path.join(out, out_name), scene=scene)

    glow = bpy.data.images.load(os.path.join(root, BLOB_SOURCE))
    if max(glow.size) > BLOB_SIZE:
        glow.scale(BLOB_SIZE, BLOB_SIZE)
    width, height = glow.size
    px = list(glow.pixels)
    shape = bpy.data.images.new('blob', width, height, alpha=False)
    out_px = [0.0] * (width * height * 4)
    for i in range(width * height):
        r = px[i * 4]
        r = r / 12.92 if r <= 0.04045 else ((r + 0.055) / 1.055) ** 2.4
        v = r * px[i * 4 + 3]
        out_px[i * 4] = out_px[i * 4 + 1] = out_px[i * 4 + 2] = v
        out_px[i * 4 + 3] = 1.0
    shape.pixels = out_px
    scene = bpy.context.scene
    scene.render.image_settings.file_format = 'PNG'
    scene.render.image_settings.color_mode = 'BW'
    scene.render.image_settings.color_depth = '8'
    shape.save_render(os.path.join(out, BLOB_OUT), scene=scene)


def main():
    args = parse_args()
    root = os.path.join(os.path.abspath(args.unity), 'Explorer', 'Assets')
    out = os.path.abspath(args.out)
    os.makedirs(out, exist_ok=True)
    for name, rel, keep, material_key in PROPS:
        convert_prop(root, out, name, rel, keep, material_key)
    convert_podium(root, out)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    convert_textures(root, out)


main()
