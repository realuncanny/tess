#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod chart;
mod layout;
mod logger;
mod screen;
mod style;
mod widget;
mod window;

use layout::{Layout, LayoutManager};
use screen::dashboard::{self, Dashboard};
use screen::theme_editor::{self, ThemeEditor};
use screen::tickers_table::{self, TickersTable};
use widget::{
    toast::{self, Toast},
    tooltip,
};

use data::{InternalError, layout::WindowSpec, sidebar};
use exchange::adapter::{Exchange, StreamKind, fetch_ticker_info};
use iced::{
    Alignment, Element, Subscription, Task, padding,
    widget::{
        button, center, column, container, responsive, row, text,
        tooltip::Position as TooltipPosition,
    },
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
    layout_manager: LayoutManager,
    tickers_table: TickersTable,
    sidebar: screen::Sidebar,
    confirm_dialog: Option<(String, Box<Message>)>,
    scale_factor: data::ScaleFactor,
    timezone: data::UserTimezone,
    theme: data::Theme,
    notifications: Vec<Toast>,
    audio_stream: audio::AudioStream,
    theme_editor: ThemeEditor,
}

#[derive(Debug, Clone)]
enum Message {
    LoadLayout(Layout),
    Layouts(layout::Message),

    MarketWsEvent(exchange::Event),
    AudioStream(audio::Message),
    FetchTickersInfo,
    ToggleTradeFetch(bool),

    Dashboard(Option<uuid::Uuid>, dashboard::Message),
    TickersTable(tickers_table::Message),

    Tick(Instant),
    WindowEvent(window::Event),
    ExitRequested(HashMap<window::Id, WindowSpec>),
    ErrorOccurred(InternalError),

    ThemeSelected(data::Theme),
    ScaleFactorChanged(data::ScaleFactor),
    SetTimezone(data::UserTimezone),
    SetSidebarPosition(sidebar::Position),
    ToggleSidebarMenu(Option<sidebar::Menu>),
    ToggleDialogModal(Option<(String, Box<Message>)>),
    DataFolderRequested,

    AddNotification(Toast),
    DeleteNotification(usize),

    ThemeEditor(theme_editor::Message),
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

