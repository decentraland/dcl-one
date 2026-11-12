use bevy::{
    asset::io::embedded::EmbeddedAssetRegistry,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use super::*;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
        assets::EmbedAssetsPlugin,
        LandscapeTexturesPlugin,
    ));
    app.finish();
    app.cleanup();
    app
}

/// Updates until no texture is still loading.
fn settle(app: &mut App) -> LandscapeTextures {
    for _ in 0..200 {
        app.update();
        if load_state(app.world().resource::<LandscapeTextures>()) != "loading" {
            break;
        }
    }
    app.world().resource::<LandscapeTextures>().clone()
}

/// Every texture loads from the embedded assets into the handle materials bind, gains a full
/// mip chain, and keeps the colour space and tiling sampler its use needs.
#[test]
fn ground_assets_load_embedded() {
    let mut app = app();
    let textures = settle(&mut app);
    assert_eq!(load_state(&textures), "loaded");
    let images = app.world().resource::<Assets<Image>>();
    for ((handle, kind), (path, _, _)) in textures.all().into_iter().zip(SOURCES) {
        let image = images.get(handle).unwrap();
        assert_eq!(image.size(), UVec2::splat(1024), "{path}");
        assert_eq!(image.texture_descriptor.mip_level_count, 11, "{path}");
        // every level of a 1024 square, four bytes a texel
        let texels: usize = (0..11).map(|level| (1024usize >> level).pow(2)).sum();
        assert_eq!(image.data.as_ref().unwrap().len(), texels * 4, "{path}");
        assert_eq!(
            image.texture_descriptor.format,
            match kind {
                TexelKind::Colour => TextureFormat::Rgba8UnormSrgb,
                TexelKind::Data | TexelKind::Normal => TextureFormat::Rgba8Unorm,
            },
            "{path}"
        );
        let ImageSampler::Descriptor(sampler) = &image.sampler else {
            panic!("{path}: default sampler");
        };
        assert!(
            matches!(
                (
                    sampler.address_mode_u,
                    sampler.address_mode_v,
                    sampler.mag_filter,
                    sampler.min_filter,
                    sampler.mipmap_filter
                ),
                (
                    ImageAddressMode::Repeat,
                    ImageAddressMode::Repeat,
                    ImageFilterMode::Linear,
                    ImageFilterMode::Linear,
                    ImageFilterMode::Linear
                )
            ),
            "{path}: {sampler:?}"
        );
    }
}

/// Before any file has loaded, every bound handle already holds its one-texel stand-in, so the
/// ground and cliffs draw from the first frame.
#[test]
fn stand_ins_are_bound_from_the_start() {
    let app = app();
    let textures = app.world().resource::<LandscapeTextures>();
    assert_eq!(load_state(textures), "loading");
    let images = app.world().resource::<Assets<Image>>();
    for ((handle, _), (_, _, texel)) in textures.all().into_iter().zip(SOURCES) {
        let image = images.get(handle).unwrap();
        assert_eq!(image.size(), UVec2::ONE);
        assert_eq!(image.data.as_deref(), Some(texel.as_slice()));
    }
}

/// A file that does not decode (here an LFS pointer embedded in place of the image, as in a
/// checkout without `git lfs pull`) leaves its flat stand-in bound, in the handle the materials
/// use, and the debug reading says so; the other textures still load.
#[test]
fn a_texture_that_fails_keeps_its_flat_stand_in() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
        assets::EmbedAssetsPlugin,
    ));
    app.world_mut()
        .resource_mut::<EmbeddedAssetRegistry>()
        .insert_asset(
            "pointer.jpg".into(),
            std::path::Path::new("landscape/test/pointer.jpg"),
            b"version https://git-lfs.github.com/spec/v1\noid sha256:0\nsize 1\n".as_slice(),
        );
    let mut paths = SOURCES.map(|(path, _, _)| path);
    paths[1] = "test/pointer.jpg";
    let textures = LandscapeTextures::load(app.world_mut(), paths);
    app.insert_resource(textures)
        .add_plugins(LandscapeTexturesPlugin);
    app.finish();
    app.cleanup();

    let textures = settle(&mut app);
    assert_eq!(load_state(&textures), "failed");
    let ground = crate::terrain::TerrainExtension::new(&textures).ground_colour;
    assert_eq!(ground, textures.ground);
    let images = app.world().resource::<Assets<Image>>();
    let stand_in = images.get(&ground).unwrap();
    assert_eq!(stand_in.size(), UVec2::ONE);
    assert_eq!(stand_in.data.as_deref(), Some(FLAT_GROUND.as_slice()));
    assert_eq!(
        images.get(&textures.splat).unwrap().size(),
        UVec2::splat(1024)
    );
    // resolved once: later frames leave it alone
    let changed = app
        .world()
        .resource::<Assets<Image>>()
        .get(&ground)
        .cloned();
    app.update();
    assert_eq!(
        app.world()
            .resource::<Assets<Image>>()
            .get(&ground)
            .map(|image| image.data.clone()),
        changed.map(|image| image.data)
    );
}

