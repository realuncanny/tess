use std::collections::BTreeMap;

use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Cache, Event, Geometry, Path};
use iced::widget::{Canvas, center, container, row, text, vertical_rule};
use iced::{Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector, mouse};

use crate::chart::{Basis, Caches, CommonChartData, Interaction, Message};
use crate::style::{self, get_dashed_line};
use data::util::{format_with_commas, round_to_tick};

const LABEL_BINNING: f32 = 0.0001;

pub fn create_indicator_elem<'a>(
    chart_state: &'a CommonChartData,
    cache: &'a Caches,
    data_points: &'a BTreeMap<u64, f32>,
    earliest: u64,
    latest: u64,
) -> Element<'a, Message> {
    let (mut max_value, mut min_value) = {
        match chart_state.basis {
            Basis::Time(_) => data_points
                .iter()
                .filter(|(timestamp, _)| **timestamp >= earliest && **timestamp <= latest)
                .fold((f32::MIN, f32::MAX), |(max, min), (_, value)| {
                    (max.max(*value), min.min(*value))
                }),
            Basis::Tick(_) => {
                return center(text("WIP: Funding Rate is not available for tick charts.")).into();
            }
        }
    };

    let value_range = max_value - min_value;
    let padding = value_range * 0.01;
    max_value += padding;
    min_value -= padding;

    let indi_chart = Canvas::new(FundingRate {
        indicator_cache: &cache.main,
        crosshair_cache: &cache.crosshair,
        chart_state,
        max_value,
        min_value,
        timeseries: data_points,
    })
    .height(Length::Fill)
    .width(Length::Fill);

    let indi_labels = Canvas::new(super::IndicatorLabel {
        label_cache: &cache.y_labels,
        max: max_value,
        min: min_value,
        crosshair: chart_state.crosshair,
        chart_bounds: chart_state.bounds,
        tick_size: LABEL_BINNING,
    })
    .height(Length::Fill)
    .width(Length::Fixed(64.0 + (chart_state.decimals as f32 * 4.0)));

    row![
        indi_chart,
        vertical_rule(1).style(style::split_ruler),
        container(indi_labels),
    ]
    .into()
}

pub struct FundingRate<'a> {
    pub indicator_cache: &'a Cache,
    pub crosshair_cache: &'a Cache,
    pub chart_state: &'a CommonChartData,
    pub max_value: f32,
    pub min_value: f32,
    pub timeseries: &'a BTreeMap<u64, f32>,
}

impl FundingRate<'_> {
    fn visible_region(&self, size: Size) -> Rectangle {
        let width = size.width / self.chart_state.scaling;
        let height = size.height / self.chart_state.scaling;

        Rectangle {
            x: -self.chart_state.translation.x - width / 2.0,
            y: 0.0,
            width,
            height,
        }
    }
}

