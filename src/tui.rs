//! Interface interactive (TUI) pour régler les paramètres avec un aperçu en
//! temps réel, puis exporter. Réutilise entièrement le cœur (config/pipeline).
//!
//! L'aperçu est rendu en demi-blocs Unicode (`▀`) : chaque cellule du terminal
//! encode deux pixels verticaux (couleur de premier plan = pixel haut, couleur
//! de fond = pixel bas). C'est portable sur n'importe quel terminal truecolor,
//! sans dépendre d'un protocole graphique (kitty/sixel).

use crate::config::{BwMode, Config, DotShape, Rgb};
use crate::{grid, pipeline, preprocess, render};
use anyhow::Result;
use image::{imageops::FilterType, RgbImage};
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::time::Duration;
use tiny_skia::{Pixmap, PixmapPaint, Transform};

/// Taille max (en px) de l'image de travail servant à l'aperçu.
const PREVIEW_SRC_MAX: u32 = 480;
/// Nombre de paramètres réglables.
const PARAM_COUNT: usize = 10;

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
    /// Cache de l'aperçu : (largeur, hauteur, lignes). Invalidé au moindre réglage.
    cache: Option<(u16, u16, Vec<Line<'static>>)>,
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
            cache: None,
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
            _ => "",
        }
    }

    fn value(&self, i: usize) -> String {
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
            9 => self.bg_options[self.bg_index].0.clone(),
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
            _ => {}
        }
        self.cache = None; // un réglage invalide l'aperçu
    }

    /// Renvoie true si l'application doit se fermer.
    fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = (self.selected + PARAM_COUNT - 1) % PARAM_COUNT;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1) % PARAM_COUNT;
            }
            KeyCode::Left | KeyCode::Char('h') => self.adjust(-1),
            KeyCode::Right | KeyCode::Char('l') => self.adjust(1),
            KeyCode::Char(' ') | KeyCode::Enter => self.adjust(1),
            KeyCode::Char('z') => {
                self.zoom = !self.zoom;
                self.cache = None;
            }
            KeyCode::Char('s') => self.save(),
            _ => {}
        }
        false
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

    /// Construit les lignes de l'aperçu pour une zone de `w`×`h` cellules.
    fn build_preview(&self, w: u16, h: u16) -> Vec<Line<'static>> {
        let cw = w.max(1) as u32;
        let ch = (h.max(1) as u32) * 2; // deux pixels verticaux par cellule

        let sw = self.preview_src.width() as f32;
        let sh = self.preview_src.height() as f32;
        let src_aspect = sw / sh;
        let canvas_aspect = cw as f32 / ch as f32;

        // Ajuste l'image dans le canevas en conservant les proportions.
        let (fw, fh) = if src_aspect > canvas_aspect {
            (cw, ((cw as f32) / src_aspect).round().max(1.0) as u32)
        } else {
            (((ch as f32) * src_aspect).round().max(1.0) as u32, ch)
        };

        // Config d'aperçu.
        // - mode ajusté : l'image entière tient dans le canevas (~fw de large).
        // - mode zoom : chaque cellule fait ZOOM_CELL_PX pixels pour que la forme
        //   des points soit visible ; le canevas montre alors un recadrage centré.
        let mut pcfg = self.cfg.clone();
        pcfg.scale = if self.zoom {
            const ZOOM_CELL_PX: f32 = 8.0;
            (self.cfg.cols as f32 * ZOOM_CELL_PX) / sw
        } else {
            fw as f32 / sw
        };

        let mut img = self.preview_src.clone();
        preprocess::apply(&mut img, pcfg.bw, pcfg.threshold);
        let dot_grid = grid::build(&img, &pcfg);

        let bg = self.cfg.background;
        let canvas = (|| -> Option<Pixmap> {
            let gp = render::render_pixmap(&dot_grid, bg, self.cfg.shape).ok()?;
            let mut canvas = Pixmap::new(cw, ch)?;
            canvas.fill(tiny_skia::Color::from_rgba8(bg.r, bg.g, bg.b, 255));
            // Centrage signé : en zoom, gp est plus grand que le canevas et
            // l'offset négatif produit un recadrage centré (draw_pixmap clippe).
            let ox = (cw as i32 - gp.width() as i32) / 2;
            let oy = (ch as i32 - gp.height() as i32) / 2;
            canvas.draw_pixmap(
                ox,
                oy,
                gp.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
            Some(canvas)
        })();

        let Some(canvas) = canvas else {
            return vec![Line::from("aperçu indisponible")];
        };

        let _ = fh; // (info de cadrage, non utilisée directement)
        let mut lines = Vec::with_capacity(h as usize);
        for cy in 0..h as u32 {
            let mut spans = Vec::with_capacity(cw as usize);
            for cx in 0..cw {
                let top = canvas
                    .pixel(cx, cy * 2)
                    .map(|p| p.demultiply())
                    .unwrap_or_else(|| tiny_skia::ColorU8::from_rgba(bg.r, bg.g, bg.b, 255));
                let bot = canvas
                    .pixel(cx, cy * 2 + 1)
                    .map(|p| p.demultiply())
                    .unwrap_or_else(|| tiny_skia::ColorU8::from_rgba(bg.r, bg.g, bg.b, 255));
                spans.push(Span::styled(
                    "▀",
                    Style::default()
                        .fg(Color::Rgb(top.red(), top.green(), top.blue()))
                        .bg(Color::Rgb(bot.red(), bot.green(), bot.blue())),
                ));
            }
            lines.push(Line::from(spans));
        }
        lines
    }

    fn draw(&mut self, frame: &mut Frame) {
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
        self.draw_preview(frame, cols[1]);
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
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{label:<24}"), style),
                Span::raw(" "),
                Span::styled(value, Style::default().fg(Color::Yellow)),
            ]));
        }
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Paramètres ");
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn draw_preview(&mut self, frame: &mut Frame, area: Rect) {
        let title = if self.zoom {
            " Aperçu — zoom (recadrage central) "
        } else {
            " Aperçu "
        };
        let block = Block::default().borders(Borders::ALL).title(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let need = match &self.cache {
            Some((w, h, _)) => *w != inner.width || *h != inner.height,
            None => true,
        };
        if need {
            let lines = self.build_preview(inner.width, inner.height);
            self.cache = Some((inner.width, inner.height, lines));
        }
        if let Some((_, _, lines)) = &self.cache {
            frame.render_widget(Paragraph::new(Text::from(lines.clone())), inner);
        }
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let help = Line::from(vec![
            Span::styled("↑↓", Style::default().fg(Color::Cyan)),
            Span::raw(" paramètre  "),
            Span::styled("←→", Style::default().fg(Color::Cyan)),
            Span::raw(" ajuster  "),
            Span::styled("espace", Style::default().fg(Color::Cyan)),
            Span::raw(" cycler  "),
            Span::styled("z", Style::default().fg(Color::Cyan)),
            Span::raw(" zoom  "),
            Span::styled("s", Style::default().fg(Color::Cyan)),
            Span::raw(" sauver  "),
            Span::styled("q", Style::default().fg(Color::Cyan)),
            Span::raw(" quitter"),
        ]);
        let status = Line::from(Span::styled(
            self.status.clone(),
            Style::default().fg(Color::Green),
        ));
        frame.render_widget(
            Paragraph::new(vec![help, status])
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: true }),
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
    fn preview_has_expected_shape() {
        // Image de test avec un dégradé (donc des points de tailles variées).
        let mut img = RgbImage::new(200, 120);
        for (x, _y, p) in img.enumerate_pixels_mut() {
            let v = (x * 255 / 200) as u8;
            *p = image::Rgb([v, v, v]);
        }
        let app = App::new(cfg(), img);
        let (w, h) = (40u16, 20u16);
        let lines = app.build_preview(w, h);
        assert_eq!(lines.len(), h as usize, "une ligne par rangée de cellules");
        for line in &lines {
            assert_eq!(line.spans.len(), w as usize, "un span par colonne");
        }
    }

    #[test]
    fn full_frame_draws_and_shows_preview() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut img = RgbImage::new(160, 90);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let v = (((x + y) * 255) / 250) as u8;
            *p = image::Rgb([v, v / 2, 255 - v]);
        }
        let mut app = App::new(cfg(), img);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| app.draw(f)).unwrap();

        let buf = terminal.backend().buffer();
        // L'aperçu utilise le demi-bloc « ▀ » : il doit apparaître à l'écran.
        let has_halfblock = buf.content().iter().any(|c| c.symbol() == "▀");
        assert!(has_halfblock, "l'aperçu devrait contenir des demi-blocs");
        // Et le panneau de paramètres doit afficher au moins un libellé.
        let dump: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(dump.contains("Grille"), "le panneau de paramètres manque");
    }

    #[test]
    fn zoom_preview_differs_by_shape() {
        // Image claire et uniforme -> gros points, forme visible en zoom.
        let img = RgbImage::from_pixel(160, 100, image::Rgb([230, 230, 230]));
        let mut circle = App::new(cfg(), img.clone());
        circle.zoom = true;
        circle.cfg.shape = DotShape::Circle;
        let mut square = App::new(cfg(), img);
        square.zoom = true;
        square.cfg.shape = DotShape::Square;

        let (w, h) = (60u16, 30u16);
        let lc = circle.build_preview(w, h);
        let ls = square.build_preview(w, h);
        assert_ne!(
            lc, ls,
            "en zoom, cercle et carré doivent produire des aperçus différents"
        );
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

/// Lance la TUI. Charge l'image, prépare l'aperçu, puis boucle jusqu'à `q`.
pub fn run(cfg: Config) -> Result<()> {
    cfg.validate().map_err(anyhow::Error::msg)?;
    let full = image::open(&cfg.input)
        .map_err(|e| anyhow::anyhow!("ouverture de l'image {} : {e}", cfg.input.display()))?
        .to_rgb8();
    let preview_src = downscale(&full, PREVIEW_SRC_MAX);
    drop(full);

    let mut app = App::new(cfg, preview_src);
    let mut terminal = ratatui::init();
    let result = (|| -> Result<()> {
        loop {
            terminal.draw(|frame| app.draw(frame))?;
            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press && app.handle_key(key.code) {
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
