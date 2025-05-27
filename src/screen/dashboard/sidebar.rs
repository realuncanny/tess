use crate::{
    TooltipPosition,
    layout::SavedState,
    screen::create_button,
    style::{Icon, icon_text},
};
use data::sidebar;
use iced::{
    Alignment, Element, Length, Task,
    widget::{Space, column, row},
};
use iced::{Subscription, widget::responsive};

use super::tickers_table::{self, TickersTable};

#[derive(Debug, Clone)]
pub enum Message {
    ToggleSidebarMenu(Option<sidebar::Menu>),
    SetSidebarPosition(sidebar::Position),
    TickersTable(super::tickers_table::Message),
}

pub struct Sidebar {
    pub state: data::Sidebar,
    tickers_table: TickersTable,
}

pub enum Action {
    TickerSelected(exchange::TickerInfo, exchange::adapter::Exchange, String),
    ErrorOccurred(data::InternalError),
}

impl Sidebar {
    pub fn new(state: &SavedState) -> (Self, Task<Message>) {
        let sidebar_state = state.sidebar.clone();
        let (tickers_table, initial_fetch) = TickersTable::new(state.favorited_tickers.clone());

        (
            Self {
                state: sidebar_state,
                tickers_table,
            },
            initial_fetch.map(Message::TickersTable),
        )
    }

    pub fn update(&mut self, message: Message) -> (Task<Message>, Option<Action>) {
        match message {
            Message::ToggleSidebarMenu(menu) => {
                self.set_menu(menu.filter(|&m| !self.is_menu_active(m)));
            }
            Message::SetSidebarPosition(position) => {
                self.state.position = position;
            }
            Message::TickersTable(msg) => {
                let action = self.tickers_table.update(msg);

                match action {
                    Some(tickers_table::Action::TickerSelected(ticker_info, exchange, content)) => {
                        return (
                            Task::none(),
                            Some(Action::TickerSelected(ticker_info, exchange, content)),
                        );
                    }
                    Some(tickers_table::Action::Fetch(task)) => {
                        return (task.map(Message::TickersTable), None);
                    }
                    Some(tickers_table::Action::ErrorOccurred(error)) => {
                        return (
                            Task::none(),
                            Some(Action::ErrorOccurred(data::InternalError::from(error))),
                        );
                    }
                    None => {}
                }
            }
        }

        (Task::none(), None)
    }

    pub fn view<'a>(&'a self, audio_volume: Option<f32>) -> Element<'a, Message> {
        let state = &self.state;

        let tooltip_position = if state.position == sidebar::Position::Left {
            TooltipPosition::Right
        } else {
            TooltipPosition::Left
        };

        let is_table_open = self.tickers_table.is_open();

        let nav_buttons = self.nav_buttons(is_table_open, audio_volume, tooltip_position);

        let tickers_table = if is_table_open {
            column![responsive(move |size| self
                .tickers_table
                .view(size)
                .map(Message::TickersTable))]
            .width(200)
        } else {
            column![]
        };

        match state.position {
            sidebar::Position::Left => row![nav_buttons, tickers_table],
            sidebar::Position::Right => row![tickers_table, nav_buttons],
        }
        .spacing(if is_table_open { 8 } else { 4 })
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        self.tickers_table.subscription().map(Message::TickersTable)
    }

    fn nav_buttons(
        &self,
        is_table_open: bool,
        audio_volume: Option<f32>,
        tooltip_position: TooltipPosition,
    ) -> iced::widget::Column<'_, Message> {
        let settings_modal_button = {
            let is_active = self.is_menu_active(sidebar::Menu::Settings)
                || self.is_menu_active(sidebar::Menu::ThemeEditor);

            create_button(
                icon_text(Icon::Cog, 14)
                    .width(24)
                    .align_x(Alignment::Center),
                Message::ToggleSidebarMenu(Some(sidebar::Menu::Settings)),
                None,
                tooltip_position,
                move |theme, status| crate::style::button::transparent(theme, status, is_active),
            )
        };

        let layout_modal_button = {
            let is_active = self.is_menu_active(sidebar::Menu::Layout);

            create_button(
                icon_text(Icon::Layout, 14)
                    .width(24)
                    .align_x(Alignment::Center),
                Message::ToggleSidebarMenu(Some(sidebar::Menu::Layout)),
                None,
                tooltip_position,
                move |theme, status| crate::style::button::transparent(theme, status, is_active),
            )
        };

        let ticker_search_button = {
            create_button(
                icon_text(Icon::Search, 14)
                    .width(24)
                    .align_x(Alignment::Center),
                Message::TickersTable(super::tickers_table::Message::ToggleTable),
                None,
                tooltip_position,
                move |theme, status| {
                    crate::style::button::transparent(theme, status, is_table_open)
                },
            )
        };

        let audio_btn = {
            let is_active = self.is_menu_active(sidebar::Menu::Audio);

            let icon = match audio_volume.unwrap_or(0.0) {
                v if v >= 40.0 => Icon::SpeakerHigh,
                v if v > 0.0 => Icon::SpeakerLow,
                _ => Icon::SpeakerOff,
            };

            create_button(
                icon_text(icon, 14).width(24).align_x(Alignment::Center),
                Message::ToggleSidebarMenu(Some(sidebar::Menu::Audio)),
                None,
                tooltip_position,
                move |theme, status| crate::style::button::transparent(theme, status, is_active),
            )
        };

        column![
            ticker_search_button,
            layout_modal_button,
            audio_btn,
            Space::with_height(Length::Fill),
            settings_modal_button,
        ]
        .width(32)
        .spacing(8)
    }

    pub fn is_menu_active(&self, menu: sidebar::Menu) -> bool {
        self.state.active_menu == Some(menu)
    }

    pub fn active_menu(&self) -> Option<sidebar::Menu> {
        self.state.active_menu
    }

    pub fn position(&self) -> sidebar::Position {
        self.state.position
    }

    pub fn set_menu(&mut self, menu: Option<sidebar::Menu>) {
        self.state.active_menu = menu;
    }

    pub fn favorited_tickers(&self) -> Vec<(exchange::adapter::Exchange, exchange::Ticker)> {
        self.tickers_table.favorited_tickers()
    }
}
