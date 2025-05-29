pub mod audio;
pub mod layout_manager;
pub mod study;
pub mod theme_editor;

pub use layout_manager::LayoutManager;
pub use theme_editor::ThemeEditor;

use crate::screen::dashboard::pane::{self, Message};
use crate::screen::dashboard::panel::timeandsales;
use crate::style::{Icon, icon_text};
use crate::widget::{classic_slider_row, column_drag, dragger_row, labeled_slider};
use crate::{style, tooltip, widget::scrollable_content};

use data::chart::{
    Basis, KlineChartKind, VisualConfig,
    heatmap::{self, CoalesceKind},
    indicator::Indicator,
    kline::ClusterKind,
};
use data::util::format_with_commas;
use exchange::{TickMultiplier, Ticker, Timeframe, adapter::Exchange};
use iced::{
    Alignment, Element, Length,
    alignment::{Horizontal, Vertical},
    padding,
    widget::{
        button, column, container, horizontal_rule, horizontal_space, pane_grid, pick_list, radio,
        row, scrollable, slider, text, tooltip::Position as TooltipPosition,
    },
};

pub fn heatmap_cfg_view<'a>(cfg: heatmap::Config, pane: pane_grid::Pane) -> Element<'a, Message> {
    let trade_size_slider = {
        let filter = cfg.trade_size_filter;

        labeled_slider(
            "Trade",
            0.0..=50000.0,
            filter,
            move |value| {
                Message::VisualConfigChanged(
                    Some(pane),
                    VisualConfig::Heatmap(heatmap::Config {
                        trade_size_filter: value,
                        ..cfg
                    }),
                )
            },
            |value| format!("${}", format_with_commas(*value)),
            Some(500.0),
        )
    };

    let order_size_slider = {
        let filter = cfg.order_size_filter;

        labeled_slider(
            "Order",
            0.0..=500_000.0,
            filter,
            move |value| {
                Message::VisualConfigChanged(
                    Some(pane),
                    VisualConfig::Heatmap(heatmap::Config {
                        order_size_filter: value,
                        ..cfg
                    }),
                )
            },
            |value| format!("${}", format_with_commas(*value)),
            Some(5000.0),
        )
    };

    let circle_scaling_slider = {
        if let Some(radius_scale) = cfg.trade_size_scale {
            classic_slider_row(
                text("Circle radius scaling"),
                slider(10..=200, radius_scale, move |value| {
                    Message::VisualConfigChanged(
                        Some(pane),
                        VisualConfig::Heatmap(heatmap::Config {
                            trade_size_scale: Some(value),
                            ..cfg
                        }),
                    )
                })
                .step(10)
                .into(),
                Some(text(format!("{}%", radius_scale)).size(13)),
            )
        } else {
            container(row![]).into()
        }
    };

    let coalescer_cfg: Element<_> = {
        if let Some(coalescing) = cfg.coalescing {
            let threshold_pct = coalescing.threshold();

            let coalescer_kinds = {
                let average = radio(
                    "Average",
                    CoalesceKind::Average(threshold_pct),
                    Some(coalescing),
                    move |value| {
                        Message::VisualConfigChanged(
                            Some(pane),
                            VisualConfig::Heatmap(heatmap::Config {
                                coalescing: Some(value),
                                ..cfg
                            }),
                        )
                    },
                )
                .spacing(4);

                let first = radio(
                    "First",
                    CoalesceKind::First(threshold_pct),
                    Some(coalescing),
                    move |value| {
                        Message::VisualConfigChanged(
                            Some(pane),
                            VisualConfig::Heatmap(heatmap::Config {
                                coalescing: Some(value),
                                ..cfg
                            }),
                        )
                    },
                )
                .spacing(4);

                let max = radio(
                    "Max",
                    CoalesceKind::Max(threshold_pct),
                    Some(coalescing),
                    move |value| {
                        Message::VisualConfigChanged(
                            Some(pane),
                            VisualConfig::Heatmap(heatmap::Config {
                                coalescing: Some(value),
                                ..cfg
                            }),
                        )
                    },
                )
                .spacing(4);

                row![
                    text("Merge method: "),
                    row![average, first, max].spacing(12)
                ]
                .spacing(12)
            };

            let threshold_slider = classic_slider_row(
                text("Size similarity"),
                slider(0.05..=0.8, threshold_pct, move |value| {
                    Message::VisualConfigChanged(
                        Some(pane),
                        VisualConfig::Heatmap(heatmap::Config {
                            coalescing: Some(coalescing.with_threshold(value)),
                            ..cfg
                        }),
                    )
                })
                .step(0.05)
                .into(),
                Some(text(format!("{:.0}%", threshold_pct * 100.0)).size(13)),
            );

            container(column![coalescer_kinds, threshold_slider,].spacing(8))
                .style(style::modal_container)
                .padding(8)
                .into()
        } else {
            row![].into()
        }
    };

    container(scrollable_content(
        column![
            column![
                text("Size filters").size(14),
                column![trade_size_slider, order_size_slider].spacing(8),
            ]
            .spacing(20)
            .padding(16)
            .align_x(Alignment::Start),
            horizontal_rule(1),
            column![
                text("Noise filters").size(14),
                iced::widget::checkbox(
                    "Merge orders if sizes are similar",
                    cfg.coalescing.is_some(),
                )
                .on_toggle(move |value| {
                    Message::VisualConfigChanged(
                        Some(pane),
                        VisualConfig::Heatmap(heatmap::Config {
                            coalescing: if value {
                                Some(CoalesceKind::Average(0.15))
                            } else {
                                None
                            },
                            ..cfg
                        }),
                    )
                }),
                coalescer_cfg,
            ]
            .spacing(20)
            .padding(16)
            .align_x(Alignment::Start),
            horizontal_rule(1),
            column![
                text("Trade visualization").size(14),
                iced::widget::checkbox("Dynamic circle radius", cfg.trade_size_scale.is_some(),)
                    .on_toggle(move |value| {
                        Message::VisualConfigChanged(
                            Some(pane),
                            VisualConfig::Heatmap(heatmap::Config {
                                trade_size_scale: if value { Some(100) } else { None },
                                ..cfg
                            }),
                        )
                    }),
                circle_scaling_slider,
            ]
            .spacing(20)
            .padding(16)
            .width(Length::Fill)
            .align_x(Alignment::Start),
            horizontal_rule(1),
            row![
                horizontal_space(),
                sync_all_button(VisualConfig::Heatmap(cfg))
            ],
        ]
        .spacing(8),
    ))
    .width(Length::Shrink)
    .padding(16)
    .max_width(360)
    .style(style::chart_modal)
    .into()
}

