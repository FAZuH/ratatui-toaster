use ratatui::{
    style::Style,
    symbols::{self},
    widgets::{Block, Borders, Padding, Paragraph, Widget, WidgetRef},
};

use crate::engine::ToastType;

/// A simple widget that represents a toast message. It displays a message with a border colored according to the toast type.
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub type_: ToastType,
    pub count: u32,
}

impl Toast {
    /// Creates a new `Toast` widget with the given message and type.
    pub fn new(message: &str, type_: ToastType) -> Self {
        Self {
            message: message.to_string(),
            type_,
            count: 1,
        }
    }

    /// Increment the duplicate count.
    pub fn increment_count(&mut self) {
        self.count += 1;
    }

    pub(crate) fn display_text(&self) -> String {
        if self.count > 1 {
            format!("{} (x{})", self.message, self.count)
        } else {
            self.message.clone()
        }
    }
}

impl WidgetRef for Toast {
    fn render_ref(&self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        const PADDING: u16 = 1;
        let paragraph = Paragraph::new(self.display_text()).block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT)
                .border_set(symbols::border::QUADRANT_OUTSIDE)
                .padding(Padding::uniform(PADDING))
                .border_style(Style::default().fg(self.type_.into())),
        );
        paragraph.render(area, buf);
    }
}
