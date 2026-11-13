//! The blurred cover behind the home greeting.

use iced::widget::image::Handle;

/// The widest the blurred image gets. Blurring a small image is fast,
/// and the blur hides the missing detail anyway.
const MAX_WIDTH: u32 = 480;

/// How strong the blur is. Higher is blurrier.
const BLUR_SIGMA: f32 = 30.0;

/// The size to shrink an image to before blurring: at most `MAX_WIDTH` wide,
/// keeping its proportions, and never larger than the original.
pub fn backdrop_size(width: u32, height: u32) -> (u32, u32) {
    if width <= MAX_WIDTH {
        return (width, height);
    }
    let scaled_height = height * MAX_WIDTH / width;
    (MAX_WIDTH, scaled_height)
}

/// Decodes a cover, shrinks it and blurs it. `None` if the bytes are not an image.
pub fn blurred_backdrop(bytes: &[u8]) -> Option<Handle> {
    let cover = image::load_from_memory(bytes).ok()?;
    let (width, height) = backdrop_size(cover.width(), cover.height());
    let blurred = cover
        .resize_exact(width, height, image::imageops::FilterType::Triangle)
        .fast_blur(BLUR_SIGMA)
        .to_rgba8();

    Some(Handle::from_rgba(width, height, blurred.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinks_wide_images_to_the_max_width() {
        assert_eq!(backdrop_size(1200, 1200), (480, 480));
        assert_eq!(backdrop_size(960, 480), (480, 240));
    }

    #[test]
    fn never_enlarges_small_images() {
        assert_eq!(backdrop_size(300, 300), (300, 300));
    }

    #[test]
    fn rejects_bytes_that_are_not_an_image() {
        assert!(blurred_backdrop(b"this is not an image").is_none());
    }

    #[test]
    fn blurs_a_sample_cover() {
        let bytes = include_bytes!("../assets/covers/luz-de-neon.png");
        assert!(blurred_backdrop(bytes).is_some());
    }
}