pub fn timesales_cfg_view<'a>(
    cfg: timeandsales::Config,
    pane: pane_grid::Pane,
) -> Element<'a, Message> {
    let trade_size_slider = {
        let filter = cfg.trade_size_filter;

        labeled_slider(
            "Trade",
            0.0..=50000.0,
            filter,
            move |value| {
                Message::VisualConfigChanged(
                    Some(pane),
                    VisualConfig::TimeAndSales(timeandsales::Config {
                        trade_size_filter: value,
                    }),
                )
            },
            |value| format!("${}", format_with_commas(*value)),
            Some(500.0),
        )
    };

    container(scrollable_content(
        column![
            column![text("Size filter").size(14), trade_size_slider,]
                .spacing(20)
                .padding(16)
                .align_x(Alignment::Center),
            row![
                horizontal_space(),
                sync_all_button(VisualConfig::TimeAndSales(cfg))
            ],
        ]
        .spacing(8),
    ))
    .width(Length::Shrink)
    .padding(16)
    .max_width(500)
    .style(style::chart_modal)
    .into()
}

fn sync_all_button<'a>(config: VisualConfig) -> Element<'a, Message> {
    container(tooltip(
        button("Sync all")
            .on_press(Message::VisualConfigChanged(None, config))
            .padding(8),
        Some("Apply configuration to similar panes"),
        TooltipPosition::Top,
    ))
    .padding(16)
    .into()
}

