//! Keyboard focus for Iced 0.14's standard pointer-only button.
use crate::app::Message;
use iced::advanced::widget::{Operation, Tree, operation::Focusable, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, renderer, widget};
use iced::{Border, Element, Event, Length, Padding, Rectangle, Renderer, Size, Theme, keyboard};

#[derive(Default)]
struct Focus {
    focused: bool,
}

impl Focusable for Focus {
    fn is_focused(&self) -> bool {
        self.focused
    }
    fn focus(&mut self) {
        self.focused = true;
    }
    fn unfocus(&mut self) {
        self.focused = false;
    }
}

pub struct Button<'a> {
    inner: iced::widget::Button<'a, Message>,
    message: Option<Message>,
    id: Option<widget::Id>,
}

pub fn button<'a>(content: impl Into<Element<'a, Message>>) -> Button<'a> {
    Button {
        inner: iced::widget::button(content),
        message: None,
        id: None,
    }
}

impl<'a> Button<'a> {
    pub fn on_press(self, message: Message) -> Self {
        self.on_press_maybe(Some(message))
    }
    pub fn on_press_maybe(mut self, message: Option<Message>) -> Self {
        self.inner = self.inner.on_press_maybe(message.clone());
        self.message = message;
        self
    }
    pub fn id(mut self, id: &'static str) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.inner = self.inner.padding(padding);
        self
    }
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.inner = self.inner.width(width);
        self
    }
    pub fn style(
        mut self,
        style: impl Fn(&Theme, iced::widget::button::Status) -> iced::widget::button::Style + 'a,
    ) -> Self {
        self.inner = self.inner.style(style);
        self
    }
}

impl Widget<Message, Theme, Renderer> for Button<'_> {
    fn size(&self) -> Size<Length> {
        self.inner.size()
    }
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Focus>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(Focus::default())
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(
            &self.inner as &dyn Widget<Message, Theme, Renderer>,
        )]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.children[0].diff(&self.inner as &dyn Widget<Message, Theme, Renderer>);
    }
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.inner.layout(&mut tree.children[0], renderer, limits)
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if self.message.is_some() {
            operation.focusable(
                self.id.as_ref(),
                layout.bounds(),
                tree.state.downcast_mut::<Focus>(),
            );
        }
        self.inner
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let focus = tree.state.downcast_mut::<Focus>();
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            focus.focused = self.message.is_some() && cursor.is_over(layout.bounds());
        }
        if focus.focused
            && let Event::Keyboard(keyboard::Event::KeyPressed {
                key, repeat: false, ..
            }) = event
            && matches!(
                key,
                keyboard::Key::Named(keyboard::key::Named::Enter | keyboard::key::Named::Space)
            )
            && let Some(message) = &self.message
        {
            shell.publish(message.clone());
            shell.capture_event();
            return;
        }
        self.inner.update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.inner.draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        if tree.state.downcast_ref::<Focus>().focused && self.message.is_some() {
            use iced::advanced::Renderer as _;
            if layout.bounds().intersection(viewport).is_some() {
                renderer.with_layer(*viewport, |renderer| {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: layout.bounds(),
                            border: Border {
                                color: theme.palette().primary,
                                width: 2.0,
                                radius: 6.0.into(),
                            },
                            ..Default::default()
                        },
                        iced::Color::TRANSPARENT,
                    );
                });
            }
        }
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.inner
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }
}

impl<'a> From<Button<'a>> for Element<'a, Message> {
    fn from(button: Button<'a>) -> Self {
        Element::new(button)
    }
}
