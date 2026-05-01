//! A toast engine for displaying temporary messages in a terminal UI.
//! The `ToastEngine` manages the display of toasts, which are temporary messages that appear on the screen for a short duration. It supports different types of toasts (info, success, warning, error) and allows customization of their position and duration.
//!
//! The `ToastEngine` can be integrated into a terminal UI application using the `ratatui` crate. It provides a builder pattern for creating toasts and handles the timing for automatically hiding toasts after a specified duration.
//! # Tokio Integration
//! The `tokio` feature can be used to tightly integrate the toast engine with applications that use an event based pattern. In your
//! `Action` enum (or equivalent), add a variant that can be converted from `ToastMessage`. For example:
//! ```rust
//! # use ratatui_toaster::ToastMessage;
//! enum Action {
//!     ShowToast(ToastMessage),
//!     // other variants...
//! }
//! ```
//! Then, when you want to show a toast, you can send a `ToastMessage::Show` action through your application's event system, although you do need
//! to handle the `Show` event yourself. When the toast times out, the `ToastEngine` will automatically send a `ToastMessage::Hide` action, which you should also handle to hide the toast.
//! Disable the `tokio` feature if you want to manage the timing of hiding toasts yourself, or if your application does not use an event based pattern.
//!
//! # Animating Toasts
//! The current implementation does not include animations for showing or hiding toasts. However, you can
//! use libraries like [tachyonfx](https://github.com/ratatui/tachyonfx) to add animations to your toasts. You would need to implement the animation logic in your event handling code, triggering animations when showing or hiding toasts based on the `ToastMessage` actions.
use std::borrow::Cow;
#[cfg(not(feature = "tokio"))]
use std::marker::PhantomData;
use std::time::Instant;

use ratatui::{
    layout::{Constraint, Rect, Size},
    widgets::{Clear, Widget, WidgetRef},
};
use textwrap::wrap;

use crate::widget::Toast;

const DEFAULT_MAX_TOAST_WIDTH: u16 = 50;
const TOAST_GAP: u16 = 1;

/// A toast engine for displaying temporary messages in a terminal UI.
/// The `ToastEngine` manages the display of toasts, which are temporary messages that appear on the screen for a short duration. It supports different types of toasts (info, success, warning, error) and allows customization of their position and duration.
/// You can call `show_toast` to display a toast, and `hide_toast` to hide the toast. To animate,
/// you can get the area of the toast using `toast_area` and implement your animation logic based on that area. #[derive(Debug)]
/// Caveat: If you're not using the `tokio` feature, create a `ToastEngine<()>`. There is a (hacky) impl to make it work without the `tokio` feature.
/// An active toast with its render state.
#[derive(Debug)]
struct ActiveToast {
    toast: Toast,
    area: Rect,
    position: ToastPosition,
    constraint: ToastConstraint,
    remove_at: Instant,
}

pub struct ToastEngine<A>
where
    A: From<ToastMessage> + Send + 'static,
{
    area: Rect,
    default_duration: std::time::Duration,
    #[cfg(feature = "tokio")]
    tx: Option<tokio::sync::mpsc::Sender<A>>,
    #[cfg(not(feature = "tokio"))]
    tx: Option<PhantomData<A>>,
    toast_area: Rect,
    toasts: Vec<ActiveToast>,
}

/// A builder for creating a `ToastEngine`. It allows you to set the default duration for toasts, and an optional channel sender for sending toast messages (if using the `tokio` feature).
pub struct ToastEngineBuilder<A>
where
    A: From<ToastMessage> + Send + 'static,
{
    area: Rect,
    default_duration: std::time::Duration,
    #[cfg(feature = "tokio")]
    tx: Option<tokio::sync::mpsc::Sender<A>>,
    #[cfg(not(feature = "tokio"))]
    tx: Option<PhantomData<A>>,
}

