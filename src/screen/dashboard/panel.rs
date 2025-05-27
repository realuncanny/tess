pub mod timeandsales;

use crate::{
    screen::dashboard::pane::Message,
    widget::{self},
};

use super::pane;

use iced::{
    Alignment, Element,
    widget::{center, pane_grid},
};

pub trait PanelView {
    fn view(
        &self,
        pane: pane_grid::Pane,
        state: &pane::State,
        timezone: data::UserTimezone,
    ) -> Element<Message>;
}

pub fn view<'a, C: PanelView>(
    pane: pane_grid::Pane,
    state: &'a pane::State,
    content: &'a C,
    timezone: data::UserTimezone,
) -> Element<'a, Message> {
    let base = center(content.view(pane, state, timezone));

    widget::toast::Manager::new(base, &state.notifications, Alignment::End, move |idx| {
        Message::DeleteNotification(pane, idx)
    })
    .into()
}