impl canvas::Program<Message> for FundingRate<'_> {
    type State = Interaction;

    fn update(
        &self,
        interaction: &mut Interaction,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let message = match *interaction {
                    Interaction::None => {
                        if self.chart_state.crosshair && cursor.is_over(bounds) {
                            Some(Message::CrosshairMoved)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                let action =
                    message.map_or(canvas::Action::request_redraw(), canvas::Action::publish);

                Some(match interaction {
                    Interaction::None => action,
                    _ => action.and_capture(),
                })
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let chart_state = self.chart_state;

        if chart_state.bounds.width == 0.0 || self.max_value == 0.0 {
            return vec![];
        }

        let timeframe: u64 = match chart_state.basis {
            Basis::Time(interval) => interval,
            Basis::Tick(_) => {
                // TODO
                return vec![];
            }
        };

        let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);
        let palette = theme.extended_palette();

        let interval = {
            let mut rev_keys = self.timeseries.keys().rev();
            let latest = rev_keys.next().copied().unwrap_or(0);
            let prev = rev_keys.next().copied().unwrap_or(latest);

            latest.saturating_sub(prev)
        };

        let indicator = self.indicator_cache.draw(renderer, bounds.size(), |frame| {
            frame.translate(center);
            frame.scale(chart_state.scaling);
            frame.translate(Vector::new(
                chart_state.translation.x,
                (-bounds.height / chart_state.scaling) / 2.0,
            ));

            let region = self.visible_region(frame.size());
            let (mut earliest, latest) = chart_state.interval_range(&region);

            earliest = earliest.saturating_sub(interval);

            let mut max_value = f32::MIN;
            let mut min_value = f32::MAX;

            self.timeseries
                .range(earliest..=latest)
                .for_each(|(_, value)| {
                    max_value = max_value.max(*value);
                    min_value = min_value.min(*value);
                });

            let padding = (max_value - min_value) * 0.08;
            max_value += padding;
            min_value -= padding;

            let points = self
                .timeseries
                .range(earliest..=latest)
                .map(|(timestamp, value)| {
                    let x_position = chart_state.interval_to_x(*timestamp);
                    let normalized_height = if max_value > min_value {
                        (value - min_value) / (max_value - min_value)
                    } else {
                        0.0
                    };
                    let y_position = (bounds.height / chart_state.scaling)
                        - (normalized_height * (bounds.height / chart_state.scaling));

                    Point::new(x_position - (chart_state.cell_width / 2.0), y_position)
                })
                .collect::<Vec<Point>>();

            if let Some(first_point) = points.first() {
                let first_value = self
                    .timeseries
                    .range(earliest..=latest)
                    .next()
                    .map(|(_, v)| *v)
                    .unwrap_or(0.0);

                let first_size = Size::new(
                    chart_state.interval_to_x(earliest)
                        - (chart_state.cell_width / 2.0)
                        - first_point.x,
                    1.0,
                );

                draw_colored_rect(frame, *first_point, first_size, first_value, palette);
            }

            let mut value_iter = self.timeseries.range(earliest..=latest).map(|(_, v)| *v);
            for segment in points.windows(2) {
                let value = value_iter.next().unwrap_or(0.0);
                let width = segment[1].x - segment[0].x;
                draw_colored_rect(
                    frame,
                    Point::new(segment[0].x, segment[0].y),
                    Size::new(width, 1.0),
                    value,
                    palette,
                );
            }

            if let Some(last_point) = points.last() {
                let next_expected = {
                    let mut rev_keys = self.timeseries.keys().rev();
                    let latest = rev_keys.next().copied().unwrap_or(0);
                    let prev = rev_keys.next().copied().unwrap_or(latest);

                    latest + latest.saturating_sub(prev)
                };

                let last_value = self
                    .timeseries
                    .range(earliest..=latest)
                    .last()
                    .map(|(_, v)| *v)
                    .unwrap_or(0.0);

                let last_size = Size::new(
                    chart_state.interval_to_x(next_expected)
                        - (chart_state.cell_width / 2.0)
                        - last_point.x,
                    1.0,
                );

                draw_colored_rect(frame, *last_point, last_size, last_value, palette);
            }
        });

        if chart_state.crosshair {
            let crosshair = self.crosshair_cache.draw(renderer, bounds.size(), |frame| {
                let dashed_line = get_dashed_line(theme);

                if let Some(cursor_position) = cursor.position_in(chart_state.bounds) {
                    let region = self.visible_region(frame.size());

                    // Vertical time line
                    let earliest = chart_state.x_to_interval(region.x) as f64;
                    let latest = chart_state.x_to_interval(region.x + region.width) as f64;

                    let crosshair_ratio = f64::from(cursor_position.x / bounds.width);
                    let crosshair_millis = earliest + crosshair_ratio * (latest - earliest);

                    let rounded_timestamp =
                        (crosshair_millis / (timeframe as f64)).round() as u64 * timeframe;
                    let snap_ratio =
                        ((rounded_timestamp as f64 - earliest) / (latest - earliest)) as f32;

                    frame.stroke(
                        &Path::line(
                            Point::new(snap_ratio * bounds.width, 0.0),
                            Point::new(snap_ratio * bounds.width, bounds.height),
                        ),
                        dashed_line,
                    );

                    let fr_data = {
                        self.timeseries
                            .range(..=rounded_timestamp)
                            .next_back()
                            .or_else(|| self.timeseries.iter().next())
                    };

                    if let Some((_, fr_value)) = fr_data {
                        let value_text = format!("Value: {}", format_with_commas(*fr_value));
                        let tooltip_bg_width = value_text.len() as f32 * 8.0;

                        frame.fill_rectangle(
                            Point::new(4.0, 0.0),
                            Size::new(tooltip_bg_width, 14.0),
                            palette.background.weakest.color.scale_alpha(0.9),
                        );

                        let text = canvas::Text {
                            content: value_text,
                            position: Point::new(8.0, 2.0),
                            size: iced::Pixels(9.0),
                            color: palette.background.base.text,
                            font: style::AZERET_MONO,
                            ..canvas::Text::default()
                        };
                        frame.fill_text(text);
                    }
                } else if let Some(cursor_position) = cursor.position_in(bounds) {
                    // Horizontal price line
                    let highest = self.max_value;
                    let lowest = self.min_value;

                    let crosshair_ratio = cursor_position.y / bounds.height;
                    let crosshair_price = highest + crosshair_ratio * (lowest - highest);

                    let rounded_price = round_to_tick(crosshair_price, LABEL_BINNING);
                    let snap_ratio = (rounded_price - highest) / (lowest - highest);

                    frame.stroke(
                        &Path::line(
                            Point::new(0.0, snap_ratio * bounds.height),
                            Point::new(bounds.width, snap_ratio * bounds.height),
                        ),
                        dashed_line,
                    );
                }
            });

            vec![indicator, crosshair]
        } else {
            vec![indicator]
        }
    }

    fn mouse_interaction(
        &self,
        interaction: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match interaction {
            Interaction::Panning { .. } => mouse::Interaction::Grabbing,
            Interaction::Zoomin { .. } => mouse::Interaction::ZoomIn,
            Interaction::None if cursor.is_over(bounds) => {
                if self.chart_state.crosshair {
                    mouse::Interaction::Crosshair
                } else {
                    mouse::Interaction::default()
                }
            }
            _ => mouse::Interaction::default(),
        }
    }
}

fn draw_colored_rect(
    frame: &mut canvas::Frame,
    point: Point,
    size: Size,
    value: f32,
    palette: &Extended,
) {
    let positive_color = palette.danger.strong.color;
    let negative_color = palette.success.strong.color;

    let alpha = 0.1 + 0.9 * (value.abs() / 0.02).min(1.0);
    let overlay_color = if value >= 0.0 {
        positive_color.scale_alpha(alpha)
    } else {
        negative_color.scale_alpha(alpha)
    };

    frame.fill_rectangle(point, size, palette.primary.weak.color.scale_alpha(0.4));
    frame.fill_rectangle(point, size, overlay_color);
}