pub fn kline_cfg_view<'a>(
    study_config: &'a study::ChartStudy,
    kind: &'a KlineChartKind,
    pane: pane_grid::Pane,
) -> Element<'a, Message> {
    match kind {
        KlineChartKind::Candles => container(text(
            "This chart type doesn't have any configurations, WIP...",
        ))
        .padding(16)
        .width(Length::Shrink)
        .max_width(500)
        .style(style::chart_modal)
        .into(),
        KlineChartKind::Footprint { clusters, studies } => {
            let cluster_picklist =
                pick_list(ClusterKind::ALL, Some(clusters), move |new_cluster_kind| {
                    Message::ClusterKindSelected(pane, new_cluster_kind)
                });

            let study_cfg = study_config
                .view(studies)
                .map(move |msg| Message::StudyConfigurator(pane, msg));

            container(scrollable_content(
                column![
                    column![text("Clustering type").size(14), cluster_picklist].spacing(4),
                    column![text("Footprint studies").size(14), study_cfg].spacing(4),
                ]
                .spacing(20)
                .padding(16)
                .align_x(Alignment::Start),
            ))
            .width(Length::Shrink)
            .max_width(320)
            .padding(16)
            .style(style::chart_modal)
            .into()
        }
    }
}

pub fn indicators_view<'a, I: Indicator>(
    pane: pane_grid::Pane,
    state: &'a pane::State,
    selected: &[I],
) -> Element<'a, Message> {
    let market_type = state.settings.ticker_info.map(|info| info.market_type());

    let build_indicators = |allows_drag: bool| -> Element<'a, Message> {
        if let Some(market) = market_type {
            let indicator_row_elem_fn = |indicator: &I, is_selected_indicator: bool| {
                let content = if is_selected_indicator {
                    row![
                        text(indicator.to_string()),
                        horizontal_space(),
                        container(icon_text(Icon::Checkmark, 12)),
                    ]
                    .width(Length::Fill)
                } else {
                    row![text(indicator.to_string())].width(Length::Fill)
                };

                button(content)
                    .on_press(Message::ToggleIndicator(pane, indicator.to_string()))
                    .width(Length::Fill)
                    .style(move |theme, status| {
                        style::button::modifier(theme, status, is_selected_indicator)
                    })
                    .into()
            };

            let mut base_row_elements: Vec<Element<_>> = vec![];

            for indicator in selected {
                base_row_elements.push(indicator_row_elem_fn(indicator, true));
            }

            for indicator in I::for_market(market) {
                if !selected.contains(indicator) {
                    base_row_elements.push(indicator_row_elem_fn(indicator, false));
                }
            }

            let reorderable = allows_drag && selected.len() >= 2;

            let all_indicator_elements: Vec<Element<_>> = base_row_elements
                .into_iter()
                .map(|base_content| {
                    if reorderable {
                        dragger_row(base_content)
                    } else {
                        container(base_content)
                            .padding(2)
                            .style(style::dragger_row_container)
                            .into()
                    }
                })
                .collect();

            let indicators_list_content: Element<_> = if reorderable {
                let mut draggable_column = column_drag::Column::new()
                    .on_drag(move |event| Message::ReorderIndicator(pane, event))
                    .spacing(4);
                for element in all_indicator_elements {
                    draggable_column = draggable_column.push(element);
                }
                draggable_column.into()
            } else {
                iced::widget::Column::with_children(all_indicator_elements)
                    .spacing(4)
                    .into()
            };

            column![
                container(text("Indicators").size(14)).padding(padding::bottom(8)),
                indicators_list_content
            ]
            .spacing(4)
            .into()
        } else {
            column![].spacing(4).into()
        }
    };

    let content_allows_dragging = matches!(state.content, pane::Content::Kline(_, _));
    let content_row = build_indicators(content_allows_dragging);

    container(content_row)
        .max_width(200)
        .padding(16)
        .style(style::chart_modal)
        .into()
}

#[derive(Debug, Clone, Copy)]
pub enum StreamModifier {
    Candlestick(Basis),
    Footprint(Basis, TickMultiplier),
    Heatmap(Basis, TickMultiplier),
}

