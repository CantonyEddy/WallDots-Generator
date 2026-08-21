//! Interface interactive (TUI) pour régler les paramètres avec un aperçu en
//! temps réel, puis exporter. Réutilise entièrement le cœur (config/pipeline).
//!
//! L'aperçu est rendu via `ratatui-image` : vraie image (protocole Kitty/Sixel
//! si le terminal le supporte) avec repli automatique en demi-blocs Unicode.

use crate::browser::{Action as BrowserAction, Browser};
use crate::config::{BwMode, Config, DotShape, Rgb};
use crate::theme::{panel, selected_style, ACCENT, ACCENT2};
use crate::{grid, pipeline, preprocess, render};
use anyhow::Result;
use image::{imageops::FilterType, DynamicImage, RgbImage};
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Taille max (en px) de l'image de travail servant à l'aperçu.
const PREVIEW_SRC_MAX: u32 = 480;
/// Nombre de paramètres réglables.
const PARAM_COUNT: usize = 11;

/// Action demandée par le tuner suite à une touche.
enum TunerAction {
    None,
    Quit,
    OpenBrowser,
}

/// Presets de couleur de fond proposés dans la TUI.
fn bg_presets() -> Vec<(&'static str, Rgb)> {
    vec![
        ("Noir", Rgb { r: 0, g: 0, b: 0 }),
        ("Blanc", Rgb { r: 255, g: 255, b: 255 }),
        ("Gris foncé", Rgb { r: 17, g: 17, b: 17 }),
        ("Ardoise", Rgb { r: 26, g: 27, b: 38 }),
        ("Bleu nuit", Rgb { r: 11, g: 19, b: 43 }),
        ("Crème", Rgb { r: 245, g: 240, b: 230 }),
    ]
}

struct App {
    cfg: Config,
    preview_src: RgbImage,
    selected: usize,
    status: String,
    bg_options: Vec<(String, Rgb)>,
    bg_index: usize,
    /// Aperçu zoomé (recadrage central agrandi) pour voir la forme des points.
    zoom: bool,
    /// Protocole d'image de l'aperçu (rendu haute qualité). Reconstruit si `dirty`.
    protocol: Option<StatefulProtocol>,
    /// L'aperçu doit être régénéré (un réglage a changé).
    dirty: bool,
    /// Saisie directe d'une valeur en cours (pour le champ sélectionné).
    editing: bool,
    /// Tampon de saisie.
    edit_buf: String,
}

impl App {
    fn new(cfg: Config, preview_src: RgbImage) -> Self {
        // Construit la liste des fonds ; insère le fond courant s'il n'est pas un preset.
        let mut bg_options: Vec<(String, Rgb)> = bg_presets()
            .into_iter()
            .map(|(n, c)| (n.to_string(), c))
            .collect();
        let bg_index = match bg_options.iter().position(|(_, c)| *c == cfg.background) {
            Some(i) => i,
            None => {
                bg_options.insert(0, ("Perso".to_string(), cfg.background));
                0
            }
        };
        App {
            cfg,
            preview_src,
            selected: 0,
            status: "Prêt. Réglez, puis « s » pour sauvegarder.".to_string(),
            bg_options,
            bg_index,
            zoom: false,
            protocol: None,
            dirty: true,
            editing: false,
            edit_buf: String::new(),
        }
    }

    /// Le champ `i` accepte-t-il une saisie directe de valeur ?
    fn is_typeable(i: usize) -> bool {
        matches!(i, 0 | 1 | 3 | 4 | 5 | 6 | 9)
    }

    /// Démarre la saisie du champ sélectionné (pré-remplie avec la valeur courante).
    fn start_edit(&mut self) {
        if !Self::is_typeable(self.selected) {
            return;
        }
        self.edit_buf = match self.selected {
            0 => self.cfg.cols.to_string(),
            1 => format!("{:.2}", self.cfg.scale),
            3 => format!("{:.2}", self.cfg.threshold),
            4 => format!("{:.2}", self.cfg.min_radius),
            5 => format!("{:.2}", self.cfg.max_radius),
            6 => format!("{:.2}", self.cfg.gamma),
            9 => self.cfg.background.to_hex(),
            _ => String::new(),
        };
        self.editing = true;
    }