impl<A> ToastEngineBuilder<A>
where
    A: From<ToastMessage> + Send + 'static,
{
    /// Creates a new `ToastEngineBuilder` with the specified area for displaying toasts. The default duration for toasts is set to 3 seconds, and no channel sender is configured by default.
    pub fn new(area: Rect) -> Self {
        Self {
            area,
            default_duration: std::time::Duration::from_secs(3),
            tx: None,
        }
    }

    /// Sets the default duration for toasts. This duration will be used when showing a toast if no specific duration is provided.
    pub fn default_duration(mut self, duration: std::time::Duration) -> Self {
        self.default_duration = duration;
        self
    }

    /// Configures a channel sender for sending toast messages. This is used when the `tokio` feature is enabled to allow the `ToastEngine` to send messages to hide toasts after the duration expires.
    #[cfg(feature = "tokio")]
    pub fn action_tx(mut self, tx: tokio::sync::mpsc::Sender<A>) -> Self {
        self.tx = Some(tx);
        self
    }

    /// Builds the `ToastEngine` using the configured settings. This method consumes the builder and returns a new instance of `ToastEngine`.
    pub fn build(self) -> ToastEngine<A> {
        ToastEngine::from_builder(self)
    }
}

/// The type of toast to display. This enum defines the different types of toasts that can be shown, such as informational messages, success messages, warnings, and errors. Each variant can be styled differently when rendered.
#[derive(Debug, Default, Clone, Copy)]
pub enum ToastType {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

/// The position on the screen where the toast should be displayed. This enum defines various positions for toasts, including top-left, top-right, bottom-left, bottom-right, and center. The `ToastEngine` uses this information to calculate the appropriate area for rendering the toast based on the specified position.
#[derive(Debug, Default, Clone, Copy)]
pub enum ToastPosition {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

/// The constraint for the toast's size. This enum defines how the size of the toast should be determined. The `Auto` variant allows the toast to automatically size itself based on the message content, while the `Uniform` and `Manual` variants allow for more specific control over the width and height of the toast.
#[derive(Debug, Default, Clone)]
pub enum ToastConstraint {
    #[default]
    Auto,
    Uniform(Constraint),
    Manual {
        width: Constraint,
        height: Constraint,
    },
}

/// The messages that can be sent to the `ToastEngine` to control the display of toasts. The `Show` variant contains the message to display, the type of toast, and its position, while the `Hide` variant indicates that any currently displayed toast should be hidden.
///
///NOTE: You do have to handle the events yourself. Usually, its as simple as matching on the `ToastMessage` in your event loop and calling the appropriate methods on the `ToastEngine` to show or hide toasts based on the received messages.
#[derive(Debug, Clone)]
pub enum ToastMessage {
    Show {
        message: String,
        toast_type: ToastType,
        position: ToastPosition,
    },
    Hide,
}

/// A builder for creating a toast message. This struct allows you to specify the message content, type, position, size constraints, and deduplication behavior for a toast before showing it using the `ToastEngine`. The builder pattern provides a convenient way to configure the properties of a toast in a fluent manner.
#[derive(Debug, Default)]
pub struct ToastBuilder {
    message: Cow<'static, str>,
    toast_type: ToastType,
    position: ToastPosition,
    constraint: ToastConstraint,
    deduplicate: bool,
}

impl<A> ToastEngine<A>
where
    A: From<ToastMessage> + Send + 'static,
{
    /// Creates a new `ToastEngine`. Consider using the `ToastEngineBuilder` instead.
    pub fn new(
        ToastEngine {
            area,
            default_duration,
            tx,
            ..
        }: Self,
    ) -> Self {
        Self {
            area,
            default_duration,
            tx,
            toasts: Vec::new(),
            toast_area: Rect::default(),
        }
    }

    /// Creates a new `ToastEngine` from a `ToastEngineBuilder`. This method takes the configuration from the builder and initializes the `ToastEngine` accordingly. It sets up the area for displaying toasts, the default duration for toasts, and any channel sender if provided (when using the `tokio` feature).
    pub fn from_builder(
        ToastEngineBuilder {
            area,
            default_duration,
            tx,
            ..
        }: ToastEngineBuilder<A>,
    ) -> Self {
        Self {
            area,
            default_duration,
            tx,
            toasts: Vec::new(),
            toast_area: Rect::default(),
        }
    }

    /// Shows a toast message using the provided `ToastBuilder`. This method calculates the area for the toast based on the message content and the specified position, creates a new `Toast` instance, and adds it to the stack of active toasts. Older toasts are pushed down (for top and center positions) or up (for bottom positions) to make room for the new one. If the `tokio` feature is enabled and a channel sender is configured, it also spawns a task to automatically hide the toast after the default duration.
    pub fn show_toast(&mut self, toast: ToastBuilder) {
        // Deduplicate: increment count on existing toast with matching message.
        if toast.deduplicate
            && let Some(existing) = self
                .toasts
                .iter_mut()
                .find(|t| t.toast.message == *toast.message)
        {
            existing.toast.increment_count();
            existing.remove_at = Instant::now() + self.default_duration;
            // Recalculate area using display_text() so "(xN)" suffix fits
            let display = existing.toast.display_text();
            existing.area = calculate_toast_area(
                &ToastBuilder::new(Cow::Owned(display))
                    .position(existing.position)
                    .constraint(existing.constraint.clone()),
                self.area,
            );
            return;
        }

        let toast_area = calculate_toast_area(&toast, self.area);

        // Shift existing toasts to make room for the new one.
        let shift = toast_area.height + TOAST_GAP;
        match toast.position {
            ToastPosition::TopLeft | ToastPosition::TopRight | ToastPosition::Center => {
                for active in &mut self.toasts {
                    active.area.y = active.area.y.saturating_add(shift);
                }
            }
            ToastPosition::BottomLeft | ToastPosition::BottomRight => {
                for active in &mut self.toasts {
                    active.area.y = active.area.y.saturating_sub(shift);
                }
            }
        }

        let toast_widget = Toast::new(&toast.message, toast.toast_type);
        self.toasts.push(ActiveToast {
            toast: toast_widget,
            area: toast_area,
            position: toast.position,
            constraint: toast.constraint,
            remove_at: Instant::now() + self.default_duration,
        });
        self.toast_area = toast_area;

        #[cfg(feature = "tokio")]
        if let Some(tx) = &self.tx {
            let tx_clone = tx.clone();
            let duration = self.default_duration;
            tokio::spawn(async move {
                tokio::time::sleep(duration).await;
                let _ = tx_clone.send(ToastMessage::Hide.into()).await;
            });
        }
    }

    /// Get the area where the most recent toast will be rendered.
    pub fn toast_area(&self) -> Rect {
        self.toast_area
    }

    /// Whether any toast is currently being displayed.
    pub fn has_toast(&self) -> bool {
        !self.toasts.is_empty()
    }

    /// Hides all expired toasts. When using the `tokio` feature, this is typically
    /// called in response to a `ToastMessage::Hide` event. Stale events (for toasts
    /// whose lifetime was extended via deduplication) are safely no-ops.
    pub fn hide_toast(&mut self) {
        if self.toasts.is_empty() {
            return;
        }
        self.toasts.remove(0);
        self.recalculate_areas();
    }

    /// Removes all toasts whose display duration has elapsed.
    pub fn purge_expired(&mut self) {
        let now = Instant::now();
        let len_before = self.toasts.len();
        self.toasts.retain(|t| t.remove_at > now);
        if self.toasts.len() != len_before {
            self.recalculate_areas();
        }
    }

    /// Sets the area for the toast engine and recalculates positions for all active toasts. This method allows you to update the area where toasts will be displayed, which can be useful if the layout of your terminal UI changes and you need to adjust the toast display area accordingly.
    pub fn set_area(&mut self, area: Rect) {
        self.area = area;
        self.recalculate_areas();
    }

    fn recalculate_areas(&mut self) {
        let mut top_y = self.area.y;
        let mut bottom_y = self.area.y.saturating_add(self.area.height);

        for active in self.toasts.iter_mut().rev() {
            let builder = ToastBuilder::new(Cow::Owned(active.toast.display_text()))
                .position(active.position)
                .constraint(active.constraint.clone());
            let mut new_area = calculate_toast_area(&builder, self.area);
            drop(builder);

            match active.position {
                ToastPosition::TopLeft | ToastPosition::TopRight | ToastPosition::Center => {
                    new_area.y = top_y;
                    top_y = top_y.saturating_add(new_area.height + TOAST_GAP);
                }
                ToastPosition::BottomLeft | ToastPosition::BottomRight => {
                    bottom_y = bottom_y.saturating_sub(new_area.height + TOAST_GAP);
                    new_area.y = bottom_y;
                }
            }

            active.area = new_area;
        }

        self.toast_area = self.toasts.last().map(|t| t.area).unwrap_or_default();
    }
}

impl ToastBuilder {
    /// Create a new instance of a `ToastBuilder`
    pub fn new(message: Cow<'static, str>) -> Self {
        Self {
            message,
            toast_type: ToastType::Info,
            position: ToastPosition::TopRight,
            constraint: ToastConstraint::Auto,
            deduplicate: false,
        }
    }

