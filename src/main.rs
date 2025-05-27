#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod chart;
mod layout;
mod logger;
mod modal;
mod screen;
mod style;
mod widget;
mod window;

use data::config::theme::default_theme;
use iced::widget::{pane_grid, pick_list};
use layout::Layout;
use screen::dashboard::{self, Dashboard};
use widget::{confirm_dialog_container, dashboard_modal, main_dialog_modal};
use widget::{
    toast::{self, Toast},
    tooltip,
};

use data::{InternalError, layout::WindowSpec, sidebar};
use exchange::adapter::StreamKind;
use iced::{
    Alignment, Element, Subscription, Task, padding,
    widget::{button, center, column, container, row, text, tooltip::Position as TooltipPosition},
};
use std::{
    borrow::Cow,
    collections::HashMap,
    time::{Duration, Instant},
    vec,
};

fn main() {
    logger::setup(cfg!(debug_assertions)).expect("Failed to initialize logger");

    std::thread::spawn(data::cleanup_old_market_data);

    let _ = iced::daemon(Flowsurface::new, Flowsurface::update, Flowsurface::view)
        .settings(iced::Settings {
            antialiasing: true,
            fonts: vec![
                Cow::Borrowed(style::AZERET_MONO_BYTES),
                Cow::Borrowed(style::ICONS_BYTES),
            ],
            default_text_size: iced::Pixels(12.0),
            ..Default::default()
        })
        .title(Flowsurface::title)
        .theme(Flowsurface::theme)
        .scale_factor(Flowsurface::scale_factor)
        .subscription(Flowsurface::subscription)
        .run();
}

struct Flowsurface {
    main_window: window::Window,
    layout_manager: modal::layout_manager::LayoutManager,
    sidebar: dashboard::sidebar::Sidebar,
    theme_editor: modal::ThemeEditor,
    confirm_dialog: Option<(String, Box<Message>)>,
    scale_factor: data::ScaleFactor,
    timezone: data::UserTimezone,
    theme: data::Theme,
    notifications: Vec<Toast>,
    audio_stream: modal::audio::AudioStream,
}

#[derive(Debug, Clone)]
enum Message {
    LoadLayout(Layout),
    Layouts(modal::layout_manager::Message),

    MarketWsEvent(exchange::Event),
    AudioStream(modal::audio::Message),
    ToggleTradeFetch(bool),

    Dashboard(Option<uuid::Uuid>, dashboard::Message),

    Tick(Instant),
    WindowEvent(window::Event),
    ExitRequested(HashMap<window::Id, WindowSpec>),

    ThemeSelected(data::Theme),
    ScaleFactorChanged(data::ScaleFactor),
    SetTimezone(data::UserTimezone),
    ToggleDialogModal(Option<(String, Box<Message>)>),
    DataFolderRequested,

    AddNotification(Toast),
    DeleteNotification(usize),

    ThemeEditor(modal::theme_editor::Message),
    Sidebar(dashboard::sidebar::Message),
}