    /// Valide la saisie : parse et applique la valeur (bornée), sinon signale l'erreur.
    fn commit_edit(&mut self) {
        let s = self.edit_buf.trim();
        let mut ok = true;
        match self.selected {
            0 => match s.parse::<i64>() {
                Ok(v) => self.cfg.cols = v.clamp(2, 600) as u32,
                Err(_) => ok = false,
            },
            1 => match s.parse::<f32>() {
                Ok(v) => self.cfg.scale = v.clamp(0.25, 8.0),
                Err(_) => ok = false,
            },
            3 => match s.parse::<f32>() {
                Ok(v) => self.cfg.threshold = v.clamp(0.0, 1.0),
                Err(_) => ok = false,
            },
            4 => match s.parse::<f32>() {
                Ok(v) => self.cfg.min_radius = v.clamp(0.0, self.cfg.max_radius),
                Err(_) => ok = false,
            },
            5 => match s.parse::<f32>() {
                Ok(v) => self.cfg.max_radius = v.clamp(self.cfg.min_radius, 2.0),
                Err(_) => ok = false,
            },
            6 => match s.parse::<f32>() {
                Ok(v) => self.cfg.gamma = v.clamp(0.1, 5.0),
                Err(_) => ok = false,
            },
            9 => match s.parse::<Rgb>() {
                Ok(c) => {
                    self.cfg.background = c;
                    if let Some(i) = self.bg_options.iter().position(|(_, x)| *x == c) {
                        self.bg_index = i;
                    }
                }
                Err(_) => ok = false,
            },
            _ => {}
        }
        self.editing = false;
        self.edit_buf.clear();
        if ok {
            self.dirty = true;
        } else {
            self.status = "valeur invalide".to_string();
        }
    }

