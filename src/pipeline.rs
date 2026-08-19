//! Orchestration du cœur : chargement → prétraitement → grille → rendu.
//!
//! Aucune dépendance à l'interface : la CLI (V1) et la future TUI (V2)
//! appellent toutes deux [`run`].

use crate::config::Config;
use crate::{grid, preprocess, render};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Exécute le pipeline complet et renvoie la liste des fichiers écrits.
pub fn run(cfg: &Config) -> Result<Vec<PathBuf>> {
    cfg.validate().map_err(anyhow::Error::msg)?;

    let mut img = image::open(&cfg.input)
        .with_context(|| format!("ouverture de l'image : {}", cfg.input.display()))?
        .to_rgb8();

    preprocess::apply(&mut img, cfg.bw, cfg.threshold);

    let dot_grid = grid::build(&img, cfg);

    let mut written = Vec::new();

    if cfg.png {
        let mut path = cfg.output.clone();
        path.set_extension("png");
        render::save_png(&dot_grid, cfg.background, cfg.shape, &path)?;
        written.push(path);
    }
    if cfg.svg {
        let mut path = cfg.output.clone();
        path.set_extension("svg");
        render::save_svg(&dot_grid, cfg.background, cfg.shape, &path)?;
        written.push(path);
    }

    Ok(written)
}
