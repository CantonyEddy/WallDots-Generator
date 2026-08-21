//! Explorateur de fichiers (type yazi) : navigation dossiers/images avec aperçu
//! de l'image survolée (rendu haute qualité via ratatui-image). Sélectionner
//! une image la charge dans le tuner.

use crate::theme::{panel, selected_style, ACCENT};
use image::{imageops::FilterType, DynamicImage, RgbImage};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use std::path::{Path, PathBuf};

/// Extensions considérées comme des images.
const IMG_EXT: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff", "avif", "qoi", "tga", "ppm",
];
/// Taille max de l'image de travail pour l'aperçu.
const PREVIEW_MAX: u32 = 480;

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMG_EXT.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

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

/// Une entrée du dossier courant.
struct Entry {
    label: String,
    path: PathBuf,
    is_dir: bool,
    /// Entrée « .. » (remonter).
    parent: bool,
}

/// Résultat du traitement d'une touche.
pub enum Action {
    /// Rien de particulier.
    None,
    /// L'utilisateur a choisi une image.
    Open(PathBuf),
    /// Quitter l'application.
    Quit,
    /// Revenir au tuner (si une image est déjà chargée).
    Back,
}

pub struct Browser {
    cwd: PathBuf,
    entries: Vec<Entry>,
    /// Indices de `entries` correspondant au filtre courant (= tout si vide).
    filtered: Vec<usize>,
    /// Position dans `filtered`.
    selected: usize,
    status: String,
    /// Texte du filtre de recherche.
    query: String,
    /// Saisie de recherche active (les lettres alimentent la requête).
    searching: bool,
    /// Protocole d'image de l'aperçu et chemin qu'il représente.
    protocol: Option<StatefulProtocol>,
    proto_path: Option<PathBuf>,
}

impl Browser {
    pub fn new(start: PathBuf) -> Self {
        let cwd = if start.is_dir() {
            start
        } else {
            start.parent().map(Path::to_path_buf).unwrap_or(start)
        };
        let mut b = Browser {
            cwd,
            entries: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            status: "Choisis une image (↵), « / » pour filtrer.".to_string(),
            query: String::new(),
            searching: false,
            protocol: None,
            proto_path: None,
        };
        b.reload();
        b
    }

    pub fn set_status(&mut self, s: String) {
        self.status = s;
    }

    /// Entrée actuellement survolée (à travers le filtre).
    fn current(&self) -> Option<&Entry> {
        self.filtered.get(self.selected).and_then(|&i| self.entries.get(i))
    }

    /// Recalcule la liste filtrée selon `query`.
    fn apply_filter(&mut self) {
        let q = self.query.to_lowercase();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| q.is_empty() || e.label.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.filtered.len() {
            self.selected = 0;
        }
    }

    /// Sort du mode recherche et efface le filtre.
    fn exit_search(&mut self) {
        self.query.clear();
        self.searching = false;
    }

    fn reload(&mut self) {
        let mut dirs: Vec<Entry> = Vec::new();
        let mut files: Vec<Entry> = Vec::new();

        if let Ok(rd) = std::fs::read_dir(&self.cwd) {
            for e in rd.flatten() {
                let path = e.path();
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    continue; // on masque les fichiers cachés
                }
                let is_dir = path.is_dir();
                if is_dir {
                    dirs.push(Entry {
                        label: format!("{name}/"),
                        path,
                        is_dir: true,
                        parent: false,
                    });
                } else if is_image(&path) {
                    files.push(Entry {
                        label: name,
                        path,
                        is_dir: false,
                        parent: false,
                    });
                }
            }
        } else {
            self.status = format!("dossier illisible : {}", self.cwd.display());
        }