pub fn stream_modifier_view<'a>(
    pane: pane_grid::Pane,
    modifiers: StreamModifier,
    ticker_info: Option<(Exchange, Ticker)>,
) -> Element<'a, Message> {
    let (selected_basis, selected_ticksize) = match modifiers {
        StreamModifier::Candlestick(basis) => (Some(basis), None),
        StreamModifier::Footprint(basis, ticksize) | StreamModifier::Heatmap(basis, ticksize) => {
            (Some(basis), Some(ticksize))
        }
    };

    let create_button = |content: String, msg: Option<Message>, active: bool| {
        let btn = button(container(text(content)).align_x(Horizontal::Center))
            .width(Length::Fill)
            .style(move |theme, status| style::button::transparent(theme, status, active));

        if let Some(msg) = msg {
            btn.on_press(msg)
        } else {
            btn
        }
    };

    let mut content_row = row![].align_y(Vertical::Center).spacing(16);

    let mut timeframes_column = column![].padding(4).align_x(Horizontal::Center);
    let mut tick_basis_column = column![].padding(4).align_x(Horizontal::Center);

    let is_kline_chart = match modifiers {
        StreamModifier::Candlestick(_) | StreamModifier::Footprint(_, _) => true,
        StreamModifier::Heatmap(_, _) => false,
    };

    if let Some(basis) = selected_basis {
        match basis {
            Basis::Time(selected_timeframe) => {
                timeframes_column = timeframes_column.push(if is_kline_chart {
                    row![
                        create_button("Timeframe".to_string(), None, false,),
                        create_button(
                            "Ticks".to_string(),
                            Some(Message::BasisSelected(Basis::Tick(200), pane,)),
                            true,
                        ),
                    ]
                    .padding(padding::bottom(8))
                    .spacing(4)
                } else {
                    row![text("Aggregation")]
                        .padding(padding::bottom(8))
                        .spacing(4)
                });

                if is_kline_chart {
                    for timeframe in &Timeframe::KLINE {
                        let msg = if *timeframe == selected_timeframe.into() {
                            None
                        } else {
                            Some(Message::BasisSelected(
                                Basis::Time(u64::from(*timeframe)),
                                pane,
                            ))
                        };
                        timeframes_column = timeframes_column.push(create_button(
                            timeframe.to_string(),
                            msg,
                            false,
                        ));
                    }
                } else if let Some((exchange, _)) = ticker_info {
                    for timeframe in &Timeframe::HEATMAP {
                        if exchange == Exchange::BybitSpot && timeframe == &Timeframe::MS100 {
                            continue;
                        }

                        let msg = if *timeframe == selected_timeframe.into() {
                            None
                        } else {
                            Some(Message::BasisSelected(
                                Basis::Time(u64::from(*timeframe)),
                                pane,
                            ))
                        };
                        timeframes_column = timeframes_column.push(create_button(
                            timeframe.to_string(),
                            msg,
                            false,
                        ));
                    }
                }

                content_row =
                    content_row.push(container(timeframes_column).style(style::modal_container));
            }
            Basis::Tick(selected_tick) => {
                tick_basis_column = tick_basis_column.push(
                    row![
                        create_button(
                            "Timeframe".to_string(),
                            Some(Message::BasisSelected(
                                Basis::Time(Timeframe::M5.into()),
                                pane
                            )),
                            true,
                        ),
                        create_button("Ticks".to_string(), None, false,),
                    ]
                    .padding(padding::bottom(8))
                    .spacing(4),
                );

                for tick_count in &data::aggr::TickCount::ALL {
                    let msg = if *tick_count == selected_tick.into() {
                        None
                    } else {
                        Some(Message::BasisSelected(
                            Basis::Tick(u64::from(*tick_count)),
                            pane,
                        ))
                    };
                    tick_basis_column =
                        tick_basis_column.push(create_button(tick_count.to_string(), msg, false));
                }

                content_row =
                    content_row.push(container(tick_basis_column).style(style::modal_container));
            }
        }
    }

    let mut ticksizes_column = column![].padding(4).align_x(Horizontal::Center);

    if selected_ticksize.is_some() {
        ticksizes_column =
            ticksizes_column.push(container(text("Ticksize Mltp.")).padding(padding::bottom(8)));

        for ticksize in &exchange::TickMultiplier::ALL {
            let msg = if selected_ticksize == Some(*ticksize) {
                None
            } else {
                Some(Message::TicksizeSelected(*ticksize, pane))
            };
            ticksizes_column =
                ticksizes_column.push(create_button(ticksize.to_string(), msg, false));
        }

        content_row = content_row.push(container(ticksizes_column).style(style::modal_container));
    }

    container(scrollable::Scrollable::with_direction(
        content_row.align_y(Alignment::Start),
        scrollable::Direction::Vertical(scrollable::Scrollbar::new().width(4).scroller_width(4)),
    ))
    .padding(16)
    .max_width(if selected_ticksize.is_some() && selected_basis.is_some() {
        380
    } else if selected_basis.is_some() {
        200
    } else {
        120
    })
    .style(style::chart_modal)
    .into()
}
