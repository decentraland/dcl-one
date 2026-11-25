//! A frame read back from the GPU, scaled and encoded. `/screenshot`, `/texture_camera_screenshot`
//! and the camera reel's photo all go through here, so a frame is read, scaled and encoded by one
//! rule.

use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};
use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, ImageFormat};
use tokio::sync::oneshot;

/// The JPEG quality of a captured frame and of a saved photo.
pub const JPEG_QUALITY: u8 = 75;

/// How a frame is scaled before it is encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    /// As it was read back.
    Full,
    /// Down to fit this box, keeping its shape. A frame that already fits is left alone: a frame
    /// is never scaled up to a box.
    Within { width: u32, height: u32 },
    /// To exactly this size, up or down.
    Exact { width: u32, height: u32 },
}

impl Scale {
    /// At most `width` wide, whatever the height.
    pub fn max_width(width: u32) -> Self {
        Self::Within {
            width,
            height: u32::MAX,
        }
    }

    pub fn apply(self, image: DynamicImage) -> DynamicImage {
        match self {
            Self::Within { width, height } if image.width() > width || image.height() > height => {
                image.resize(width, height, FilterType::Triangle)
            }
            Self::Exact { width, height } if (image.width(), image.height()) != (width, height) => {
                image.resize_exact(width, height, FilterType::Triangle)
            }
            _ => image,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Png,
    /// JPEG at this quality; alpha is dropped.
    Jpeg(u8),
}

/// Ask for `shot` at the end of the frame. The receiver yields the frame once it is read back,
/// and closes if it never is.
pub fn read_back(commands: &mut Commands, shot: Screenshot) -> oneshot::Receiver<Image> {
    let (sender, receiver) = oneshot::channel();
    let mut sender = Some(sender);
    commands
        .spawn(shot)
        .observe(move |mut trigger: Trigger<ScreenshotCaptured>| {
            if let Some(sender) = sender.take() {
                let _ = sender.send(std::mem::take(&mut trigger.0));
            }
        });
    receiver
}

/// The pixels of a frame that was read back.
pub fn pixels(frame: Image) -> Result<DynamicImage, String> {
    frame
        .try_into_dynamic()
        .map_err(|e| format!("the frame cannot be read: {e:?}"))
}

pub fn encode(image: &DynamicImage, encoding: Encoding) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    match encoding {
        Encoding::Png => image
            .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .map_err(|e| format!("PNG encode failed: {e}"))?,
        Encoding::Jpeg(quality) => {
            let mut encoder = JpegEncoder::new_with_quality(&mut bytes, quality);
            match image.as_rgb8() {
                Some(rgb) => encoder.encode_image(rgb),
                None => encoder.encode_image(&image.to_rgb8()),
            }
            .map_err(|e| format!("JPEG encode failed: {e}"))?
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            width,
            height,
            image::Rgba([255, 0, 0, 255]),
        ))
    }

    fn size(image: &DynamicImage) -> (u32, u32) {
        (image.width(), image.height())
    }

    #[test]
    fn a_box_scales_a_frame_down_and_never_up() {
        assert_eq!(size(&Scale::max_width(64).apply(image(320, 160))), (64, 32));
        assert_eq!(size(&Scale::max_width(64).apply(image(48, 24))), (48, 24));
        // the taller side decides
        let fit = Scale::Within {
            width: 64,
            height: 64,
        };
        assert_eq!(size(&fit.apply(image(160, 320))), (32, 64));
        assert_eq!(size(&fit.apply(image(64, 20))), (64, 20));
        assert_eq!(size(&Scale::Full.apply(image(320, 160))), (320, 160));
    }

    #[test]
    fn an_exact_size_scales_either_way() {
        let exact = Scale::Exact {
            width: 100,
            height: 50,
        };
        assert_eq!(size(&exact.apply(image(320, 160))), (100, 50));
        assert_eq!(size(&exact.apply(image(40, 40))), (100, 50));
    }

    #[test]
    fn each_encoding_writes_its_format() {
        let png = encode(&image(8, 8), Encoding::Png).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        // an image with alpha and one without both encode as JPEG
        let jpeg = encode(&image(8, 8), Encoding::Jpeg(JPEG_QUALITY)).unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        let rgb = DynamicImage::ImageRgb8(image(8, 8).to_rgb8());
        let decoded = image::load_from_memory(&encode(&rgb, Encoding::Jpeg(60)).unwrap()).unwrap();
        assert_eq!(size(&decoded), (8, 8));
    }
}