    fn label(&self, i: usize) -> &'static str {
        match i {
            0 => "Grille (points/largeur)",
            1 => "Échelle de sortie",
            2 => "Noir & blanc",
            3 => "Seuil (binarisation)",
            4 => "Rayon min",
            5 => "Rayon max",
            6 => "Gamma (luminosité)",
            7 => "Inverser taille",
            8 => "Forme",
            9 => "Fond",
            10 => "Format de sortie",
            _ => "",
        }
    }

    fn value(&self, i: usize) -> String {
        // Champ en cours de saisie : afficher le tampon + curseur.
        if self.editing && i == self.selected {
            return format!("{}▏", self.edit_buf);
        }
        match i {
            0 => self.cfg.cols.to_string(),
            1 => format!("{:.2}x", self.cfg.scale),
            2 => match self.cfg.bw {
                BwMode::None => "non (couleur)".into(),
                BwMode::Grayscale => "niveaux de gris".into(),
                BwMode::Threshold => "seuil".into(),
            },
            3 => format!("{:.2}", self.cfg.threshold),
            4 => format!("{:.2}", self.cfg.min_radius),
            5 => format!("{:.2}", self.cfg.max_radius),
            6 => format!("{:.2}", self.cfg.gamma),
            7 => if self.cfg.invert { "oui" } else { "non" }.into(),
            8 => self.cfg.shape.label().into(),
            9 => {
                // Nom du preset si le fond en est un, sinon la valeur hex.
                let bg = self.cfg.background;
                self.bg_options
                    .iter()
                    .find(|(_, c)| *c == bg)
                    .map(|(n, _)| n.clone())
                    .unwrap_or_else(|| bg.to_hex())
            }
            10 => match (self.cfg.png, self.cfg.svg) {
                (true, true) => "png + svg".into(),
                (true, false) => "png".into(),
                (false, true) => "svg".into(),
                (false, false) => "png".into(),
            },
            _ => String::new(),
        }
    }

    /// Ajuste le paramètre sélectionné. `dir` vaut +1 (droite) ou -1 (gauche).
    fn adjust(&mut self, dir: i32) {
        let d = dir as f32;
        match self.selected {
            0 => {
                let step = 4i64 * dir as i64;
                self.cfg.cols = (self.cfg.cols as i64 + step).clamp(2, 600) as u32;
            }
            1 => self.cfg.scale = (self.cfg.scale + 0.25 * d).clamp(0.25, 8.0),
            2 => {
                self.cfg.bw = match (self.cfg.bw, dir >= 0) {
                    (BwMode::None, true) => BwMode::Grayscale,
                    (BwMode::Grayscale, true) => BwMode::Threshold,
                    (BwMode::Threshold, true) => BwMode::None,
                    (BwMode::None, false) => BwMode::Threshold,
                    (BwMode::Grayscale, false) => BwMode::None,
                    (BwMode::Threshold, false) => BwMode::Grayscale,
                };
            }
            3 => self.cfg.threshold = (self.cfg.threshold + 0.05 * d).clamp(0.0, 1.0),
            4 => {
                self.cfg.min_radius =
                    (self.cfg.min_radius + 0.05 * d).clamp(0.0, self.cfg.max_radius);
            }
            5 => {
                self.cfg.max_radius =
                    (self.cfg.max_radius + 0.05 * d).clamp(self.cfg.min_radius, 2.0);
            }
            6 => self.cfg.gamma = (self.cfg.gamma + 0.1 * d).clamp(0.1, 5.0),
            7 => self.cfg.invert = !self.cfg.invert,
            8 => {
                let all = DotShape::ALL;
                let n = all.len() as i32;
                let cur = all.iter().position(|s| *s == self.cfg.shape).unwrap_or(0) as i32;
                self.cfg.shape = all[(((cur + dir) % n + n) % n) as usize];
            }
            9 => {
                let n = self.bg_options.len() as i32;
                self.bg_index = (((self.bg_index as i32 + dir) % n + n) % n) as usize;
                self.cfg.background = self.bg_options[self.bg_index].1;
            }
            10 => {
                // Cycle png -> svg -> png+svg.
                let cur = match (self.cfg.png, self.cfg.svg) {
                    (true, false) => 0,
                    (false, true) => 1,
                    _ => 2,
                };
                let next = ((cur + dir) % 3 + 3) % 3;
                (self.cfg.png, self.cfg.svg) = match next {
                    0 => (true, false),
                    1 => (false, true),
                    _ => (true, true),
                };
            }
            _ => {}
        }
        self.dirty = true; // un réglage invalide l'aperçu
    }

    fn handle_key(&mut self, code: KeyCode) -> TunerAction {
        // Saisie directe d'une valeur : les touches alimentent le tampon.
        if self.editing {
            match code {
                KeyCode::Enter => self.commit_edit(),
                KeyCode::Esc => {
                    self.editing = false;
                    self.edit_buf.clear();
                }
                KeyCode::Backspace => {
                    self.edit_buf.pop();
                }
                KeyCode::Char(c) => self.edit_buf.push(c),
                _ => {}
            }
            return TunerAction::None;
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => return TunerAction::Quit,
            KeyCode::Char('o') => return TunerAction::OpenBrowser,
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = (self.selected + PARAM_COUNT - 1) % PARAM_COUNT;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1) % PARAM_COUNT;
            }
            KeyCode::Left | KeyCode::Char('h') => self.adjust(-1),
            KeyCode::Right | KeyCode::Char('l') => self.adjust(1),
            // ↵ : saisir une valeur (champs numériques/hex) ou cycler (champs à choix).
            KeyCode::Enter => {
                if Self::is_typeable(self.selected) {
                    self.start_edit();
                } else {
                    self.adjust(1);
                }
            }
            KeyCode::Char(' ') => self.adjust(1),
            KeyCode::Char('z') => {
                self.zoom = !self.zoom;
                self.dirty = true;
            }
            KeyCode::Char('s') => self.save(),
            _ => {}
        }
        TunerAction::None
    }

    fn save(&mut self) {
        match pipeline::run(&self.cfg) {
            Ok(paths) => {
                let names: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
                self.status = format!("Sauvegardé : {}", names.join(", "));
            }
            Err(e) => self.status = format!("Erreur : {e}"),
        }
    }

    /// Rend l'aperçu dotifié en une `RgbImage` haute résolution.
    ///
    /// Mode ajusté : l'image entière ; mode zoom : recadrage central agrandi
    /// (chaque point couvre plus de pixels, la forme devient nette).
    fn preview_image(&self) -> Option<RgbImage> {
        // Source, éventuellement recadrée au centre en mode zoom.
        let src = if self.zoom {
            let (sw, sh) = (self.preview_src.width(), self.preview_src.height());
            let cw = ((sw as f32 * 0.4).round() as u32).max(1);
            let ch = ((sh as f32 * 0.4).round() as u32).max(1);
            let x = (sw.saturating_sub(cw)) / 2;
            let y = (sh.saturating_sub(ch)) / 2;
            image::imageops::crop_imm(&self.preview_src, x, y, cw, ch).to_image()
        } else {
            self.preview_src.clone()
        };

        let mut img = src;
        preprocess::apply(&mut img, self.cfg.bw, self.cfg.threshold);

        // Résolution de rendu : ~10 px par cellule (net), plafonnée.
        let target_w = (self.cfg.cols as f32 * 10.0).clamp(240.0, 1600.0);
        let mut pcfg = self.cfg.clone();
        pcfg.scale = target_w / img.width() as f32;

        let dot_grid = grid::build(&img, &pcfg);
        render::render_rgb(&dot_grid, self.cfg.background, self.cfg.shape).ok()
    }

    fn draw(&mut self, frame: &mut Frame, picker: &mut Picker) {
        let area = frame.area();
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(2)])
            .split(area);

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(34), Constraint::Min(10)])
            .split(root[0]);

        self.draw_params(frame, cols[0]);
        self.draw_preview(frame, cols[1], picker);
        self.draw_footer(frame, root[1]);
    }

    fn draw_params(&self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::with_capacity(PARAM_COUNT);
        for i in 0..PARAM_COUNT {
            let selected = i == self.selected;
            let marker = if selected { "▶ " } else { "  " };
            let label = format!("{marker}{}", self.label(i));
            let value = self.value(i);
            let style = if selected {
                selected_style(Style::default())
            } else {
                Style::default()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{label:<24}"), style),
                Span::raw(" "),
                Span::styled(value, Style::default().fg(ACCENT2)),
            ]));
        }
        frame.render_widget(Paragraph::new(lines).block(panel(" Paramètres ")), area);
    }

    fn draw_preview(&mut self, frame: &mut Frame, area: Rect, picker: &mut Picker) {
        let title = if self.zoom {
            " Aperçu — zoom (recadrage central) "
        } else {
            " Aperçu "
        };
        let block = panel(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        if self.dirty || self.protocol.is_none() {
            if let Some(rgb) = self.preview_image() {
                self.protocol = Some(picker.new_resize_protocol(DynamicImage::ImageRgb8(rgb)));
            }
            self.dirty = false;
        }
        if let Some(proto) = self.protocol.as_mut() {
            // Crop = l'image couvre toute la zone (remplit, sans bandes noires).
            let widget = StatefulImage::default().resize(Resize::Crop(None));
            frame.render_stateful_widget(widget, inner, proto);
        }
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let help = Line::from(vec![
            Span::styled("↑↓", Style::default().fg(ACCENT)),
            Span::raw(" paramètre  "),
            Span::styled("←→", Style::default().fg(ACCENT)),
            Span::raw(" ajuster  "),
            Span::styled("↵", Style::default().fg(ACCENT)),
            Span::raw(" saisir/cycler  "),
            Span::styled("z", Style::default().fg(ACCENT)),
            Span::raw(" zoom  "),
            Span::styled("s", Style::default().fg(ACCENT)),
            Span::raw(" sauver  "),
            Span::styled("o", Style::default().fg(ACCENT)),
            Span::raw(" fichiers  "),
            Span::styled("q", Style::default().fg(ACCENT)),
            Span::raw(" quitter"),
        ]);
        let status = Line::from(Span::styled(
            self.status.clone(),
            Style::default().fg(Color::Green),
        ));
        frame.render_widget(
            Paragraph::new(vec![help, status]).alignment(Alignment::Left),
            area,
        );
    }
}