impl Flowsurface {
    fn new() -> (Self, Task<Message>) {
        let saved_state = layout::load_saved_state();

        let main_window_cfg = window::Settings {
            size: saved_state
                .main_window
                .map_or_else(window::default_size, |w| w.size()),
            position: saved_state.main_window.map(|w| w.position()).map_or(
                iced::window::Position::Centered,
                iced::window::Position::Specific,
            ),
            exit_on_close_request: false,
            ..window::settings()
        };

        let active_layout = saved_state.layout_manager.active_layout();
        let (main_window_id, open_main_window) = window::open(main_window_cfg);

        let (tickers_table, initial_fetch) =
            dashboard::tickers_table::TickersTable::new(saved_state.favorited_tickers);

        (
            Self {
                main_window: window::Window::new(main_window_id),
                layout_manager: saved_state.layout_manager,
                theme_editor: modal::ThemeEditor::new(saved_state.custom_theme),
                sidebar: dashboard::sidebar::Sidebar::new(saved_state.sidebar, tickers_table),
                audio_stream: modal::audio::AudioStream::new(saved_state.audio_cfg),
                confirm_dialog: None,
                timezone: saved_state.timezone,
                scale_factor: saved_state.scale_factor,
                theme: saved_state.theme,
                notifications: vec![],
            },
            open_main_window
                .then(|_| Task::none())
                .chain(Task::batch(vec![
                    Task::done(Message::LoadLayout(active_layout)),
                    Task::done(Message::SetTimezone(saved_state.timezone)),
                    initial_fetch.map(|msg: dashboard::tickers_table::Message| {
                        Message::Sidebar(dashboard::sidebar::Message::TickersTable(msg))
                    }),
                ])),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MarketWsEvent(event) => {
                let main_window_id = self.main_window.id;

                if let Some(dashboard) = self.active_dashboard_mut() {
                    match event {
                        exchange::Event::Connected(exchange, _) => {
                            log::info!("a stream connected to {exchange} WS");
                        }
                        exchange::Event::Disconnected(exchange, reason) => {
                            log::info!("a stream disconnected from {exchange} WS: {reason:?}");
                        }
                        exchange::Event::DepthReceived(
                            stream,
                            depth_update_t,
                            depth,
                            trades_buffer,
                        ) => {
                            let task = dashboard
                                .update_depth_and_trades(
                                    &stream,
                                    depth_update_t,
                                    &depth,
                                    &trades_buffer,
                                    main_window_id,
                                )
                                .map(move |msg| Message::Dashboard(None, msg));

                            if let Err(err) =
                                self.audio_stream.try_play_sound(&stream, &trades_buffer)
                            {
                                log::error!("Failed to play sound: {err}");
                            }

                            return task;
                        }
                        exchange::Event::KlineReceived(stream, kline) => {
                            return dashboard
                                .update_latest_klines(&stream, &kline, main_window_id)
                                .map(move |msg| Message::Dashboard(None, msg));
                        }
                    }
                }
            }
            Message::Tick(now) => {
                let main_window_id = self.main_window.id;

                if let Some(dashboard) = self.active_dashboard_mut() {
                    return dashboard
                        .tick(now, main_window_id)
                        .map(move |msg| Message::Dashboard(None, msg));
                }
            }
            Message::WindowEvent(event) => match event {
                window::Event::CloseRequested(window) => {
                    let main_window = self.main_window.id;

                    let Some(dashboard) = self.active_dashboard_mut() else {
                        return iced::exit();
                    };

                    if window != main_window {
                        dashboard.popout.remove(&window);
                        return window::close(window);
                    }

                    let mut opened_windows = dashboard
                        .popout
                        .keys()
                        .copied()
                        .collect::<Vec<window::Id>>();

                    opened_windows.push(main_window);

                    return window::collect_window_specs(opened_windows, Message::ExitRequested);
                }
            },
            Message::ExitRequested(windows) => {
                let dashboard = self.active_dashboard_mut().expect("No active dashboard");

                dashboard
                    .popout
                    .iter_mut()
                    .for_each(|(id, (_, window_spec))| {
                        if let Some(new_window_spec) = windows.get(id) {
                            *window_spec = *new_window_spec;
                        }
                    });

                let mut ser_layouts = Vec::new();

                for id in &self.layout_manager.layout_order {
                    if let Some((layout, dashboard)) = self.layout_manager.layouts.get(id) {
                        let serialized_dashboard = data::Dashboard::from(dashboard);

                        ser_layouts.push(data::Layout {
                            name: layout.name.clone(),
                            dashboard: serialized_dashboard,
                        });
                    }
                }

                let layouts = data::Layouts {
                    layouts: ser_layouts,
                    active_layout: self.layout_manager.active_layout.name.clone(),
                };

                let main_window = windows
                    .iter()
                    .find(|(id, _)| **id == self.main_window.id)
                    .map(|(_, spec)| *spec);

                let audio_cfg = data::AudioStream::from(&self.audio_stream);

                let layout = data::State::from_parts(
                    layouts,
                    self.theme.clone(),
                    self.theme_editor.custom_theme.clone().map(data::Theme),
                    self.sidebar.favorited_tickers(),
                    main_window,
                    self.timezone,
                    self.sidebar.state,
                    self.scale_factor,
                    audio_cfg,
                );

                match serde_json::to_string(&layout) {
                    Ok(layout_str) => {
                        let file_name = data::SAVED_STATE_PATH;

                        if let Err(e) = data::write_json_to_file(&layout_str, file_name) {
                            log::error!("Failed to write layout state to file: {}", e);
                        } else {
                            log::info!("Successfully wrote layout state to {file_name}");
                        }
                    }
                    Err(e) => log::error!("Failed to serialize layout: {}", e),
                }

                return iced::exit();
            }
            Message::ThemeSelected(theme) => {
                self.theme = theme.clone();
            }
            Message::Dashboard(id, message) => {
                let main_window = self.main_window;
                let layout_id = id.unwrap_or(self.layout_manager.active_layout.id);

                if let Some(dashboard) = self.layout_manager.mut_dashboard(&layout_id) {
                    let (main_task, event) = dashboard.update(message, &main_window, &layout_id);

                    let additional_task = match event {
                        Some(dashboard::Event::DistributeFetchedData {
                            layout_id,
                            pane_id,
                            data,
                            stream,
                        }) => dashboard
                            .distribute_fetched_data(main_window.id, pane_id, data, stream)
                            .map(move |msg| Message::Dashboard(Some(layout_id), msg)),
                        Some(dashboard::Event::Notification(toast)) => {
                            Task::done(Message::AddNotification(toast))
                        }
                        None => Task::none(),
                    };

                    return main_task
                        .map(move |msg| Message::Dashboard(Some(layout_id), msg))
                        .chain(additional_task);
                }
            }
            Message::SetTimezone(tz) => {
                self.timezone = tz;
            }
            Message::ScaleFactorChanged(value) => {
                self.scale_factor = value;
            }
            Message::ToggleTradeFetch(checked) => {
                self.layout_manager
                    .iter_dashboards_mut()
                    .for_each(|dashboard| {
                        dashboard.toggle_trade_fetch(checked, &self.main_window);
                    });

                if checked {
                    self.confirm_dialog = None;
                }
            }
            Message::ToggleDialogModal(dialog) => {
                self.confirm_dialog = dialog;
            }
            Message::Layouts(message) => {
                let action = self.layout_manager.update(message);

                match action {
                    Some(modal::layout_manager::Action::Select(layout)) => {
                        if let Some(dashboard) = self.active_dashboard() {
                            let active_popout_keys =
                                dashboard.popout.keys().copied().collect::<Vec<_>>();

                            let window_tasks = Task::batch(
                                active_popout_keys
                                    .iter()
                                    .map(|&popout_id| window::close(popout_id))
                                    .collect::<Vec<_>>(),
                            )
                            .then(|_: Task<window::Id>| Task::none());

                            return window::collect_window_specs(
                                active_popout_keys,
                                dashboard::Message::SavePopoutSpecs,
                            )
                            .map(move |msg| Message::Dashboard(None, msg))
                            .chain(window_tasks)
                            .chain(Task::done(Message::LoadLayout(layout)));
                        }
                    }
                    None => {}
                }
            }
            Message::LoadLayout(layout) => {
                self.layout_manager.active_layout = layout.clone();
                if let Some(dashboard) = self.active_dashboard_mut() {
                    dashboard.focus = None;
                    return dashboard
                        .load_layout()
                        .map(move |msg| Message::Dashboard(None, msg));
                }
            }
            Message::AddNotification(toast) => {
                self.notifications.push(toast);
            }
            Message::DeleteNotification(index) => {
                if index < self.notifications.len() {
                    self.notifications.remove(index);
                }
            }
            Message::AudioStream(message) => self.audio_stream.update(message),
            Message::DataFolderRequested => {
                if let Err(err) = data::open_data_folder() {
                    return Task::done(Message::AddNotification(Toast::error(format!(
                        "Failed to open data folder: {err}",
                    ))));
                }
            }
            Message::ThemeEditor(msg) => {
                let action = self.theme_editor.update(msg, &self.theme.clone().into());

                match action {
                    Some(modal::theme_editor::Action::Exit) => {
                        self.sidebar.set_menu(Some(sidebar::Menu::Settings));
                    }
                    Some(modal::theme_editor::Action::UpdateTheme(theme)) => {
                        self.theme = data::Theme(theme);

                        let main_window = self.main_window.id;

                        if let Some(dashboard) = self.active_dashboard_mut() {
                            dashboard.invalidate_all_panes(main_window);
                        }
                    }
                    None => {}
                }
            }
            Message::Sidebar(message) => {
                let (task, action) = self.sidebar.update(message);

                match action {
                    Some(dashboard::sidebar::Action::TickerSelected(
                        ticker_info,
                        exchange,
                        content,
                    )) => {
                        let main_window_id = self.main_window.id;

                        if let Some(dashboard) = self.active_dashboard_mut() {
                            let task = dashboard.init_pane_task(
                                main_window_id,
                                ticker_info,
                                exchange,
                                &content,
                            );

                            return task.map(move |msg| Message::Dashboard(None, msg));
                        }
                    }
                    Some(dashboard::sidebar::Action::ErrorOccurred(err)) => {
                        self.notify_error(err);
                    }
                    None => {}
                }

                return task.map(Message::Sidebar);
            }
        }
        Task::none()
    }

