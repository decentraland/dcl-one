"""Converts unity-explorer's landscape meshes and textures for the Explorer's embedded assets.

Run with Blender's bundled Python, headless:

    blender --background --factory-startup --python-exit-code 1 --python fbx_to_glb.py -- \
        mesh --fbx IN.fbx --object NAME --material NAME [--material NAME ...] --out OUT.glb

    blender --background --factory-startup --python-exit-code 1 --python fbx_to_glb.py -- \
        texture --image IN.png --size 512 [--alpha-from-gray] --out OUT.png|OUT.jpg

`mesh` keeps the one object `--object` names (the first level of detail), bakes its transform,
turns it half a turn about the vertical axis and writes a GLB with positions, normals, tangents
and the first UV set. Its material slots are renamed in order to the `--material` names and carry
no textures: the engine builds the materials from those names. Unity's importer mirrors x where
Blender's does not; with the engine's z = -Unity z, the half turn puts each vertex where Unity has
it.

`texture` copies an image's stored values unchanged (no colour conversion), optionally replaces
alpha with the mean of red, green and blue, scales it to `--size` square and writes PNG with
alpha or JPEG without, by the output's extension.
"""

import argparse
import math
import sys

import bpy
import mathutils

JPEG_QUALITY = 90


def arguments():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    parser = argparse.ArgumentParser(prog="fbx_to_glb.py")
    commands = parser.add_subparsers(dest="command", required=True)

    mesh = commands.add_parser("mesh")
    mesh.add_argument("--fbx", required=True)
    mesh.add_argument("--object", required=True)
    mesh.add_argument("--material", action="append", required=True)
    mesh.add_argument("--out", required=True)

    texture = commands.add_parser("texture")
    texture.add_argument("--image", required=True)
    texture.add_argument("--size", type=int, required=True)
    texture.add_argument("--alpha-from-gray", action="store_true")
    texture.add_argument("--out", required=True)
    return parser.parse_args(argv)


def convert_mesh(args):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(filepath=args.fbx)
    keep = bpy.data.objects.get(args.object)
    if keep is None or keep.type != "MESH":
        names = ", ".join(o.name for o in bpy.data.objects)
        raise SystemExit(f"no mesh object {args.object!r} in {args.fbx} (objects: {names})")
    for other in [o for o in bpy.data.objects if o != keep]:
        bpy.data.objects.remove(other, do_unlink=True)

    mesh = keep.data
    keep.parent = None
    mesh.transform(keep.matrix_world)
    mesh.transform(mathutils.Matrix.Rotation(math.pi, 4, "Z"))
    keep.matrix_world = mathutils.Matrix.Identity(4)
    mesh.update()

    for attribute in list(mesh.color_attributes):
        mesh.color_attributes.remove(attribute)
    for layer in list(mesh.uv_layers)[1:]:
        mesh.uv_layers.remove(layer)

    if len(args.material) != len(keep.material_slots):
        raise SystemExit(
            f"{args.object} has {len(keep.material_slots)} material slots, "
            f"{len(args.material)} names given"
        )
    for slot, name in zip(keep.material_slots, args.material):
        material = bpy.data.materials.new(name)
        slot.material = material

    keep.select_set(True)
    bpy.context.view_layer.objects.active = keep
    bpy.ops.export_scene.gltf(
        filepath=args.out,
        export_format="GLB",
        use_selection=True,
        export_yup=True,
        export_apply=True,
        export_texcoords=True,
        export_normals=True,
        export_tangents=True,
        export_vertex_color="NONE",
        export_materials="EXPORT",
        export_image_format="NONE",
        export_animations=False,
        export_skins=False,
        export_morph=False,
        export_cameras=False,
        export_lights=False,
        export_extras=False,
    )
    print(f"{args.out}: {len(mesh.vertices)} vertices, {len(mesh.polygons)} faces")


def convert_texture(args):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    source = bpy.data.images.load(args.image)
    # read the stored values as they are, whatever colour space the file claims
    source.colorspace_settings.name = "Non-Color"
    width, height = source.size
    pixels = list(source.pixels)
    if args.alpha_from_gray:
        for i in range(0, len(pixels), 4):
            pixels[i + 3] = (pixels[i] + pixels[i + 1] + pixels[i + 2]) / 3.0

    png = args.out.lower().endswith(".png")
    target = bpy.data.images.new("converted", width, height, alpha=png, float_buffer=False)
    target.colorspace_settings.name = "Non-Color"
    target.pixels = pixels
    if (width, height) != (args.size, args.size):
        target.scale(args.size, args.size)
    target.filepath_raw = args.out
    target.file_format = "PNG" if png else "JPEG"
    if png:
        target.alpha_mode = "STRAIGHT"
        bpy.context.scene.render.image_settings.color_mode = "RGBA"
        target.save(filepath=args.out)
    else:
        target.save(filepath=args.out, quality=JPEG_QUALITY)
    print(f"{args.out}: {args.size}x{args.size}")


def main():
    args = arguments()
    if args.command == "mesh":
        convert_mesh(args)
    else:
        convert_texture(args)


main()