/// Réduit l'image source à une taille de travail raisonnable pour l'aperçu.
fn downscale(img: &RgbImage, max: u32) -> RgbImage {
    let (w, h) = (img.width(), img.height());
    if w <= max && h <= max {
        return img.clone();
    }
    let ratio = (max as f32 / w.max(h) as f32).min(1.0);
    let nw = ((w as f32 * ratio).round() as u32).max(1);
    let nh = ((h as f32 * ratio).round() as u32).max(1);
    image::imageops::resize(img, nw, nh, FilterType::Triangle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg() -> Config {
        Config {
            input: PathBuf::new(),
            output: PathBuf::new(),
            cols: 20,
            scale: 1.0,
            bw: BwMode::None,
            threshold: 0.5,
            min_radius: 0.0,
            max_radius: 1.0,
            gamma: 1.0,
            invert: false,
            background: Rgb { r: 0, g: 0, b: 0 },
            shape: DotShape::Circle,
            png: true,
            svg: true,
        }
    }

    #[test]
    fn preview_image_dimensions() {
        // L'aperçu dotifié est une image non vide.
        let mut img = RgbImage::new(200, 120);
        for (x, _y, p) in img.enumerate_pixels_mut() {
            let v = (x * 255 / 200) as u8;
            *p = image::Rgb([v, v, v]);
        }
        let app = App::new(cfg(), img);
        let out = app.preview_image().expect("aperçu");
        assert!(out.width() > 0 && out.height() > 0);
    }

    #[test]
    fn full_frame_draws_and_shows_preview() {
        use ratatui::{backend::TestBackend, Terminal};
        use ratatui_image::picker::Picker;
        let mut img = RgbImage::new(160, 90);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let v = (((x + y) * 255) / 250) as u8;
            *p = image::Rgb([v, v / 2, 255 - v]);
        }
        let mut app = App::new(cfg(), img);
        let mut picker = Picker::halfblocks();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| app.draw(f, &mut picker)).unwrap();

        // Le panneau de paramètres doit afficher au moins un libellé.
        let dump: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(dump.contains("Grille"), "le panneau de paramètres manque");
    }

    #[test]
    fn format_cycles_through_three_states() {
        let mut app = App::new(cfg(), RgbImage::new(20, 20));
        app.selected = 10;
        let seen: Vec<(bool, bool)> = (0..3)
            .map(|_| {
                let st = (app.cfg.png, app.cfg.svg);
                app.adjust(1);
                st
            })
            .collect();
        // Les trois états distincts apparaissent, aucun (false,false).
        assert!(seen.contains(&(true, true)));
        assert!(seen.contains(&(true, false)));
        assert!(seen.contains(&(false, true)));
        assert!(!seen.contains(&(false, false)));
        // Retour au point de départ après 3 pas.
        assert_eq!((app.cfg.png, app.cfg.svg), seen[0]);
    }

    #[test]
    fn preview_differs_by_shape() {
        // Image claire uniforme -> gros points ; cercle et carré diffèrent.
        let img = RgbImage::from_pixel(160, 100, image::Rgb([230, 230, 230]));
        let mut circle = App::new(cfg(), img.clone());
        circle.cfg.shape = DotShape::Circle;
        let mut square = App::new(cfg(), img);
        square.cfg.shape = DotShape::Square;

        let ic = circle.preview_image().expect("aperçu cercle");
        let is = square.preview_image().expect("aperçu carré");
        assert_ne!(
            ic.as_raw(),
            is.as_raw(),
            "cercle et carré doivent produire des aperçus différents"
        );
    }

    #[test]
    fn typing_sets_value_directly() {
        let mut app = App::new(cfg(), RgbImage::new(20, 20));
        // Grille : saisir 250.
        app.selected = 0;
        app.handle_key(KeyCode::Enter);
        assert!(app.editing);
        for _ in 0..8 {
            app.handle_key(KeyCode::Backspace);
        }
        for c in "250".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Enter);
        assert!(!app.editing);
        assert_eq!(app.cfg.cols, 250);

        // Fond : saisir un hex.
        app.selected = 9;
        app.handle_key(KeyCode::Enter);
        for _ in 0..8 {
            app.handle_key(KeyCode::Backspace);
        }
        for c in "#123456".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.cfg.background, Rgb { r: 0x12, g: 0x34, b: 0x56 });

        // Valeur invalide : ignorée, l'ancienne reste.
        app.selected = 0;
        app.handle_key(KeyCode::Enter);
        for _ in 0..8 {
            app.handle_key(KeyCode::Backspace);
        }
        for c in "abc".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.cfg.cols, 250, "valeur invalide -> inchangée");
    }

    #[test]
    fn adjust_and_cycle_stay_in_bounds() {
        let mut app = App::new(cfg(), RgbImage::new(40, 40));
        // Descendre sur « gamma » et le pousser au minimum.
        app.selected = 6;
        for _ in 0..100 {
            app.adjust(-1);
        }
        assert!(app.cfg.gamma >= 0.1 - 1e-6);
        // Cycler le mode N&B doit rester valide et revenir au départ après 3 pas.
        app.selected = 2;
        let start = app.cfg.bw;
        app.adjust(1);
        app.adjust(1);
        app.adjust(1);
        assert_eq!(app.cfg.bw, start);
    }
}

