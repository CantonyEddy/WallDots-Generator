//! walldots — transforme une image en grille de points type halftone.
//!
//! La taille de chaque point suit la luminosité locale et sa couleur reprend
//! la teinte dominante des pixels environnants. Une option permet de passer
//! l'image en noir & blanc avant la transformation.

mod color;
mod config;
mod grid;
mod pipeline;
mod preprocess;
mod render;
mod tui;

use clap::Parser;
use config::{BwMode, Config, DotShape, Rgb};
use std::path::PathBuf;
use std::process::ExitCode;

/// Formats de sortie demandés.
#[derive(Debug, Clone, Copy)]
enum Format {
    Png,
    Svg,
    Both,
}

impl std::str::FromStr for Format {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "png" => Ok(Format::Png),
            "svg" => Ok(Format::Svg),
            "both" | "les-deux" | "all" => Ok(Format::Both),
            other => Err(format!("format inconnu : {other} (png, svg ou both)")),
        }
    }
}

const LONG_ABOUT: &str = "\
Transforme une image en grille de points (style halftone) pour en faire un wallpaper.

La taille de chaque point suit la luminosité locale et sa couleur reprend la teinte
dominante des pixels environnants. Une option permet de convertir l'image en noir &
blanc avant la transformation. Sortie en PNG (rastérisé, anti-crénelé) et/ou SVG
(vectoriel).

L'aide est regroupée par fonctionnalité ci-dessous. Exemples :
  walldots photo.jpg                       # PNG + SVG, réglages par défaut
  walldots photo.jpg -g 160 -f png         # grille fine, PNG seul
  walldots photo.jpg --bw grayscale        # halftone noir & blanc
  walldots photo.jpg --tui                 # réglage interactif avec aperçu";

/// Transforme une image en grille de points (halftone) pour en faire un wallpaper.
#[derive(Parser, Debug)]
#[command(name = "walldots", version, about, long_about = LONG_ABOUT)]
struct Cli {
    /// Image d'entrée (png, jpg, webp, …).
    input: PathBuf,

    // ---- Transformation en points ------------------------------------------
    /// Nombre de points sur la largeur (résolution de la grille).
    #[arg(short = 'g', long, default_value_t = 100, help_heading = "Transformation en points")]
    grid: u32,

    /// Facteur d'échelle de l'image de sortie (1.0 = taille source).
    #[arg(long, default_value_t = 1.0, help_heading = "Transformation en points")]
    scale: f32,

    /// Rayon minimal du point, en fraction du demi-pas de la grille.
    #[arg(long, default_value_t = 0.0, help_heading = "Transformation en points")]
    min_radius: f32,

    /// Rayon maximal du point, en fraction du demi-pas (>1.0 = chevauchement).
    #[arg(long, default_value_t = 1.0, help_heading = "Transformation en points")]
    max_radius: f32,

    /// Courbe (gamma) appliquée à la luminosité : >1 assombrit, <1 éclaircit.
    #[arg(long, default_value_t = 1.0, help_heading = "Transformation en points")]
    gamma: f32,

    /// Inverse la relation luminosité→taille (défaut : clair = gros point).
    #[arg(long, default_value_t = false, help_heading = "Transformation en points")]
    invert: bool,

    /// Forme des points : circle | square (arrondi) | triangle | hexagon.
    #[arg(long, default_value = "circle", help_heading = "Transformation en points")]
    shape: DotShape,

    /// Couleur de fond (hex : #rrggbb, rrggbb, #rgb).
    #[arg(long, default_value = "#000000", help_heading = "Transformation en points")]
    bg: Rgb,

    // ---- Noir & blanc ------------------------------------------------------
    /// Conversion avant transformation : none | grayscale | threshold.
    #[arg(long, default_value = "none", help_heading = "Noir & blanc")]
    bw: BwMode,

    /// Seuil de binarisation pour --bw threshold (0.0..=1.0).
    #[arg(long, default_value_t = 0.5, help_heading = "Noir & blanc")]
    threshold: f32,

    // ---- Sortie & mode -----------------------------------------------------
    /// Base du fichier de sortie (les extensions .png/.svg sont ajoutées).
    /// Par défaut : "<image>_dots".
    #[arg(short, long, help_heading = "Sortie & mode")]
    output: Option<PathBuf>,

    /// Format(s) de sortie : png | svg | both.
    #[arg(short, long, default_value = "both", help_heading = "Sortie & mode")]
    format: Format,

    /// Ouvre l'interface interactive (aperçu temps réel) au lieu de générer directement.
    #[arg(short = 't', long, help_heading = "Sortie & mode")]
    tui: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let output = cli.output.unwrap_or_else(|| {
        let stem = cli
            .input
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "walldots".to_string());
        let mut p = cli.input.clone();
        p.set_file_name(format!("{stem}_dots"));
        p
    });

    let (png, svg) = match cli.format {
        Format::Png => (true, false),
        Format::Svg => (false, true),
        Format::Both => (true, true),
    };

    let cfg = Config {
        input: cli.input,
        output,
        cols: cli.grid,
        scale: cli.scale,
        bw: cli.bw,
        threshold: cli.threshold,
        min_radius: cli.min_radius,
        max_radius: cli.max_radius,
        gamma: cli.gamma,
        invert: cli.invert,
        background: cli.bg,
        shape: cli.shape,
        png,
        svg,
    };

    if cli.tui {
        return match tui::run(cfg) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("erreur : {e:#}");
                ExitCode::FAILURE
            }
        };
    }

    match pipeline::run(&cfg) {
        Ok(written) => {
            for p in &written {
                println!("\u{2713} {}", p.display());
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("erreur : {e:#}");
            ExitCode::FAILURE
        }
    }
}
