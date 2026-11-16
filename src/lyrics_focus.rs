//! The column of synced lyrics, placed so a given line sits at the focus height of the panel.
//!
//! Lines are as tall as their words need (a long line wraps, a romanized line adds a row), so
//! where a line is can only be known once iced has laid the column out. This widget lays the
//! lines out at their natural height and then moves the column so that line `line` (a line
//! index, fractional while the lyrics slide from one line to the next) is at `focus` of the
//! panel's height. It costs no more than the layout iced does anyway.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Shell, mouse, overlay, renderer};
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};

/// `content` (a column of lines) scrolled to `line`, which sits `focus` (a share of the
/// height) from the top.
pub struct LyricsFocus<'a, Message> {
    content: Element<'a, Message>,
    line: f32,
    focus: f32,
}

impl<'a, Message> LyricsFocus<'a, Message> {
    pub fn new(content: impl Into<Element<'a, Message>>, line: f32, focus: f32) -> Self {
        Self {
            content: content.into(),
            line,
            focus,
        }
    }
}

/// Where line `line` starts in a column whose lines start at `tops` and which is `height`
/// tall: between two lines, the same share of the way from one to the next.
pub fn line_top(tops: &[f32], height: f32, line: f32) -> f32 {
    if tops.is_empty() {
        return 0.0;
    }
    let line = line.clamp(0.0, (tops.len() - 1) as f32);
    let index = line.floor() as usize;
    let start = tops[index];
    let end = tops.get(index + 1).copied().unwrap_or(height);
    start + (end - start) * line.fract()
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for LyricsFocus<'_, Message> {
    fn tag(&self) -> widget::tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> widget::tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);
        // The lines as tall as they need, however tall the column gets.
        let column = self.content.as_widget_mut().layout(
            tree,
            renderer,
            &layout::Limits::new(Size::ZERO, Size::new(size.width, f32::INFINITY)),
        );
        let tops: Vec<f32> = column
            .children()
            .iter()
            .map(|line| line.bounds().y)
            .collect();
        let top = line_top(&tops, column.size().height, self.line);
        let column = column.move_to(Point::new(0.0, size.height * self.focus - top));
        layout::Node::with_children(size, vec![column])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if let Some(column) = layout.children().next() {
            self.content
                .as_widget_mut()
                .operate(tree, column, renderer, operation);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Some(column) = layout.children().next() {
            self.content.as_widget_mut().update(
                tree, event, column, cursor, renderer, clipboard, shell, viewport,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        layout
            .children()
            .next()
            .map_or(mouse::Interaction::default(), |column| {
                self.content
                    .as_widget()
                    .mouse_interaction(tree, column, cursor, viewport, renderer)
            })
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if let (Some(column), Some(visible)) = (
            layout.children().next(),
            layout.bounds().intersection(viewport),
        ) {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, column, cursor, &visible);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let column = layout.children().next()?;
        self.content
            .as_widget_mut()
            .overlay(tree, column, renderer, viewport, translation)
    }
}

impl<'a, Message: 'a> From<LyricsFocus<'a, Message>> for Element<'a, Message> {
    fn from(focus: LyricsFocus<'a, Message>) -> Self {
        Element::new(focus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_where_a_line_starts_between_lines_of_any_height() {
        // A short line, a long one that wraps, a short one.
        let tops = [0.0, 60.0, 180.0];
        assert_eq!(line_top(&tops, 240.0, 0.0), 0.0);
        assert_eq!(line_top(&tops, 240.0, 1.0), 60.0);
        // Halfway from the long line to the next goes half of the long line's height.
        assert_eq!(line_top(&tops, 240.0, 1.5), 120.0);
        // The last line slides toward the column's end; out of range stays in range.
        assert_eq!(line_top(&tops, 240.0, 2.5), 180.0);
        assert_eq!(line_top(&tops, 240.0, 9.0), 180.0);
        assert_eq!(line_top(&tops, 240.0, -1.0), 0.0);
        assert_eq!(line_top(&[], 0.0, 3.0), 0.0);
    }
}