/// Construit un tuner à partir d'une config (ouvre l'image + prépare l'aperçu).
fn build_tuner(cfg: Config) -> Result<App> {
    let full = image::open(&cfg.input)
        .map_err(|e| anyhow::anyhow!("ouverture de l'image {} : {e}", cfg.input.display()))?
        .to_rgb8();
    let preview_src = downscale(&full, PREVIEW_SRC_MAX);
    Ok(App::new(cfg, preview_src))
}

/// Chemin de sortie par défaut pour une image : "<image>_dots".
fn derive_output(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "walldots".to_string());
    let mut p = input.to_path_buf();
    p.set_file_name(format!("{stem}_dots"));
    p
}

enum Mode {
    Browser,
    Tuner,
}

/// Coordonne l'explorateur de fichiers et le tuner.
struct Ui {
    mode: Mode,
    browser: Browser,
    tuner: Option<App>,
    /// Config courante (sert de gabarit en chargeant une nouvelle image).
    cfg: Config,
    /// Détecteur du meilleur protocole d'image du terminal.
    picker: Picker,
}

impl Ui {
    fn draw(&mut self, frame: &mut Frame) {
        // Emprunts disjoints des champs pour satisfaire le borrow-checker.
        let Ui {
            mode,
            browser,
            tuner,
            picker,
            ..
        } = self;
        match mode {
            Mode::Tuner => {
                if let Some(t) = tuner.as_mut() {
                    t.draw(frame, picker);
                }
            }
            Mode::Browser => {
                let area = frame.area();
                browser.draw(frame, area, picker);
            }
        }
    }