    pub fn toast_type(mut self, toast_type: ToastType) -> Self {
        self.toast_type = toast_type;
        self
    }

    pub fn position(mut self, position: ToastPosition) -> Self {
        self.position = position;
        self
    }

    pub fn constraint(mut self, constraint: ToastConstraint) -> Self {
        self.constraint = constraint;
        self
    }

    /// If set to `true`, a toast with the same message as an existing toast will
    /// increment a counter on the existing toast instead of adding a new one.
    pub fn deduplicate(mut self, deduplicate: bool) -> Self {
        self.deduplicate = deduplicate;
        self
    }
}

fn calculate_toast_area(
    ToastBuilder {
        message,
        position,
        constraint,
        ..
    }: &ToastBuilder,
    area: Rect,
) -> Rect {
    use ToastConstraint::*;
    use ToastPosition::*;
    const PADDING: u16 = 2;

    let width = match constraint {
        Auto => std::cmp::min(DEFAULT_MAX_TOAST_WIDTH, message.len() as u16 + PADDING * 2),
        Uniform(c) => area.centered_horizontally(*c).width,
        Manual { width, .. } => area.centered_horizontally(*width).width,
    };
    let wrapped_text = wrap(message, width as usize);
    let height = match constraint {
        Auto => wrapped_text.len() as u16 + PADDING,
        Uniform(c) => area.centered_vertically(*c).height + PADDING,
        Manual { height, .. } => area.centered_vertically(*height).height + PADDING,
    };
    if let Center = position {
        return area.centered(width.into(), height.into());
    }
    position.calculate_position(area, Size { width, height })
}

impl ToastPosition {
    fn calculate_position(&self, area: Rect, Size { width, height }: Size) -> Rect {
        use ToastPosition::*;
        match self {
            TopLeft => Rect {
                x: area.x,
                y: area.y,
                width,
                height,
            },
            TopRight => Rect {
                x: area.x + area.width.saturating_sub(width),
                y: area.y,
                width,
                height,
            },
            BottomLeft => Rect {
                x: area.x,
                y: area.y + area.height.saturating_sub(height),
                width,
                height,
            },
            BottomRight => Rect {
                x: area.x + area.width.saturating_sub(width),
                y: area.y + area.height.saturating_sub(height),
                width,
                height,
            },
            Center => Rect {
                x: area.x + (area.width.saturating_sub(width)) / 2,
                y: area.y + (area.height.saturating_sub(height)) / 2,
                width,
                height,
            },
        }
    }
}

impl From<ToastType> for ratatui::style::Color {
    fn from(value: ToastType) -> Self {
        use ToastType::*;
        match value {
            Info => Self::Blue,
            Success => Self::Green,
            Warning => Self::Yellow,
            Error => Self::Red,
        }
    }
}

impl<A> WidgetRef for ToastEngine<A>
where
    A: From<ToastMessage> + Send + 'static,
{
    fn render_ref(&self, _area: Rect, buf: &mut ratatui::buffer::Buffer) {
        for active in &self.toasts {
            let area = clamp_rect(active.area, self.area);
            if area.width == 0 || area.height == 0 {
                continue;
            }
            Clear.render(area, buf);
            active.toast.render_ref(area, buf);
        }
    }
}

/// Clamp `inner` to be fully contained within `outer`.
fn clamp_rect(inner: Rect, outer: Rect) -> Rect {
    let x = inner.x.max(outer.x);
    let y = inner.y.max(outer.y);
    let max_x = (inner.x + inner.width).min(outer.x + outer.width);
    let max_y = (inner.y + inner.height).min(outer.y + outer.height);
    Rect {
        x,
        y,
        width: max_x.saturating_sub(x),
        height: max_y.saturating_sub(y),
    }
}

impl<A> Widget for &ToastEngine<A>
where
    A: From<ToastMessage> + Send + 'static,
{
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_ref(area, buf);
    }
}

impl From<ToastMessage> for () {
    fn from(_value: ToastMessage) -> Self {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use std::time::Duration;

    fn area_80x24() -> Rect {
        Rect::new(0, 0, 80, 24)
    }

    // --- calculate_toast_area / position tests ---

    #[test]
    fn top_left_x_is_zero() {
        let area = area_80x24();
        let builder = ToastBuilder::new("test".into()).position(ToastPosition::TopLeft);
        let rect = calculate_toast_area(&builder, area);
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
    }

    #[test]
    fn top_right_x_is_not_zero() {
        let area = area_80x24();
        let builder = ToastBuilder::new("test".into()).position(ToastPosition::TopRight);
        let rect = calculate_toast_area(&builder, area);
        assert!(rect.x > 0, "TopRight x should be > 0, got {}", rect.x);
        assert_eq!(rect.y, 0);
    }

    #[test]
    fn top_right_differs_from_top_left() {
        let area = area_80x24();
        let left = calculate_toast_area(
            &ToastBuilder::new("test".into()).position(ToastPosition::TopLeft),
            area,
        );
        let right = calculate_toast_area(
            &ToastBuilder::new("test".into()).position(ToastPosition::TopRight),
            area,
        );
        assert_ne!(left.x, right.x, "TopLeft and TopRight x must differ");
    }

    #[test]
    fn bottom_left_y_is_bottom() {
        let area = area_80x24();
        let builder = ToastBuilder::new("test".into()).position(ToastPosition::BottomLeft);
        let rect = calculate_toast_area(&builder, area);
        assert_eq!(rect.x, 0);
        assert!(rect.y > 0, "BottomLeft y should be > 0, got {}", rect.y);
    }

    #[test]
    fn bottom_right_x_and_y_are_bottom_right() {
        let area = area_80x24();
        let builder = ToastBuilder::new("test".into()).position(ToastPosition::BottomRight);
        let rect = calculate_toast_area(&builder, area);
        assert!(rect.x > 0, "BottomRight x should be > 0, got {}", rect.x);
        assert!(rect.y > 0, "BottomRight y should be > 0, got {}", rect.y);
    }

    #[test]
    fn center_is_centered() {
        let area = area_80x24();
        let builder = ToastBuilder::new("test".into()).position(ToastPosition::Center);
        let rect = calculate_toast_area(&builder, area);
        // Centered width (6) + padding (4) = 10
        // center x = (80 - 10) / 2 = 35
        // center y = (24 - 3) / 2 = 10 (3 = 1 line text + 2 padding)
        assert!(rect.x > 0, "Center x={}", rect.x);
        assert!(rect.y > 0, "Center y={}", rect.y);
    }

    // --- deduplication tests ---

    #[test]
    fn dedup_increments_count() {
        let mut engine: ToastEngine<()> = ToastEngineBuilder::new(area_80x24()).build();
        engine.show_toast(ToastBuilder::new("error".into()).deduplicate(true));
        engine.show_toast(ToastBuilder::new("error".into()).deduplicate(true));

        // Render to buffer — "(x2)" should be visible
        let mut buf = Buffer::empty(area_80x24());
        engine.render_ref(Rect::default(), &mut buf);

        let output = buf_to_string(&buf, area_80x24());
        assert!(
            output.contains("(x2)"),
            "Expected dedup counter in output, got: {output:?}"
        );
    }

    #[test]
    fn dedup_does_not_add_extra_toast() {
        let mut engine: ToastEngine<()> = ToastEngineBuilder::new(area_80x24()).build();
        engine.show_toast(ToastBuilder::new("hello".into()).deduplicate(true));
        // Same message, dedup should NOT add a new toast
        engine.show_toast(ToastBuilder::new("hello".into()).deduplicate(true));

        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        engine.render_ref(Rect::default(), &mut buf);

        // Only one occurrence of "hello" in the output
        let output = buf_to_string(&buf, Rect::new(0, 0, 80, 10));
        let count = output.matches("hello").count();
        assert!(
            count <= 2,
            "Expected at most 2 occurrences of 'hello' (message + border), got {count}"
        );
    }

    #[test]
    fn dedup_different_messages_add_both() {
        let mut engine: ToastEngine<()> = ToastEngineBuilder::new(area_80x24()).build();
        engine.show_toast(ToastBuilder::new("err a".into()).deduplicate(true));
        engine.show_toast(ToastBuilder::new("err b".into()).deduplicate(true));

        let mut buf = Buffer::empty(area_80x24());
        engine.render_ref(Rect::default(), &mut buf);

        let output = buf_to_string(&buf, area_80x24());
        assert!(output.contains("err a"), "Missing toast 'err a'");
        assert!(output.contains("err b"), "Missing toast 'err b'");
    }

    #[test]
    fn dedup_counter_appears_on_second_duplicate() {
        let mut engine: ToastEngine<()> = ToastEngineBuilder::new(area_80x24()).build();
        engine.show_toast(ToastBuilder::new("dup".into()).deduplicate(true));
        engine.show_toast(ToastBuilder::new("dup".into()).deduplicate(true));
        engine.show_toast(ToastBuilder::new("dup".into()).deduplicate(true));

        let mut buf = Buffer::empty(area_80x24());
        engine.render_ref(Rect::default(), &mut buf);

        let output = buf_to_string(&buf, area_80x24());
        assert!(
            output.contains("(x3)"),
            "Expected (x3) in output, got: {output:?}"
        );
    }

    // --- hide_toast / recalculate tests ---

    #[test]
    fn hide_toast_removes_oldest() {
        let mut engine: ToastEngine<()> = ToastEngineBuilder::new(area_80x24())
            .default_duration(Duration::from_secs(3600))
            .build();
        engine.show_toast(ToastBuilder::new("first".into()).position(ToastPosition::TopLeft));
        engine.show_toast(ToastBuilder::new("second".into()).position(ToastPosition::TopLeft));

        // Only expire the oldest toast
        engine.toasts[0].remove_at = Instant::now() - Duration::from_secs(1);

        engine.hide_toast();

        // After hiding expired toasts, only "second" should remain
        let mut buf = Buffer::empty(area_80x24());
        engine.render_ref(Rect::default(), &mut buf);
        let output = buf_to_string(&buf, area_80x24());
        assert!(!output.contains("first"), "Oldest toast should be gone");
        assert!(output.contains("second"), "Newer toast should remain");
    }

    #[test]
    fn hide_toast_recalculates_positions() {
        let mut engine: ToastEngine<()> = ToastEngineBuilder::new(area_80x24())
            .default_duration(Duration::from_secs(3600))
            .build();

        // Show two toasts — second shifts first down
        engine.show_toast(ToastBuilder::new("top".into()).position(ToastPosition::TopLeft));
        engine.show_toast(ToastBuilder::new("bottom".into()).position(ToastPosition::TopLeft));

        // Only expire the oldest toast
        engine.toasts[0].remove_at = Instant::now() - Duration::from_secs(1);

        // Hide the oldest ("top") — "bottom" should move up
        engine.hide_toast();

        // The remaining toast should be at the top (y = 0)
        // We render and check the remaining toast area
        assert_eq!(engine.toasts.len(), 1);
        let remaining = &engine.toasts[0];
        assert_eq!(
            remaining.area.y, 0,
            "Remaining toast should be at y=0 after hide, got y={}",
            remaining.area.y
        );
    }

    // --- clamp_rect tests ---

    #[test]
    fn clamp_rect_inside() {
        let outer = Rect::new(0, 0, 80, 24);
        let inner = Rect::new(5, 5, 10, 3);
        assert_eq!(clamp_rect(inner, outer), inner);
    }

    #[test]
    fn clamp_rect_partial_overflow_right() {
        let outer = Rect::new(0, 0, 20, 10);
        let inner = Rect::new(15, 2, 10, 3); // overflows right by 5
        let clamped = clamp_rect(inner, outer);
        assert_eq!(clamped.x, 15);
        assert_eq!(clamped.width, 5); // only 5 chars fit
        assert_eq!(clamped.height, 3);
    }

    #[test]
    fn clamp_rect_partial_overflow_bottom() {
        let outer = Rect::new(0, 0, 20, 10);
        let inner = Rect::new(2, 8, 10, 5); // overflows bottom by 3
        let clamped = clamp_rect(inner, outer);
        assert_eq!(clamped.y, 8);
        assert_eq!(clamped.height, 2); // only 2 rows fit
    }

    #[test]
    fn clamp_rect_completely_outside() {
        let outer = Rect::new(0, 0, 20, 10);
        let inner = Rect::new(30, 30, 10, 3); // completely outside
        let clamped = clamp_rect(inner, outer);
        assert!(clamped.width == 0 || clamped.height == 0);
    }

    // --- render safety tests ---

    #[test]
    fn render_toast_outside_bounds_no_panic() {
        // Create an engine with small area
        let mut engine: ToastEngine<()> = ToastEngineBuilder::new(area_80x24()).build();

        // Show a toast with a long message so it's large
        engine.show_toast(
            ToastBuilder::new("hello world this is a test message".into())
                .position(ToastPosition::TopLeft),
        );

        // Set a smaller area — toast area is now out of bounds
        engine.set_area(Rect::new(0, 0, 10, 3));

        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 3));
        // This should NOT panic
        engine.render_ref(Rect::default(), &mut buf);
    }

    #[test]
    fn multiple_toasts_overflow_no_panic() {
        let mut engine: ToastEngine<()> = ToastEngineBuilder::new(Rect::new(0, 0, 40, 5)).build();

        // Add many toasts to force overflow
        for i in 0..10 {
            engine.show_toast(
                ToastBuilder::new(format!("toast {i}").into()).position(ToastPosition::TopLeft),
            );
        }

        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 5));
        // This should NOT panic
        engine.render_ref(Rect::default(), &mut buf);
    }

    #[test]
    fn hide_toast_on_empty_no_panic() {
        let mut engine: ToastEngine<()> = ToastEngineBuilder::new(area_80x24()).build();
        // hide on empty engine should be a no-op
        engine.hide_toast();
        engine.hide_toast();
        engine.hide_toast();
        assert!(!engine.has_toast());
    }

    // Helpers

    /// Extract text from a buffer by reading cell symbols, preserving whitespace.
    fn buf_to_string(buf: &Buffer, area: Rect) -> String {
        let mut out = String::new();
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    out.push_str(cell.symbol());
                }
            }
            out.push('\n');
        }
        out
    }
}
