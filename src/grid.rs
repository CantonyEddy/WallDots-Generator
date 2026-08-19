//! Découpage de l'image en grille et calcul de chaque point (position, rayon,
//! couleur) à partir de la luminosité et de la couleur dominante de la cellule.

use crate::color::{dominant_color, luminance};
use crate::config::{Config, Rgb};
use image::RgbImage;

/// Un point à dessiner, en coordonnées de l'image de sortie.
#[derive(Debug, Clone, Copy)]
pub struct Dot {
    pub cx: f32,
    pub cy: f32,
    pub radius: f32,
    pub color: Rgb,
}

/// Grille de points prête à être rendue.
#[derive(Debug, Clone)]
pub struct DotGrid {
    pub width: u32,
    pub height: u32,
    pub dots: Vec<Dot>,
}

/// Nombre de bits par canal pour la quantification de la couleur dominante.
const QUANT_BITS: u8 = 4;
/// En-dessous de ce rayon (px) un point est ignoré (invisible / bruit).
const MIN_VISIBLE_RADIUS: f32 = 0.15;

/// Construit la grille de points à partir de l'image (déjà prétraitée).
pub fn build(img: &RgbImage, cfg: &Config) -> DotGrid {
    let w = img.width();
    let h = img.height();
    let wf = w as f64;
    let hf = h as f64;

    let cols = cfg.cols;
    // Lignes déduites pour garder des cellules ~carrées (donc des points ronds).
    let rows = ((cols as f64) * hf / wf).round().max(1.0) as u32;

    let out_w = (wf * cfg.scale as f64).round().max(1.0) as u32;
    let out_h = (hf * cfg.scale as f64).round().max(1.0) as u32;

    let cell_w = out_w as f32 / cols as f32;
    let cell_h = out_h as f32 / rows as f32;
    let half = cell_w.min(cell_h) / 2.0;

    let mut dots = Vec::with_capacity((cols * rows) as usize);
    let mut region: Vec<(u8, u8, u8)> = Vec::new();

    for j in 0..rows {
        // Bornes verticales de la cellule dans l'image source.
        let sy0 = ((j as f64) * hf / rows as f64).floor() as u32;
        let sy1 = (((j + 1) as f64) * hf / rows as f64).floor().max((sy0 + 1) as f64) as u32;
        let sy1 = sy1.min(h);

        for i in 0..cols {
            let sx0 = ((i as f64) * wf / cols as f64).floor() as u32;
            let sx1 = (((i + 1) as f64) * wf / cols as f64).floor().max((sx0 + 1) as f64) as u32;
            let sx1 = sx1.min(w);

            region.clear();
            let mut lum_sum = 0.0f64;
            let mut count = 0u64;
            for y in sy0..sy1 {
                for x in sx0..sx1 {
                    let p = img.get_pixel(x, y);
                    region.push((p[0], p[1], p[2]));
                    lum_sum += luminance(p[0], p[1], p[2]) as f64;
                    count += 1;
                }
            }
            if count == 0 {
                continue;
            }

            let lum = (lum_sum / count as f64) as f32;
            // Courbe gamma puis inversion éventuelle.
            let mut t = lum.clamp(0.0, 1.0).powf(cfg.gamma);
            if cfg.invert {
                t = 1.0 - t;
            }
            let frac = cfg.min_radius + t * (cfg.max_radius - cfg.min_radius);
            let radius = frac * half;
            if radius < MIN_VISIBLE_RADIUS {
                continue;
            }

            let color = dominant_color(region.iter().copied(), QUANT_BITS)
                .unwrap_or(Rgb::BLACK);

            dots.push(Dot {
                cx: (i as f32 + 0.5) * cell_w,
                cy: (j as f32 + 0.5) * cell_h,
                radius,
                color,
            });
        }
    }

    DotGrid {
        width: out_w,
        height: out_h,
        dots,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BwMode, DotShape, Rgb};
    use std::path::PathBuf;

    fn base_cfg() -> Config {
        Config {
            input: PathBuf::new(),
            output: PathBuf::new(),
            cols: 4,
            scale: 1.0,
            bw: BwMode::None,
            threshold: 0.5,
            min_radius: 0.0,
            max_radius: 1.0,
            gamma: 1.0,
            invert: false,
            background: Rgb::BLACK,
            shape: DotShape::Circle,
            png: true,
            svg: true,
        }
    }

    #[test]
    fn white_image_gives_big_dots() {
        let img = RgbImage::from_pixel(40, 40, image::Rgb([255, 255, 255]));
        let g = build(&img, &base_cfg());
        assert!(!g.dots.is_empty());
        // clair -> gros points (proche du demi-pas).
        assert!(g.dots.iter().all(|d| d.radius > 3.0));
    }

    #[test]
    fn black_image_gives_no_visible_dots() {
        let img = RgbImage::from_pixel(40, 40, image::Rgb([0, 0, 0]));
        let g = build(&img, &base_cfg());
        assert!(g.dots.is_empty());
    }

    #[test]
    fn output_dimensions_follow_scale() {
        let img = RgbImage::from_pixel(40, 20, image::Rgb([128, 128, 128]));
        let mut cfg = base_cfg();
        cfg.scale = 2.0;
        let g = build(&img, &cfg);
        assert_eq!(g.width, 80);
        assert_eq!(g.height, 40);
    }
}