    /// Renvoie true si l'application doit se fermer.
    fn handle_key(&mut self, code: KeyCode) -> bool {
        match self.mode {
            Mode::Tuner => {
                if let Some(t) = self.tuner.as_mut() {
                    match t.handle_key(code) {
                        TunerAction::Quit => return true,
                        TunerAction::OpenBrowser => {
                            self.cfg = t.cfg.clone(); // conserver les réglages
                            self.mode = Mode::Browser;
                        }
                        TunerAction::None => {}
                    }
                }
            }
            Mode::Browser => match self.browser.handle_key(code) {
                BrowserAction::Quit => return true,
                BrowserAction::Back => {
                    if self.tuner.is_some() {
                        self.mode = Mode::Tuner;
                    } else {
                        return true; // aucun tuner à retrouver -> on quitte
                    }
                }
                BrowserAction::Open(path) => {
                    let mut cfg = self.cfg.clone();
                    cfg.output = derive_output(&path);
                    cfg.input = path;
                    match build_tuner(cfg) {
                        Ok(app) => {
                            self.tuner = Some(app);
                            self.mode = Mode::Tuner;
                        }
                        Err(e) => self.browser.set_status(format!("Erreur : {e}")),
                    }
                }
                BrowserAction::None => {}
            },
        }
        false
    }
}

/// Lance la TUI. Si `start_in_browser`, ouvre l'explorateur ; sinon le tuner sur
/// l'image de `cfg.input`.
pub fn run(cfg: Config, start_in_browser: bool) -> Result<()> {
    // Interroge le terminal AVANT de passer en mode alterné ; repli demi-blocs.
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

    let mut ui = if start_in_browser {
        let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Ui {
            mode: Mode::Browser,
            browser: Browser::new(dir),
            tuner: None,
            cfg,
            picker,
        }
    } else {
        cfg.validate().map_err(anyhow::Error::msg)?;
        let app = build_tuner(cfg.clone())?;
        let dir = cfg
            .input
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Ui {
            mode: Mode::Tuner,
            browser: Browser::new(dir),
            tuner: Some(app),
            cfg,
            picker,
        }
    };

    let mut terminal = ratatui::init();
    let result = (|| -> Result<()> {
        loop {
            terminal.draw(|frame| ui.draw(frame))?;
            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press && ui.handle_key(key.code) {
                        break;
                    }
                }
            }
        }
        Ok(())
    })();
    ratatui::restore();
    result
}