        (
            Self {
                main_window: window::Window::new(main_window_id),
                layout_manager: saved_state.layout_manager,
                tickers_table: TickersTable::new(saved_state.favorited_tickers),
                confirm_dialog: None,
                timezone: saved_state.timezone,
                scale_factor: saved_state.scale_factor,
                sidebar: screen::Sidebar::new(saved_state.sidebar),
                theme: saved_state.theme,
                notifications: vec![],
                audio_stream: audio::AudioStream::new(saved_state.audio_cfg),
                theme_editor: theme_editor::ThemeEditor::new(saved_state.custom_theme),
            },
            open_main_window
                .then(|_| Task::none())
                .chain(Task::batch(vec![
                    Task::done(Message::LoadLayout(active_layout)),
                    Task::done(Message::SetTimezone(saved_state.timezone)),
                    Task::done(Message::FetchTickersInfo),
                ])),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FetchTickersInfo => {
                let fetch_tasks = Exchange::ALL
                    .iter()
                    .map(|exchange| {
                        Task::perform(fetch_ticker_info(*exchange), move |result| match result {
                            Ok(ticker_info) => Message::TickersTable(
                                tickers_table::Message::UpdateTickersInfo(*exchange, ticker_info),
                            ),
                            Err(err) => {
                                Message::ErrorOccurred(InternalError::Fetch(err.to_string()))
                            }
                        })
                    })
                    .collect::<Vec<Task<Message>>>();

                return Task::batch(fetch_tasks);
            }
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
                    self.tickers_table.favorited_tickers(),
                    main_window,
                    self.timezone,
                    self.sidebar.0,
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
            Message::ErrorOccurred(err) => {
                return match err {
                    InternalError::Fetch(err) | InternalError::Layout(err) => {
                        Task::done(Message::AddNotification(Toast::error(err)))
                    }
                };
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
            Message::TickersTable(message) => {
                let action = self.tickers_table.update(message);

                match action {
                    Some(tickers_table::Action::TickerSelected(ticker_info, exchange, content)) => {
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
                    Some(tickers_table::Action::Fetch(task)) => {
                        return task.map(Message::TickersTable);
                    }
                    Some(tickers_table::Action::ErrorOccurred(err)) => {
                        return Task::done(Message::ErrorOccurred(err));
                    }
                    None => {}
                }
            }
            Message::SetTimezone(tz) => {
                self.timezone = tz;
            }
            Message::SetSidebarPosition(position) => {
                self.sidebar.set_position(position);
            }
            Message::ScaleFactorChanged(value) => {
                self.scale_factor = value;
            }
            Message::ToggleSidebarMenu(menu) => {
                self.sidebar
                    .set_menu(menu.filter(|&m| !self.sidebar.is_menu_active(m)));
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
                    Some(layout::Action::Select(layout)) => {
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
                        } else {
                            return Task::done(Message::ErrorOccurred(InternalError::Layout(
                                "Couldn't get active dashboard".to_string(),
                            )));
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
                    Some(theme_editor::Action::Exit) => {
                        self.sidebar.set_menu(Some(sidebar::Menu::Settings));
                    }
                    Some(theme_editor::Action::UpdateTheme(theme)) => {
                        self.theme = data::Theme(theme);

                        let main_window = self.main_window.id;

                        if let Some(dashboard) = self.active_dashboard_mut() {
                            dashboard.invalidate_all_panes(main_window);
                        } else {
                            return Task::done(Message::ErrorOccurred(InternalError::Layout(
                                "No active dashboard".to_string(),
                            )));
                        }
                    }
                    None => {}
                }
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
                        .on_press(Message::Layouts(layout::Message::AddLayout))
                ]
                .align_x(Alignment::Center)
                .spacing(8),
            )
            .into();
        };

        let sidebar_pos = self.sidebar.position();

        let content = if id == self.main_window.id {
            let sidebar = {
                let is_table_open = self.tickers_table.is_open();

                let tickers_table_view = if is_table_open {
                    column![responsive(move |size| self
                        .tickers_table
                        .view(size)
                        .map(Message::TickersTable))]
                } else {
                    column![]
                };

                self.sidebar.view(
                    is_table_open,
                    self.audio_stream.volume(),
                    tickers_table_view.into(),
                )
            };

            let dashboard_view = dashboard
                .view(&self.main_window, self.timezone)
                .map(move |msg| Message::Dashboard(None, msg));

            let base = column![
                {
                    #[cfg(target_os = "macos")]
                    {
                        iced::widget::center(
                            text("FLOWSURFACE")
                                .font(iced::Font {
                                    weight: iced::font::Weight::Bold,
                                    ..Default::default()
                                })
                                .size(16)
                                .style(style::title_text)
                                .align_x(Alignment::Center),
                        )
                        .height(20)
                        .align_y(Alignment::Center)
                        .padding(padding::right(8).top(4))
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        column![]
                    }
                },
                match sidebar_pos {
                    sidebar::Position::Left => row![sidebar, dashboard_view,],
                    sidebar::Position::Right => row![dashboard_view, sidebar],
                }
                .spacing(4)
                .padding(8),
            ];

            if let Some(menu) = self.sidebar.active_menu() {
                self.sidebar
                    .view_with_modals(menu, dashboard, self, base.into(), id)
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

        let tickers_table_fetch = self.tickers_table.subscription().map(Message::TickersTable);

        let tick = iced::time::every(Duration::from_millis(100)).map(Message::Tick);

        Subscription::batch(vec![
            exchange_streams,
            tickers_table_fetch,
            window_events,
            tick,
        ])
    }

    fn active_dashboard(&self) -> Option<&Dashboard> {
        self.layout_manager.active_dashboard()
    }

    fn active_dashboard_mut(&mut self) -> Option<&mut Dashboard> {
        self.layout_manager.active_dashboard_mut()
    }
}
