//! Prétraitement optionnel de l'image avant la mise en points.

use crate::color::luminance;
use crate::config::BwMode;
use image::RgbImage;

/// Applique la conversion N&B choisie, en place.
pub fn apply(img: &mut RgbImage, mode: BwMode, threshold: f32) {
    match mode {
        BwMode::None => {}
        BwMode::Grayscale => {
            for p in img.pixels_mut() {
                let l = (luminance(p[0], p[1], p[2]) * 255.0).round() as u8;
                *p = image::Rgb([l, l, l]);
            }
        }
        BwMode::Threshold => {
            for p in img.pixels_mut() {
                let v = if luminance(p[0], p[1], p[2]) >= threshold {
                    255
                } else {
                    0
                };
                *p = image::Rgb([v, v, v]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grayscale_makes_channels_equal() {
        let mut img = RgbImage::from_pixel(2, 2, image::Rgb([120, 200, 40]));
        apply(&mut img, BwMode::Grayscale, 0.5);
        for p in img.pixels() {
            assert_eq!(p[0], p[1]);
            assert_eq!(p[1], p[2]);
        }
    }

    #[test]
    fn threshold_is_binary() {
        let mut img = RgbImage::new(2, 1);
        img.put_pixel(0, 0, image::Rgb([10, 10, 10]));
        img.put_pixel(1, 0, image::Rgb([240, 240, 240]));
        apply(&mut img, BwMode::Threshold, 0.5);
        assert_eq!(img.get_pixel(0, 0)[0], 0);
        assert_eq!(img.get_pixel(1, 0)[0], 255);
    }
}
