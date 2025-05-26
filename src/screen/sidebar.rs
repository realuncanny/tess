use crate::widget::{confirm_dialog_container, dashboard_modal, main_dialog_modal, tooltip};
use crate::{Flowsurface, TooltipPosition, style, window};
use crate::{
    Message,
    screen::create_button,
    style::{Icon, icon_text},
};
use data::config::theme::default_theme;
use data::sidebar;
use iced::padding;
use iced::widget::{button, container, pane_grid, pick_list, text};
use iced::{
    Alignment, Element, Length,
    widget::{Space, column, row},
};

use super::dashboard::Dashboard;

pub struct Sidebar(pub data::Sidebar);

impl Sidebar {
    pub fn new(state: data::Sidebar) -> Self {
        Self(state)
    }

    pub fn view<'a>(
        &'a self,
        is_table_open: bool,
        audio_volume: Option<f32>,
        tickers_table_view: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let state = &self.0;

        let tooltip_position = if state.position == sidebar::Position::Left {
            TooltipPosition::Right
        } else {
            TooltipPosition::Left
        };

        let nav_buttons = self.create_nav_buttons(is_table_open, audio_volume, tooltip_position);

        let tickers_table = if is_table_open {
            column![tickers_table_view].width(200)
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

    fn create_nav_buttons(
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
        self.0.active_menu == Some(menu)
    }

    pub fn active_menu(&self) -> Option<sidebar::Menu> {
        self.0.active_menu
    }

    pub fn position(&self) -> sidebar::Position {
        self.0.position
    }

    pub fn set_position(&mut self, position: sidebar::Position) {
        self.0.position = position;
    }

    pub fn set_menu(&mut self, menu: Option<sidebar::Menu>) {
        self.0.active_menu = menu;
    }

    pub fn view_with_modals<'a>(
        &'a self,
        menu: sidebar::Menu,
        dashboard: &'a Dashboard,
        app: &'a Flowsurface,
        base: Element<'a, Message>,
        id: window::Id,
    ) -> Element<'a, Message> {
        let sidebar_pos = self.0.position;

        match menu {
            sidebar::Menu::Settings => {
                let settings_modal = {
                    let theme_picklist = {
                        let mut themes: Vec<iced::Theme> = iced_core::Theme::ALL.to_vec();

                        let default_theme = iced_core::Theme::Custom(default_theme().into());
                        themes.push(default_theme);

                        if let Some(custom_theme) = app.theme_editor.custom_theme.clone() {
                            themes.push(custom_theme);
                        }

                        pick_list(themes, Some(app.theme.clone().0), |theme| {
                            Message::ThemeSelected(data::Theme(theme))
                        })
                    };

                    let toggle_theme_editor = button(text("Theme editor"))
                        .on_press(Message::ToggleSidebarMenu(Some(sidebar::Menu::ThemeEditor)));

                    let timezone_picklist = pick_list(
                        [data::UserTimezone::Utc, data::UserTimezone::Local],
                        Some(app.timezone),
                        Message::SetTimezone,
                    );

                    let sidebar_pos = pick_list(
                        [sidebar::Position::Left, sidebar::Position::Right],
                        Some(sidebar_pos),
                        Message::SetSidebarPosition,
                    );

                    let scale_factor = {
                        let current_value: f64 = app.scale_factor.into();

                        let decrease_btn = if current_value > data::config::MIN_SCALE {
                            button(text("-"))
                                .on_press(Message::ScaleFactorChanged((current_value - 0.1).into()))
                        } else {
                            button(text("-"))
                        };

                        let increase_btn = if current_value < data::config::MAX_SCALE {
                            button(text("+"))
                                .on_press(Message::ScaleFactorChanged((current_value + 0.1).into()))
                        } else {
                            button(text("+"))
                        };

                        container(
                            row![
                                decrease_btn,
                                text(format!("{:.0}%", current_value * 100.0)).size(14),
                                increase_btn,
                            ]
                            .align_y(Alignment::Center)
                            .spacing(8)
                            .padding(4),
                        )
                        .style(style::modal_container)
                    };

                    let trade_fetch_checkbox = {
                        let is_active = dashboard.trade_fetch_enabled;

                        let checkbox = iced::widget::checkbox("Fetch trades (Binance)", is_active)
                            .on_toggle(|checked| {
                                if checked {
                                    Message::ToggleDialogModal(Some((
                                        "This might be unreliable and take some time to complete"
                                            .to_string(),
                                        Box::new(Message::ToggleTradeFetch(true)),
                                    )))
                                } else {
                                    Message::ToggleTradeFetch(false)
                                }
                            });

                        tooltip(
                            checkbox,
                            Some("Try to fetch trades for footprint charts"),
                            TooltipPosition::Top,
                        )
                    };

                    let open_data_folder = {
                        let button =
                            button(text("Open data folder")).on_press(Message::DataFolderRequested);

                        tooltip(
                            button,
                            Some("Open the folder where the data & config is stored"),
                            TooltipPosition::Top,
                        )
                    };

                    container(
                        column![
                            column![open_data_folder,].spacing(8),
                            column![text("Sidebar position").size(14), sidebar_pos,].spacing(8),
                            column![text("Time zone").size(14), timezone_picklist,].spacing(8),
                            column![text("Theme").size(14), theme_picklist,].spacing(8),
                            column![text("Interface scale").size(14), scale_factor,].spacing(8),
                            column![
                                text("Experimental").size(14),
                                trade_fetch_checkbox,
                                toggle_theme_editor
                            ]
                            .spacing(8),
                        ]
                        .spacing(20),
                    )
                    .align_x(Alignment::Start)
                    .max_width(400)
                    .padding(24)
                    .style(style::dashboard_modal)
                };

                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(48).top(8)),
                    sidebar::Position::Right => (Alignment::End, padding::right(48).top(8)),
                };

