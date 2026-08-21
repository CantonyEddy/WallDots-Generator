//! Thème visuel partagé de la TUI : cadres arrondis et couleurs.
//!
//! IMPORTANT : on n'utilise QUE des couleurs ANSI (pas de RGB codé en dur), pour
//! que la TUI suive la palette du terminal — donc s'adapte aux thèmes dynamiques
//! (pywal, wallust…) qui recolorent le terminal selon le fond d'écran.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders};

/// Accent principal (couleur ANSI 6).
pub const ACCENT: Color = Color::Cyan;
/// Accent secondaire pour les valeurs (couleur ANSI 4).
pub const ACCENT2: Color = Color::Blue;
/// Couleur des bordures (ANSI « bright black »).
pub const BORDER: Color = Color::DarkGray;

/// Style d'une ligne sélectionnée : inversion vidéo (aucune couleur supposée,
/// s'adapte donc à n'importe quelle palette).
pub fn selected_style(base: Style) -> Style {
    base.add_modifier(Modifier::REVERSED | Modifier::BOLD)
}

/// Cadre arrondi avec titre en accent.
pub fn panel(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Line::from(Span::styled(
            title.to_string(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )))
}