    fn view(&self, id: window::Id) -> Element<'_, Message> {
        let Some(dashboard) = self.active_dashboard() else {
            return center(
                column![
                    text("No dashboard available").size(20),
                    button("Add new dashboard")
                        .on_press(Message::Layouts(modal::layout_manager::Message::AddLayout))
                ]
                .align_x(Alignment::Center)
                .spacing(8),
            )
            .into();
        };

        let sidebar_pos = self.sidebar.position();

        let content = if id == self.main_window.id {
            let sidebar_view = self
                .sidebar
                .view(self.audio_stream.volume())
                .map(Message::Sidebar);

            let dashboard_view = dashboard
                .view(&self.main_window, self.timezone)
                .map(move |msg| Message::Dashboard(None, msg));

            let header_title = {
                #[cfg(target_os = "macos")]
                {
                    iced::widget::center(
                        text("FLOWSURFACE")
                            .font(iced::Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            })
                            .size(16)
                            .style(style::title_text),
                    )
                    .height(20)
                    .align_y(Alignment::Center)
                    .padding(padding::top(4))
                }
                #[cfg(not(target_os = "macos"))]
                {
                    column![]
                }
            };

            let base = column![
                header_title,
                match sidebar_pos {
                    sidebar::Position::Left => row![sidebar_view, dashboard_view,],
                    sidebar::Position::Right => row![dashboard_view, sidebar_view],
                }
                .spacing(4)
                .padding(8),
            ];