                let base_content = dashboard_modal(
                    base,
                    settings_modal,
                    Message::ToggleSidebarMenu(None),
                    padding,
                    Alignment::End,
                    align_x,
                );

                if let Some((dialog, on_confirm)) = &app.confirm_dialog {
                    let dialog_content = confirm_dialog_container(
                        dialog,
                        *on_confirm.to_owned(),
                        Message::ToggleDialogModal(None),
                    );

                    main_dialog_modal(
                        base_content,
                        dialog_content,
                        Message::ToggleDialogModal(None),
                    )
                } else {
                    base_content
                }
            }
            sidebar::Menu::Layout => {
                let reset_pane_button = tooltip(
                    button(text("Reset").align_x(Alignment::Center))
                        .width(iced::Length::Fill)
                        .on_press(Message::Dashboard(
                            None,
                            super::dashboard::Message::Pane(
                                id,
                                super::dashboard::pane::Message::ReplacePane(
                                    if let Some(focus) = dashboard.focus {
                                        focus.1
                                    } else {
                                        *dashboard.panes.iter().next().unwrap().0
                                    },
                                ),
                            ),
                        )),
                    Some("Reset selected pane"),
                    TooltipPosition::Top,
                );
                let split_pane_button = tooltip(
                    button(text("Split").align_x(Alignment::Center))
                        .width(iced::Length::Fill)
                        .on_press(Message::Dashboard(
                            None,
                            super::dashboard::Message::Pane(
                                id,
                                super::dashboard::pane::Message::SplitPane(
                                    pane_grid::Axis::Horizontal,
                                    if let Some(focus) = dashboard.focus {
                                        focus.1
                                    } else {
                                        *dashboard.panes.iter().next().unwrap().0
                                    },
                                ),
                            ),
                        )),
                    Some("Split selected pane horizontally"),
                    TooltipPosition::Top,
                );

                let manage_layout_modal = {
                    container(
                        column![
                            column![
                                text("Panes").size(14),
                                if dashboard.focus.is_some() {
                                    row![reset_pane_button, split_pane_button,]
                                        .padding(padding::left(8).right(8))
                                        .spacing(8)
                                } else {
                                    row![text("No pane selected"),]
                                },
                            ]
                            .align_x(Alignment::Center)
                            .spacing(8),
                            column![
                                text("Layouts").size(14),
                                app.layout_manager.view().map(Message::Layouts),
                            ]
                            .align_x(Alignment::Center)
                            .spacing(8),
                        ]
                        .align_x(Alignment::Center)
                        .spacing(32),
                    )
                    .width(280)
                    .padding(24)
                    .style(style::dashboard_modal)
                };

                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(48).top(40)),
                    sidebar::Position::Right => (Alignment::End, padding::right(48).top(40)),
                };

                dashboard_modal(
                    base,
                    manage_layout_modal,
                    Message::ToggleSidebarMenu(None),
                    padding,
                    Alignment::Start,
                    align_x,
                )
            }
            sidebar::Menu::Audio => {
                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(48).top(64)),
                    sidebar::Position::Right => (Alignment::End, padding::right(48).top(64)),
                };

                let depth_streams_list = dashboard.streams.depth_streams(None);

                dashboard_modal(
                    base,
                    app.audio_stream
                        .view(depth_streams_list)
                        .map(Message::AudioStream),
                    Message::ToggleSidebarMenu(None),
                    padding,
                    Alignment::Start,
                    align_x,
                )
            }
            sidebar::Menu::ThemeEditor => {
                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(48).top(8)),
                    sidebar::Position::Right => (Alignment::End, padding::right(48).top(8)),
                };

                dashboard_modal(
                    base,
                    app.theme_editor
                        .view(&app.theme.0)
                        .map(Message::ThemeEditor),
                    Message::ToggleSidebarMenu(None),
                    padding,
                    Alignment::End,
                    align_x,
                )
            }
        }
    }
}
