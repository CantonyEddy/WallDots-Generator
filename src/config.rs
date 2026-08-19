//! Configuration du pipeline, indépendante de toute interface (CLI/TUI).
//!
//! Cette struct est le seul point d'entrée du cœur : la CLI (et plus tard la
//! TUI en V2) se contente de la remplir puis d'appeler [`crate::pipeline::run`].

use std::path::PathBuf;
use std::str::FromStr;

/// Méthode de conversion en noir & blanc appliquée AVANT la transformation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BwMode {
    /// Pas de conversion : les points reprennent la couleur dominante locale.
    None,
    /// Niveaux de gris (luminance pondérée Rec. 709).
    Grayscale,
    /// Binarisation : chaque cellule devient noire ou blanche selon le seuil.
    Threshold,
}

impl FromStr for BwMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "none" | "color" | "couleur" => Ok(BwMode::None),
            "gray" | "grey" | "grayscale" | "gris" => Ok(BwMode::Grayscale),
            "threshold" | "seuil" | "bw" | "nb" => Ok(BwMode::Threshold),
            other => Err(format!("mode N&B inconnu : {other}")),
        }
    }
}

/// Forme du point dessiné.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotShape {
    /// Cercle (la forme de base).
    Circle,
    /// Carré à bords arrondis.
    Square,
    /// Triangle équilatéral pointe en haut.
    Triangle,
    /// Hexagone (pointe en haut).
    Hexagon,
}

impl DotShape {
    /// Toutes les formes, dans l'ordre de cycle de la TUI.
    pub const ALL: [DotShape; 4] = [
        DotShape::Circle,
        DotShape::Square,
        DotShape::Triangle,
        DotShape::Hexagon,
    ];

    /// Libellé lisible (français).
    pub fn label(self) -> &'static str {
        match self {
            DotShape::Circle => "cercle",
            DotShape::Square => "carré arrondi",
            DotShape::Triangle => "triangle",
            DotShape::Hexagon => "hexagone",
        }
    }
}

impl FromStr for DotShape {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "circle" | "cercle" | "rond" => Ok(DotShape::Circle),
            "square" | "carre" | "carré" | "rounded" | "arrondi" => Ok(DotShape::Square),
            "triangle" => Ok(DotShape::Triangle),
            "hexagon" | "hexagone" | "hex" => Ok(DotShape::Hexagon),
            other => Err(format!(
                "forme inconnue : {other} (circle, square, triangle, hexagon)"
            )),
        }
    }
}

/// Couleur RGB 8 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl FromStr for Rgb {
    type Err = String;
    /// Accepte `#rrggbb`, `rrggbb`, `#rgb` ou `rgb`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let h = s.trim().trim_start_matches('#');
        let full = match h.len() {
            3 => h.chars().flat_map(|c| [c, c]).collect::<String>(),
            6 => h.to_string(),
            _ => return Err(format!("couleur hex invalide : {s}")),
        };
        let parse = |i: usize| {
            u8::from_str_radix(&full[i..i + 2], 16).map_err(|_| format!("couleur hex invalide : {s}"))
        };
        Ok(Rgb {
            r: parse(0)?,
            g: parse(2)?,
            b: parse(4)?,
        })
    }
}

/// Configuration complète d'un rendu.
#[derive(Debug, Clone)]
pub struct Config {
    /// Image source.
    pub input: PathBuf,
    /// Base du chemin de sortie (les extensions .png/.svg sont ajoutées).
    pub output: PathBuf,

    /// Nombre de points sur la largeur (résolution de la grille).
    pub cols: u32,
    /// Facteur d'échelle de l'image de sortie (1.0 = taille source).
    pub scale: f32,

    /// Conversion N&B éventuelle avant transformation.
    pub bw: BwMode,
    /// Seuil pour [`BwMode::Threshold`], dans 0.0..=1.0.
    pub threshold: f32,

    /// Rayon minimal du point, en fraction du demi-pas (0.0..=1.0+).
    pub min_radius: f32,
    /// Rayon maximal du point, en fraction du demi-pas (0.0..=1.0+).
    pub max_radius: f32,
    /// Courbe appliquée à la luminosité (gamma) : >1 assombrit, <1 éclaircit.
    pub gamma: f32,
    /// Inverse la relation luminosité→taille (par défaut : clair = gros point).
    pub invert: bool,

    /// Couleur de fond du rendu.
    pub background: Rgb,
    /// Forme des points.
    pub shape: DotShape,

    /// Génère le PNG.
    pub png: bool,
    /// Génère le SVG.
    pub svg: bool,
}

impl Config {
    /// Vérifie la cohérence des paramètres avant exécution.
    pub fn validate(&self) -> Result<(), String> {
        if self.cols == 0 {
            return Err("le nombre de colonnes (--grid) doit être >= 1".into());
        }
        if self.scale <= 0.0 {
            return Err("l'échelle (--scale) doit être > 0".into());
        }
        if !(0.0..=1.0).contains(&self.threshold) {
            return Err("le seuil (--threshold) doit être dans 0.0..=1.0".into());
        }
        if self.min_radius < 0.0 || self.max_radius < 0.0 {
            return Err("les rayons doivent être >= 0".into());
        }
        if self.min_radius > self.max_radius {
            return Err("--min-radius doit être <= --max-radius".into());
        }
        if self.gamma <= 0.0 {
            return Err("le gamma doit être > 0".into());
        }
        if !self.png && !self.svg {
            return Err("aucun format de sortie sélectionné".into());
        }
        Ok(())
    }
}
