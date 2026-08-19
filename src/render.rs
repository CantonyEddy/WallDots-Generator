//! Rendu de la grille de points vers SVG (vectoriel) et PNG (rastérisé, anti-crénelé).

use crate::config::{DotShape, Rgb};
use crate::grid::DotGrid;
use anyhow::{Context, Result};
use std::fmt::Write as _;
use std::path::Path;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

/// Rayon des coins du carré arrondi, en fraction du demi-côté.
const CORNER_FRAC: f32 = 0.35;

/// Sommets d'un polygone régulier centré en `(cx, cy)`, circonscrit dans `r`.
///
/// `sides` = nombre de côtés, `start_deg` = angle du premier sommet (0° = est,
/// sens horaire vers le bas puisque l'axe Y descend).
fn regular_polygon(cx: f32, cy: f32, r: f32, sides: usize, start_deg: f32) -> Vec<(f32, f32)> {
    (0..sides)
        .map(|i| {
            let a = (start_deg + i as f32 * 360.0 / sides as f32).to_radians();
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// Sommets pour les formes polygonales (triangle, hexagone). `None` sinon.
fn polygon_points(shape: DotShape, cx: f32, cy: f32, r: f32) -> Option<Vec<(f32, f32)>> {
    match shape {
        // Pointe en haut : premier sommet à -90°.
        DotShape::Triangle => Some(regular_polygon(cx, cy, r, 3, -90.0)),
        DotShape::Hexagon => Some(regular_polygon(cx, cy, r, 6, -90.0)),
        _ => None,
    }
}

/// Génère le SVG sous forme de chaîne.
pub fn to_svg(grid: &DotGrid, bg: Rgb, shape: DotShape) -> String {
    let mut s = String::with_capacity(grid.dots.len() * 56 + 256);
    let _ = write!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
viewBox=\"0 0 {w} {h}\">\n<rect width=\"{w}\" height=\"{h}\" fill=\"{bg}\"/>\n",
        w = grid.width,
        h = grid.height,
        bg = bg.to_hex(),
    );

    for d in &grid.dots {
        let fill = d.color.to_hex();
        match shape {
            DotShape::Circle => {
                let _ = write!(
                    s,
                    "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\" fill=\"{fill}\"/>\n",
                    d.cx, d.cy, d.radius
                );
            }
            DotShape::Square => {
                let side = d.radius * 2.0;
                let c = d.radius * CORNER_FRAC;
                let _ = write!(
                    s,
                    "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" \
rx=\"{:.2}\" ry=\"{:.2}\" fill=\"{fill}\"/>\n",
                    d.cx - d.radius,
                    d.cy - d.radius,
                    side,
                    side,
                    c,
                    c
                );
            }
            DotShape::Triangle | DotShape::Hexagon => {
                if let Some(pts) = polygon_points(shape, d.cx, d.cy, d.radius) {
                    s.push_str("<polygon points=\"");
                    for (i, (x, y)) in pts.iter().enumerate() {
                        if i > 0 {
                            s.push(' ');
                        }
                        let _ = write!(s, "{x:.2},{y:.2}");
                    }
                    let _ = write!(s, "\" fill=\"{fill}\"/>\n");
                }
            }
        }
    }

    s.push_str("</svg>\n");
    s
}

/// Écrit le SVG sur disque.
pub fn save_svg(grid: &DotGrid, bg: Rgb, shape: DotShape, path: &Path) -> Result<()> {
    let svg = to_svg(grid, bg, shape);
    std::fs::write(path, svg).with_context(|| format!("écriture SVG : {}", path.display()))?;
    Ok(())
}

/// Construit le chemin géométrique d'un point pour tiny-skia.
fn dot_path(shape: DotShape, cx: f32, cy: f32, r: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    match shape {
        DotShape::Circle => {
            pb.push_circle(cx, cy, r);
        }
        DotShape::Square => {
            // Carré arrondi construit à la main (coins en courbes quadratiques).
            let (x0, y0, x1, y1) = (cx - r, cy - r, cx + r, cy + r);
            let c = (r * CORNER_FRAC).min(r);
            pb.move_to(x0 + c, y0);
            pb.line_to(x1 - c, y0);
            pb.quad_to(x1, y0, x1, y0 + c);
            pb.line_to(x1, y1 - c);
            pb.quad_to(x1, y1, x1 - c, y1);
            pb.line_to(x0 + c, y1);
            pb.quad_to(x0, y1, x0, y1 - c);
            pb.line_to(x0, y0 + c);
            pb.quad_to(x0, y0, x0 + c, y0);
            pb.close();
        }
        DotShape::Triangle | DotShape::Hexagon => {
            let pts = polygon_points(shape, cx, cy, r)?;
            pb.move_to(pts[0].0, pts[0].1);
            for p in &pts[1..] {
                pb.line_to(p.0, p.1);
            }
            pb.close();
        }
    }
    pb.finish()
}

/// Rastérise la grille de points dans un `Pixmap` anti-crénelé.
///
/// Partagé par l'export PNG et l'aperçu de la TUI.
pub fn render_pixmap(grid: &DotGrid, bg: Rgb, shape: DotShape) -> Result<Pixmap> {
    let mut pixmap = Pixmap::new(grid.width, grid.height)
        .context("dimensions de sortie invalides pour le rendu")?;
    pixmap.fill(Color::from_rgba8(bg.r, bg.g, bg.b, 255));

    let transform = Transform::identity();

    for d in &grid.dots {
        if let Some(geom) = dot_path(shape, d.cx, d.cy, d.radius) {
            let mut paint = Paint::default();
            paint.set_color_rgba8(d.color.r, d.color.g, d.color.b, 255);
            paint.anti_alias = true;
            pixmap.fill_path(&geom, &paint, FillRule::Winding, transform, None);
        }
    }

    Ok(pixmap)
}

/// Rastérise la grille en PNG anti-crénelé et l'écrit sur disque.
pub fn save_png(grid: &DotGrid, bg: Rgb, shape: DotShape, path: &Path) -> Result<()> {
    let pixmap = render_pixmap(grid, bg, shape)?;
    pixmap
        .save_png(path)
        .with_context(|| format!("écriture PNG : {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Dot, DotGrid};

    fn grid1(shape_r: f32) -> DotGrid {
        DotGrid {
            width: 20,
            height: 20,
            dots: vec![Dot {
                cx: 10.0,
                cy: 10.0,
                radius: shape_r,
                color: Rgb { r: 255, g: 255, b: 255 },
            }],
        }
    }

    #[test]
    fn every_shape_renders_pixels() {
        // Chaque forme doit peindre au moins un pixel non-fond au centre.
        for shape in DotShape::ALL {
            let pm = render_pixmap(&grid1(6.0), Rgb::BLACK, shape).unwrap();
            let px = pm.pixel(10, 10).unwrap();
            assert!(
                px.red() > 100,
                "la forme {:?} ne peint pas le centre",
                shape
            );
        }
    }

    #[test]
    fn svg_uses_right_element_per_shape() {
        assert!(to_svg(&grid1(5.0), Rgb::BLACK, DotShape::Circle).contains("<circle"));
        let sq = to_svg(&grid1(5.0), Rgb::BLACK, DotShape::Square);
        assert!(sq.contains("<rect") && sq.contains("rx="));
        assert!(to_svg(&grid1(5.0), Rgb::BLACK, DotShape::Triangle).contains("<polygon"));
        assert!(to_svg(&grid1(5.0), Rgb::BLACK, DotShape::Hexagon).contains("<polygon"));
    }
}
