//! Luminance et calcul de la couleur dominante d'un ensemble de pixels.

use crate::config::Rgb;

/// Luminance perçue (Rec. 709), normalisée dans 0.0..=1.0.
#[inline]
pub fn luminance(r: u8, g: u8, b: u8) -> f32 {
    (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0
}

/// Couleur dominante d'un ensemble de pixels `(r, g, b)`.
///
/// On quantifie chaque canal sur `1 << bits` niveaux, on compte le seau le plus
/// fréquent (le « mode » de l'histogramme — la teinte réellement majoritaire,
/// et non la moyenne qui donnerait un gris terne), puis on renvoie la moyenne
/// exacte des pixels tombés dans ce seau pour éviter le banding.
///
/// `bits` typique : 4 (16 niveaux/canal). Renvoie `None` si l'itérateur est vide.
pub fn dominant_color<I>(pixels: I, bits: u8) -> Option<Rgb>
where
    I: IntoIterator<Item = (u8, u8, u8)>,
{
    let shift = 8 - bits.min(8);
    // Accumulateur par seau : (compte, somme_r, somme_g, somme_b).
    use std::collections::HashMap;
    let mut buckets: HashMap<u32, (u64, u64, u64, u64)> = HashMap::new();

    for (r, g, b) in pixels {
        let key = ((r as u32 >> shift) << 16) | ((g as u32 >> shift) << 8) | (b as u32 >> shift);
        let e = buckets.entry(key).or_insert((0, 0, 0, 0));
        e.0 += 1;
        e.1 += r as u64;
        e.2 += g as u64;
        e.3 += b as u64;
    }

    let (_, (count, sr, sg, sb)) = buckets
        .into_iter()
        // Départage stable : le plus fréquent, puis la clé la plus basse.
        .max_by(|a, b| a.1 .0.cmp(&b.1 .0).then(b.0.cmp(&a.0)))?;

    Some(Rgb {
        r: (sr / count) as u8,
        g: (sg / count) as u8,
        b: (sb / count) as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luminance_bounds() {
        assert!((luminance(0, 0, 0) - 0.0).abs() < 1e-6);
        assert!((luminance(255, 255, 255) - 1.0).abs() < 1e-6);
        assert!(luminance(255, 0, 0) < luminance(0, 255, 0)); // le vert pèse plus
    }

    #[test]
    fn dominant_picks_majority() {
        // 3 rouges, 1 bleu -> dominante rouge.
        let px = vec![(200, 10, 10), (210, 20, 5), (205, 15, 12), (10, 10, 220)];
        let c = dominant_color(px, 4).unwrap();
        assert!(c.r > 150 && c.b < 80);
    }

    #[test]
    fn dominant_empty_is_none() {
        let px: Vec<(u8, u8, u8)> = vec![];
        assert!(dominant_color(px, 4).is_none());
    }
}