            if let Some(menu) = self.sidebar.active_menu() {
                self.view_with_modal(base.into(), dashboard, menu)
            } else {
                base.into()
            }
        } else {
            container(
                dashboard
                    .view_window(id, &self.main_window, self.timezone)
                    .map(move |msg| Message::Dashboard(None, msg)),
            )
            .padding(padding::top(style::TITLE_PADDING_TOP))
            .into()
        };

        toast::Manager::new(
            content,
            &self.notifications,
            match sidebar_pos {
                sidebar::Position::Left => Alignment::End,
                sidebar::Position::Right => Alignment::Start,
            },
            Message::DeleteNotification,
        )
        .into()
    }

    fn theme(&self, _window: window::Id) -> iced_core::Theme {
        self.theme.clone().into()
    }

    fn title(&self, _window: window::Id) -> String {
        format!("Flowsurface [{}]", self.layout_manager.active_layout.name)
    }

    fn scale_factor(&self, _window: window::Id) -> f64 {
        self.scale_factor.into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let window_events = window::events().map(Message::WindowEvent);

        let Some(dashboard) = self.active_dashboard() else {
            return window_events;
        };

        let exchange_streams = dashboard.market_subscriptions().map(Message::MarketWsEvent);

        let sidebar = self.sidebar.subscription().map(Message::Sidebar);

        let tick = iced::time::every(Duration::from_millis(100)).map(Message::Tick);

        Subscription::batch(vec![exchange_streams, sidebar, window_events, tick])
    }

    fn active_dashboard(&self) -> Option<&Dashboard> {
        self.layout_manager.active_dashboard()
    }

    fn active_dashboard_mut(&mut self) -> Option<&mut Dashboard> {
        self.layout_manager.active_dashboard_mut()
    }

    fn notify_error(&mut self, err: InternalError) {
        self.notifications.push(Toast::error(err.to_string()));
    }

    fn view_with_modal<'a>(
        &'a self,
        base: Element<'a, Message>,
        dashboard: &'a Dashboard,
        menu: sidebar::Menu,
    ) -> Element<'a, Message> {
        let sidebar_pos = self.sidebar.position();
        let main_window = self.main_window.id;

        match menu {
            sidebar::Menu::Settings => {
                let settings_modal = {
                    let theme_picklist = {
                        let mut themes: Vec<iced::Theme> = iced_core::Theme::ALL.to_vec();

                        let default_theme = iced_core::Theme::Custom(default_theme().into());
                        themes.push(default_theme);

                        if let Some(custom_theme) = self.theme_editor.custom_theme.clone() {
                            themes.push(custom_theme);
                        }

                        pick_list(themes, Some(self.theme.clone().0), |theme| {
                            Message::ThemeSelected(data::Theme(theme))
                        })
                    };

                    let toggle_theme_editor = button(text("Theme editor")).on_press(
                        Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(Some(
                            sidebar::Menu::ThemeEditor,
                        ))),
                    );

                    let timezone_picklist = pick_list(
                        [data::UserTimezone::Utc, data::UserTimezone::Local],
                        Some(self.timezone),
                        Message::SetTimezone,
                    );

                    let sidebar_pos = pick_list(
                        [sidebar::Position::Left, sidebar::Position::Right],
                        Some(sidebar_pos),
                        |pos| {
                            Message::Sidebar(dashboard::sidebar::Message::SetSidebarPosition(pos))
                        },
                    );

                    let scale_factor = {
                        let current_value: f64 = self.scale_factor.into();

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
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End,
                    align_x,
                );

                if let Some((dialog, on_confirm)) = &self.confirm_dialog {
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
                let pane = if let Some(focus) = dashboard.focus {
                    focus.1
                } else {
                    *dashboard.panes.iter().next().unwrap().0
                };

                let reset_pane_button = tooltip(
                    button(text("Reset").align_x(Alignment::Center))
                        .width(iced::Length::Fill)
                        .on_press(Message::Dashboard(
                            None,
                            dashboard::Message::Pane(
                                main_window,
                                dashboard::pane::Message::ReplacePane(pane),
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
                            dashboard::Message::Pane(
                                main_window,
                                dashboard::pane::Message::SplitPane(
                                    pane_grid::Axis::Horizontal,
                                    pane,
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
                                self.layout_manager.view().map(Message::Layouts),
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
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
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
                    self.audio_stream
                        .view(depth_streams_list)
                        .map(Message::AudioStream),
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
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
                    self.theme_editor
                        .view(&self.theme.0)
                        .map(Message::ThemeEditor),
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End,
                    align_x,
                )
            }
        }
    }
}