        dirs.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
        files.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));

        let mut entries = Vec::with_capacity(dirs.len() + files.len() + 1);
        if let Some(parent) = self.cwd.parent() {
            entries.push(Entry {
                label: "../".to_string(),
                path: parent.to_path_buf(),
                is_dir: true,
                parent: true,
            });
        }
        entries.extend(dirs);
        entries.extend(files);

        self.entries = entries;
        self.selected = 0;
        self.apply_filter();
    }

    fn move_sel(&mut self, delta: i32) {
        if self.filtered.is_empty() {
            return;
        }
        let n = self.filtered.len() as i32;
        self.selected = (((self.selected as i32 + delta) % n + n) % n) as usize;
    }

    fn open_parent(&mut self) {
        if let Some(parent) = self.cwd.parent().map(Path::to_path_buf) {
            self.cwd = parent;
            self.exit_search();
            self.reload();
        }
    }

    /// Active l'entrée courante : entre dans un dossier ou renvoie l'image choisie.
    fn activate(&mut self) -> Action {
        let Some(e) = self.current() else {
            return Action::None;
        };
        let (parent, is_dir, path) = (e.parent, e.is_dir, e.path.clone());
        if parent {
            self.open_parent();
            Action::None
        } else if is_dir {
            self.cwd = path;
            self.exit_search();
            self.reload();
            Action::None
        } else {
            Action::Open(path)
        }
    }

    pub fn handle_key(&mut self, code: ratatui::crossterm::event::KeyCode) -> Action {
        use ratatui::crossterm::event::KeyCode::*;

        // Mode saisie de recherche : les lettres alimentent la requête.
        if self.searching {
            match code {
                Esc => {
                    self.exit_search();
                    self.apply_filter();
                }
                Enter => return self.activate(),
                Backspace => {
                    self.query.pop();
                    self.selected = 0;
                    self.apply_filter();
                }
                Up => self.move_sel(-1),
                Down => self.move_sel(1),
                Right => return self.activate(),
                Char(c) => {
                    self.query.push(c);
                    self.selected = 0;
                    self.apply_filter();
                }
                _ => {}
            }
            return Action::None;
        }

        // Mode navigation.
        match code {
            Char('/') => {
                self.searching = true;
                Action::None
            }
            Char('q') => Action::Quit,
            Esc => Action::Back,
            Up | Char('k') => {
                self.move_sel(-1);
                Action::None
            }
            Down | Char('j') => {
                self.move_sel(1);
                Action::None
            }
            Enter | Right | Char('l') => self.activate(),
            Left | Char('h') | Backspace => {
                self.open_parent();
                Action::None
            }
            _ => Action::None,
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, picker: &mut Picker) {
        use ratatui::layout::{Constraint, Direction, Layout};
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(2)])
            .split(area);
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(38), Constraint::Min(10)])
            .split(root[0]);
        // Colonne de gauche : barre de recherche (au-dessus) + arborescence.
        let left = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(cols[0]);

        self.draw_search(frame, left[0]);
        self.draw_list(frame, left[1]);
        self.draw_preview(frame, cols[1], picker);
        self.draw_footer(frame, root[1]);
    }

    fn draw_search(&self, frame: &mut Frame, area: Rect) {
        let block = panel(" Recherche ");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let content = if self.searching {
            Line::from(vec![
                Span::raw(self.query.clone()),
                Span::styled("▏", Style::default().fg(ACCENT)),
            ])
        } else if !self.query.is_empty() {
            Line::from(vec![
                Span::raw(format!("{}  ", self.query)),
                Span::styled("(Échap efface)", Style::default().fg(Color::DarkGray)),
            ])
        } else {
            Line::from(Span::styled(
                "« / » pour filtrer",
                Style::default().fg(Color::DarkGray),
            ))
        };
        frame.render_widget(Paragraph::new(content), inner);
    }

    fn draw_list(&self, frame: &mut Frame, area: Rect) {
        let title = if self.query.is_empty() {
            format!(" {} ", self.cwd.display())
        } else {
            format!(" {}  [{} résultats] ", self.cwd.display(), self.filtered.len())
        };
        let block = panel(&title);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let height = inner.height as usize;
        if height == 0 {
            return;
        }
        // Fenêtre de défilement centrée sur la sélection.
        let start = self.selected.saturating_sub(height / 2);
        let start = start.min(self.filtered.len().saturating_sub(height));

        let mut lines: Vec<Line> = Vec::new();
        for (row, &ei) in self.filtered.iter().enumerate().skip(start).take(height) {
            let e = &self.entries[ei];
            let selected = row == self.selected;
            let base = if e.is_dir {
                Style::default().fg(ACCENT)
            } else {
                Style::default()
            };
            let style = if selected {
                selected_style(base)
            } else {
                base
            };
            let marker = if selected { "▶ " } else { "  " };
            lines.push(Line::from(Span::styled(format!("{marker}{}", e.label), style)));
        }
        if self.filtered.is_empty() {
            let msg = if self.query.is_empty() {
                "  (aucune image ici)"
            } else {
                "  (aucun résultat)"
            };
            lines.push(Line::from(Span::styled(
                msg,
                Style::default().fg(Color::DarkGray),
            )));
        }
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn draw_preview(&mut self, frame: &mut Frame, area: Rect, picker: &mut Picker) {
        let block = panel(" Aperçu ");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.width == 0 || inner.height == 0 {
            return;
        }

        // Chemin de l'image survolée (None si dossier ou entrée absente).
        let sel_path = self
            .current()
            .filter(|e| !e.is_dir)
            .map(|e| e.path.clone());

        match sel_path {
            Some(path) => {
                if self.proto_path.as_ref() != Some(&path) {
                    match image::open(&path) {
                        Ok(img) => {
                            let small = downscale(&img.to_rgb8(), PREVIEW_MAX);
                            self.protocol =
                                Some(picker.new_resize_protocol(DynamicImage::ImageRgb8(small)));
                            self.proto_path = Some(path);
                        }
                        Err(_) => {
                            self.protocol = None;
                            self.proto_path = None;
                        }
                    }
                }
                if let Some(proto) = self.protocol.as_mut() {
                    // Crop = l'image couvre toute la zone (remplit l'espace).
                    let widget = StatefulImage::default().resize(Resize::Crop(None));
                    frame.render_stateful_widget(widget, inner, proto);
                }
            }
            None => {
                self.protocol = None;
                self.proto_path = None;
                frame.render_widget(
                    Paragraph::new(Span::styled(
                        "dossier — entre avec ↵",
                        Style::default().fg(Color::DarkGray),
                    )),
                    inner,
                );
            }
        }
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let help = Line::from(vec![
            Span::styled("↑↓", Style::default().fg(ACCENT)),
            Span::raw(" naviguer  "),
            Span::styled("↵/→", Style::default().fg(ACCENT)),
            Span::raw(" ouvrir/choisir  "),
            Span::styled("←", Style::default().fg(ACCENT)),
            Span::raw(" remonter  "),
            Span::styled("/", Style::default().fg(ACCENT)),
            Span::raw(" filtrer  "),
            Span::styled("Échap", Style::default().fg(ACCENT)),
            Span::raw(" retour  "),
            Span::styled("q", Style::default().fg(ACCENT)),
            Span::raw(" quitter"),
        ]);
        let status = Line::from(Span::styled(
            self.status.clone(),
            Style::default().fg(Color::Green),
        ));
        frame.render_widget(Paragraph::new(vec![help, status]), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyCode;

    /// Prépare un dossier temporaire : un sous-dossier, une image, un non-image.
    fn fixture(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("walldots_browser_{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        RgbImage::from_pixel(8, 8, image::Rgb([120, 30, 200]))
            .save(dir.join("photo.png"))
            .unwrap();
        std::fs::write(dir.join("note.txt"), b"pas une image").unwrap();
        dir
    }

    #[test]
    fn lists_dirs_and_images_only() {
        let dir = fixture("list");
        let b = Browser::new(dir);
        let labels: Vec<&str> = b.entries.iter().map(|e| e.label.as_str()).collect();
        assert!(labels.contains(&"../")); // remontée présente
        assert!(labels.contains(&"sub/")); // sous-dossier
        assert!(labels.contains(&"photo.png")); // image
        assert!(!labels.iter().any(|l| l.contains("note.txt"))); // non-image exclu
    }

    #[test]
    fn entering_dir_and_selecting_image() {
        let dir = fixture("nav");
        let mut b = Browser::new(dir.clone());

        // Sélectionner puis activer l'image -> Open avec le bon chemin.
        let idx = b.entries.iter().position(|e| e.label == "photo.png").unwrap();
        b.selected = idx;
        match b.handle_key(KeyCode::Enter) {
            Action::Open(p) => assert_eq!(p, dir.join("photo.png")),
            _ => panic!("devrait renvoyer Open"),
        }

        // Entrer dans le sous-dossier change le cwd.
        let sidx = b.entries.iter().position(|e| e.label == "sub/").unwrap();
        b.selected = sidx;
        let _ = b.handle_key(KeyCode::Enter);
        assert_eq!(b.cwd, dir.join("sub"));
    }

    #[test]
    fn search_filters_entries() {
        let dir = fixture("search");
        RgbImage::from_pixel(4, 4, image::Rgb([0, 0, 0]))
            .save(dir.join("banana.png"))
            .unwrap();
        let mut b = Browser::new(dir);

        assert!(matches!(b.handle_key(KeyCode::Char('/')), Action::None));
        assert!(b.searching);
        for c in "phot".chars() {
            b.handle_key(KeyCode::Char(c));
        }
        let labels: Vec<String> = b.filtered.iter().map(|&i| b.entries[i].label.clone()).collect();
        assert!(labels.iter().any(|l| l == "photo.png"), "photo doit rester");
        assert!(!labels.iter().any(|l| l == "banana.png"), "banana doit être filtré");

        // Échap efface le filtre.
        b.handle_key(KeyCode::Esc);
        assert!(!b.searching && b.query.is_empty());
        assert!(b.filtered.len() >= 2, "tout revient après effacement");
    }

    #[test]
    fn full_frame_draws() {
        use ratatui::{backend::TestBackend, Terminal};
        let dir = fixture("draw");
        let mut b = Browser::new(dir);
        let mut picker = Picker::halfblocks();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let area = terminal.get_frame().area();
        terminal.draw(|f| b.draw(f, area, &mut picker)).unwrap();
        let dump: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(dump.contains("photo.png"), "la liste devrait montrer l'image");
    }
}