fn image(size: u32, texels: &[[u8; 4]], format: TextureFormat) -> Image {
    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        texels.iter().flatten().copied().collect(),
        format,
        RenderAssetUsages::all(),
    )
}

/// The level after the top one, as RGBA bytes.
fn second_level(image: &Image) -> &[u8] {
    let top = (image.width() * image.height() * 4) as usize;
    let next = ((image.width() / 2) * (image.height() / 2) * 4) as usize;
    &image.data.as_ref().unwrap()[top..top + next]
}

/// A black and white checker averages to half the light, not half the sRGB code value.
#[test]
fn colour_mips_average_in_linear_light() {
    let black = [0, 0, 0, 255];
    let white = [255, 255, 255, 255];
    let mut checker = image(
        2,
        &[black, white, white, black],
        TextureFormat::Rgba8UnormSrgb,
    );
    add_mips(&mut checker, TexelKind::Colour);
    assert_eq!(checker.texture_descriptor.mip_level_count, 2);
    let half = (Srgba::gamma_function_inverse(0.5) * 255.0).round() as u8;
    assert_eq!(second_level(&checker), &[half, half, half, 255]);
}

/// Data such as the splat weights averages as stored.
#[test]
fn data_mips_average_as_stored() {
    let mut splat = image(
        2,
        &[
            [255, 0, 0, 0],
            [0, 255, 0, 0],
            [255, 0, 0, 0],
            [255, 0, 0, 0],
        ],
        TextureFormat::Rgba8Unorm,
    );
    add_mips(&mut splat, TexelKind::Data);
    assert_eq!(second_level(&splat), &[191, 64, 0, 0]);
}

/// Opposed tilts average to a straight-up normal of unit length rather than a short one.
#[test]
fn normal_mips_stay_unit_length() {
    let encode = |v: Vec3| {
        let v = (v.normalize() * 0.5 + Vec3::splat(0.5)) * 255.0;
        [v.x.round() as u8, v.y.round() as u8, v.z.round() as u8, 255]
    };
    let east = encode(Vec3::new(0.6, 0.0, 0.8));
    let west = encode(Vec3::new(-0.6, 0.0, 0.8));
    let mut normals = image(2, &[east, west, west, east], TextureFormat::Rgba8Unorm);
    add_mips(&mut normals, TexelKind::Normal);
    let texel = second_level(&normals);
    let decoded =
        Vec3::new(texel[0] as f32, texel[1] as f32, texel[2] as f32) / 255.0 * 2.0 - Vec3::ONE;
    assert!((decoded.length() - 1.0).abs() < 0.01, "{decoded}");
    assert!(decoded.z > 0.99, "{decoded}");
}

/// A non-square image halves each side until both reach one texel.
#[test]
fn mips_run_down_to_one_texel() {
    let mut wide = Image::new_fill(
        Extent3d {
            width: 8,
            height: 2,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[10, 20, 30, 40],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::all(),
    );
    add_mips(&mut wide, TexelKind::Data);
    assert_eq!(wide.texture_descriptor.mip_level_count, 4);
    // 8x2, 4x1, 2x1, 1x1
    assert_eq!(wide.data.as_ref().unwrap().len(), (16 + 4 + 2 + 1) * 4);
    assert!(wide
        .data
        .as_ref()
        .unwrap()
        .chunks(4)
        .all(|texel| texel == [10, 20, 30, 40]));
}

/// The splat's world size reaches the ground shader as a shader def: the shader reads it and
/// carries no copy of the number.
#[test]
fn splat_size_is_shared_with_the_shader() {
    assert_eq!(
        ground_shader_defs(),
        [ShaderDefVal::UInt(
            "GROUND_SPLAT_METRES".to_owned(),
            GROUND_SPLAT_METRES
        )]
    );
    let shader = include_str!("../landscape_ground.wgsl");
    assert!(shader.contains("#{GROUND_SPLAT_METRES}"));
    assert!(shader.contains("#ifdef GROUND_SPLAT_METRES"));
    assert!(!shader.contains(&GROUND_SPLAT_METRES.to_string()));
}
