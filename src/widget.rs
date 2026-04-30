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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn toast_display_text_single() {
        let toast = Toast::new("hello", ToastType::Info);
        assert_eq!(toast.display_text(), "hello");
        assert_eq!(toast.count, 1);
    }

    #[test]
    fn toast_display_text_with_count() {
        let mut toast = Toast::new("error connecting", ToastType::Error);
        toast.increment_count();
        assert_eq!(toast.count, 2);
        assert_eq!(toast.display_text(), "error connecting (x2)");
    }

    #[test]
    fn toast_increment_count_multiple() {
        let mut toast = Toast::new("test", ToastType::Warning);
        toast.increment_count();
        toast.increment_count();
        toast.increment_count();
        assert_eq!(toast.count, 4);
        assert_eq!(toast.display_text(), "test (x4)");
    }

    #[test]
    fn toast_display_text_different_types() {
        let toast = Toast::new("done", ToastType::Success);
        assert_eq!(toast.display_text(), "done");
        assert_eq!(toast.type_ as usize, ToastType::Success as usize);
    }

    #[test]
    fn toast_render_does_not_panic() {
        let toast = Toast::new("test", ToastType::Info);
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        toast.render_ref(area, &mut buf);
    }
}
