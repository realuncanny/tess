use crate::{screen::dashboard::pane::Message, style};

use data::chart::Basis;
use exchange::{TickMultiplier, Ticker, Timeframe, adapter::Exchange};
use iced::{
    Alignment, Element, Length,
    alignment::{Horizontal, Vertical},
    padding,
    widget::{button, column, container, pane_grid, row, scrollable, text},
};

#[derive(Debug, Clone, Copy)]
pub enum StreamModifier {
    Candlestick(Basis),
    Footprint(Basis, TickMultiplier),
    Heatmap(Basis, TickMultiplier),
}

pub fn view<'a>(
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
            Basis::Time(selected_tf) => {
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
                        let msg = match exchange::Timeframe::try_from(selected_tf) {
                            Ok(tf) if *timeframe == tf => None,
                            _ => Some(Message::BasisSelected(Basis::Time(*timeframe), pane)),
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

                        let msg = match exchange::Timeframe::try_from(selected_tf) {
                            Ok(tf) if *timeframe == tf => None,
                            _ => Some(Message::BasisSelected(Basis::Time(*timeframe), pane)),
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
