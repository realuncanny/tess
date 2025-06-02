pub mod linear;
pub mod timeseries;

use crate::style::AZERET_MONO;

use super::{Basis, Interaction, Message};
use chrono::{DateTime, Datelike, Duration, Months};
use data::util::{reset_to_start_of_year_utc, round_to_tick};
use data::{UserTimezone, util::reset_to_start_of_month_utc};
use iced::{
    Alignment, Color, Event, Point, Rectangle, Renderer, Size, Theme, mouse,
    theme::palette::Extended,
    widget::canvas::{self, Cache, Frame, Geometry},
};

const ONE_DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// calculates `Rectangle` from given content, clamps it within bounds if needed
pub fn calc_label_rect(
    y_pos: f32,
    content_amt: i16,
    text_size: f32,
    bounds: Rectangle,
) -> Rectangle {
    let content_amt = content_amt.max(1);
    let label_height = text_size + (f32::from(content_amt) * (text_size / 2.0) + 4.0);

    let rect = Rectangle {
        x: 1.0,
        y: y_pos - label_height / 2.0,
        width: bounds.width - 1.0,
        height: label_height,
    };

    // clamp when label is partially visible within bounds
    if rect.y < bounds.height && rect.y + label_height > 0.0 {
        Rectangle {
            y: rect.y.clamp(0.0, (bounds.height - label_height).max(0.0)),
            ..rect
        }
    } else {
        rect
    }
}

#[derive(Debug, Clone)]
pub struct Label {
    pub content: String,
    pub background_color: Option<Color>,
    pub text_color: Color,
    pub text_size: f32,
}

#[derive(Debug, Clone)]
pub enum AxisLabel {
    X(Rectangle, Label),
    Y(Rectangle, Label, Option<Label>),
}

impl AxisLabel {
    fn intersects(&self, other: &AxisLabel) -> bool {
        match (self, other) {
            (AxisLabel::Y(self_rect, ..), AxisLabel::Y(other_rect, ..))
            | (AxisLabel::X(self_rect, ..), AxisLabel::X(other_rect, ..)) => {
                self_rect.intersects(other_rect)
            }
            _ => false,
        }
    }

    pub fn filter_and_draw(labels: &[AxisLabel], frame: &mut Frame) {
        for i in (0..labels.len()).rev() {
            let should_draw = labels[i + 1..]
                .iter()
                .all(|existing| !existing.intersects(&labels[i]));

            if should_draw {
                labels[i].draw(frame);
            }
        }
    }

    fn draw(&self, frame: &mut Frame) {
        match self {
            AxisLabel::X(rect, label) => {
                if let Some(background_color) = label.background_color {
                    frame.fill_rectangle(
                        Point::new(rect.x, rect.y),
                        Size::new(rect.width, rect.height),
                        background_color,
                    );
                }

                let label = canvas::Text {
                    content: label.content.clone(),
                    position: rect.center(),
                    size: label.text_size.into(),
                    color: label.text_color,
                    align_y: Alignment::Center.into(),
                    align_x: Alignment::Center.into(),
                    font: AZERET_MONO,
                    ..canvas::Text::default()
                };

                frame.fill_text(label);
            }
            AxisLabel::Y(rect, value_label, timer_label) => {
                if let Some(background_color) = value_label.background_color {
                    frame.fill_rectangle(
                        Point::new(rect.x, rect.y),
                        Size::new(rect.width, rect.height),
                        background_color,
                    );
                }

                if let Some(timer_label) = timer_label {
                    let value_label = canvas::Text {
                        content: value_label.content.clone(),
                        position: Point::new(rect.x + 4.0, rect.y + 2.0),
                        color: value_label.text_color,
                        size: value_label.text_size.into(),
                        font: AZERET_MONO,
                        ..canvas::Text::default()
                    };

                    frame.fill_text(value_label);

                    let timer_label = canvas::Text {
                        content: timer_label.content.clone(),
                        position: Point::new(rect.x + 4.0, rect.y + 15.0),
                        color: timer_label.text_color,
                        size: timer_label.text_size.into(),
                        font: AZERET_MONO,
                        ..canvas::Text::default()
                    };

                    frame.fill_text(timer_label);
                } else {
                    let value_label = canvas::Text {
                        content: value_label.content.clone(),
                        position: Point::new(rect.x + 4.0, rect.y + 4.0),
                        color: value_label.text_color,
                        size: value_label.text_size.into(),
                        font: AZERET_MONO,
                        ..canvas::Text::default()
                    };

                    frame.fill_text(value_label);
                }
            }
        }
    }
}

// X-AXIS LABELS
const TEXT_SIZE: f32 = 12.0;

pub struct AxisLabelsX<'a> {
    pub labels_cache: &'a Cache,
    pub crosshair: bool,
    pub max: u64,
    pub scaling: f32,
    pub translation_x: f32,
    pub basis: Basis,
    pub cell_width: f32,
    pub timezone: UserTimezone,
    pub chart_bounds: Rectangle,
    pub interval_keys: Option<Vec<u64>>,
}

impl AxisLabelsX<'_> {
    fn create_label(
        position: f32,
        text: String,
        bounds: Rectangle,
        is_crosshair: bool,
        palette: &Extended,
    ) -> AxisLabel {
        let content_width = text.len() as f32 * (TEXT_SIZE / 2.6);

        let rect = Rectangle {
            x: position - content_width,
            y: 4.0,
            width: 2.0 * content_width,
            height: bounds.height - 8.0,
        };

        let label = Label {
            content: text,
            background_color: if is_crosshair {
                Some(palette.secondary.base.color)
            } else {
                None
            },
            text_color: if is_crosshair {
                palette.secondary.base.text
            } else {
                palette.background.base.text
            },
            text_size: 12.0,
        };

        AxisLabel::X(rect, label)
    }

    fn generate_tick_labels(
        &self,
        region: Rectangle,
        bounds: Rectangle,
        palette: &Extended,
        x_labels_can_fit: i32,
    ) -> Vec<AxisLabel> {
        let Some(interval_keys) = &self.interval_keys else {
            return Vec::new();
        };

        let chart_x_min = region.x;
        let chart_x_max = region.x + region.width;

        let last_index = interval_keys.len() - 1;

        let min_cell = (chart_x_min / self.cell_width).floor() as i32;
        let max_cell = ((chart_x_max) / self.cell_width).ceil() as i32;

        let min_cell = min_cell.max(-((last_index + 1) as i32));

        let visible_cell_count = (max_cell - min_cell + 1).max(1) as f32;
        let step_size = (visible_cell_count / x_labels_can_fit as f32).ceil() as usize;

        let mut labels = Vec::with_capacity(interval_keys.len().min(x_labels_can_fit as usize));
        for cell_index in (min_cell..=max_cell).step_by(step_size.max(1)) {
            if cell_index > 0 {
                continue;
            }

            let offset = i64::from(-cell_index) as usize;
            if offset > last_index {
                continue;
            }

            let array_index = last_index - offset;
            let snapped_position = cell_index as f32 * self.cell_width;

            let snap_ratio = (snapped_position - chart_x_min) / (chart_x_max - chart_x_min);
            let snap_x = snap_ratio * bounds.width;

            if let Some(timestamp) = interval_keys.get(array_index) {
                let label_text = self
                    .timezone
                    .format_timestamp((*timestamp / 1000) as i64, exchange::Timeframe::MS100);

                labels.push(AxisLabelsX::create_label(
                    snap_x, label_text, bounds, false, palette,
                ));
            }
        }

        labels
    }

    fn generate_time_labels(
        &self,
        bounds: Rectangle,
        x_min: u64,
        x_max: u64,
        palette: &Extended,
        x_labels_can_fit: i32,
    ) -> Vec<AxisLabel> {
        let Basis::Time(timeframe) = self.basis else {
            return Vec::new();
        };

        let (time_step, initial_rounded_earliest) =
            timeseries::calc_time_step(x_min, x_max, x_labels_can_fit, timeframe);

        if time_step == 0 {
            return Vec::new();
        }

        let calculate_x_pos =
            |time_millis: u64, min_millis: u64, max_millis: u64, width: f32| -> f64 {
                if max_millis > min_millis {
                    ((time_millis - min_millis) as f64 / (max_millis - min_millis) as f64)
                        * f64::from(width)
                } else {
                    0.0
                }
            };

        let is_drawable = |x_pos: f64, width: f32| -> bool {
            x_pos >= (-TEXT_SIZE * 5.0).into()
                && x_pos <= f64::from(width) + (TEXT_SIZE * 5.0) as f64
        };

        let mut all_labels = Vec::with_capacity(x_labels_can_fit as usize * 3);

        if time_step >= ONE_DAY_MS {
            self.generate_daily_view_labels(
                &mut all_labels,
                bounds,
                x_min,
                x_max,
                palette,
                time_step,
                &calculate_x_pos,
                &is_drawable,
            );

            self.generate_monthly_view_labels(
                &mut all_labels,
                bounds,
                x_min,
                x_max,
                palette,
                &calculate_x_pos,
                &is_drawable,
            );

            self.generate_yearly_view_labels(
                &mut all_labels,
                bounds,
                x_min,
                x_max,
                palette,
                &calculate_x_pos,
                &is_drawable,
            );
        } else {
            self.generate_sub_daily_view_labels(
                &mut all_labels,
                bounds,
                x_min,
                x_max,
                palette,
                time_step,
                initial_rounded_earliest,
                timeframe,
                &calculate_x_pos,
                &is_drawable,
            );
        }

        all_labels
    }

    fn generate_daily_view_labels(
        &self,
        all_labels: &mut Vec<AxisLabel>,
        bounds: Rectangle,
        x_min: u64,
        x_max: u64,
        palette: &Extended,
        time_step: u64,
        calculate_x_pos: &dyn Fn(u64, u64, u64, f32) -> f64,
        is_drawable: &dyn Fn(f64, f32) -> bool,
    ) {
        if let Some(view_start_dt_utc) = DateTime::from_timestamp_millis(x_min as i64) {
            let mut current_month_loop_iter_utc = reset_to_start_of_month_utc(view_start_dt_utc);

            if current_month_loop_iter_utc.timestamp_millis() as u64 > x_min
                && current_month_loop_iter_utc.month() == view_start_dt_utc.month()
                && view_start_dt_utc.day() > 1
            {
                if let Some(prev_month_dt) =
                    current_month_loop_iter_utc.checked_sub_months(chrono::Months::new(1))
                {
                    current_month_loop_iter_utc = reset_to_start_of_month_utc(prev_month_dt);
                }
            }

            while current_month_loop_iter_utc.timestamp_millis() as u64 <= x_max {
                let month_actual_start_ts = current_month_loop_iter_utc.timestamp_millis() as u64;

                let next_month_boundary_utc = current_month_loop_iter_utc
                    .checked_add_months(chrono::Months::new(1))
                    .map(reset_to_start_of_month_utc)
                    .unwrap_or_else(|| {
                        DateTime::from_timestamp_millis(x_max as i64 + ONE_DAY_MS as i64)
                            .unwrap_or(current_month_loop_iter_utc)
                    });
                let next_month_boundary_ts = next_month_boundary_utc.timestamp_millis() as u64;

                if let Some(mut current_day_iter_utc) =
                    DateTime::from_timestamp_millis(month_actual_start_ts as i64)
                {
                    let mut iterations = 0;
                    loop {
                        let day_candidate_ts = current_day_iter_utc.timestamp_millis() as u64;

                        if day_candidate_ts >= next_month_boundary_ts || day_candidate_ts > x_max {
                            break;
                        }

                        if day_candidate_ts >= x_min {
                            let dt_in_timezone = match self.timezone {
                                UserTimezone::Local => {
                                    current_day_iter_utc.with_timezone(&chrono::Local)
                                }
                                UserTimezone::Utc => current_day_iter_utc.into(),
                            };

                            let is_jan_1st =
                                dt_in_timezone.month() == 1 && dt_in_timezone.day() == 1;

                            if !is_jan_1st {
                                let x_pos =
                                    calculate_x_pos(day_candidate_ts, x_min, x_max, bounds.width);
                                if is_drawable(x_pos, bounds.width) {
                                    let day_text = dt_in_timezone.format("%d").to_string();
                                    all_labels.push(AxisLabelsX::create_label(
                                        x_pos as f32,
                                        day_text,
                                        bounds,
                                        false,
                                        palette,
                                    ));
                                }
                            }
                        }

                        if let Some(next_dt) = current_day_iter_utc
                            .checked_add_signed(chrono::Duration::milliseconds(time_step as i64))
                        {
                            if next_dt.timestamp_millis() <= current_day_iter_utc.timestamp_millis()
                                && time_step > 0
                            {
                                break;
                            }
                            current_day_iter_utc = next_dt;
                        } else {
                            break;
                        }

                        iterations += 1;
                        if iterations > 60 {
                            break;
                        }
                    }
                }

                if let Some(next_m_start) =
                    current_month_loop_iter_utc.checked_add_months(chrono::Months::new(1))
                {
                    current_month_loop_iter_utc = reset_to_start_of_month_utc(next_m_start);

                    if current_month_loop_iter_utc.timestamp_millis() as u64 > x_max
                        && current_month_loop_iter_utc.month()
                            != DateTime::from_timestamp_millis(x_max as i64)
                                .map_or(0, |dt| dt.month())
                    {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
    }

    fn generate_monthly_view_labels(
        &self,
        all_labels: &mut Vec<AxisLabel>,
        bounds: Rectangle,
        x_min: u64,
        x_max: u64,
        palette: &Extended,
        calculate_x_pos: &dyn Fn(u64, u64, u64, f32) -> f64,
        is_drawable: &dyn Fn(f64, f32) -> bool,
    ) {
        if let Some(start_utc_dt) = DateTime::from_timestamp_millis(x_min as i64) {
            if let Some(end_utc_dt) = DateTime::from_timestamp_millis(x_max as i64) {
                let mut current_month_iter_utc = reset_to_start_of_month_utc(start_utc_dt);

                if current_month_iter_utc.timestamp_millis() < x_min as i64 {
                    if let Some(next_month_dt) =
                        current_month_iter_utc.checked_add_months(Months::new(1))
                    {
                        current_month_iter_utc = reset_to_start_of_month_utc(next_month_dt);
                    } else {
                        current_month_iter_utc = end_utc_dt
                            .checked_add_signed(Duration::days(1))
                            .unwrap_or(end_utc_dt);
                    }
                }

                while current_month_iter_utc.timestamp_millis() as u64 <= x_max {
                    let month_ts_millis = current_month_iter_utc.timestamp_millis() as u64;
                    if month_ts_millis >= x_min {
                        let dt_in_timezone = match self.timezone {
                            UserTimezone::Local => {
                                current_month_iter_utc.with_timezone(&chrono::Local)
                            }
                            UserTimezone::Utc => current_month_iter_utc.into(),
                        };
                        let is_january = dt_in_timezone.month() == 1;

                        if !is_january {
                            let x_position_month =
                                calculate_x_pos(month_ts_millis, x_min, x_max, bounds.width);
                            if is_drawable(x_position_month, bounds.width) {
                                let month_label_text = dt_in_timezone.format("%b").to_string();
                                all_labels.push(AxisLabelsX::create_label(
                                    x_position_month as f32,
                                    month_label_text,
                                    bounds,
                                    false,
                                    palette,
                                ));
                            }
                        }
                    }

                    if let Some(next_month_dt) =
                        current_month_iter_utc.checked_add_months(Months::new(1))
                    {
                        current_month_iter_utc = reset_to_start_of_month_utc(next_month_dt);

                        if current_month_iter_utc.timestamp_millis() > x_max as i64
                            && current_month_iter_utc.month() != end_utc_dt.month()
                        {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    }

    fn generate_yearly_view_labels(
        &self,
        all_labels: &mut Vec<AxisLabel>,
        bounds: Rectangle,
        x_min: u64,
        x_max: u64,
        palette: &Extended,
        calculate_x_pos: &dyn Fn(u64, u64, u64, f32) -> f64,
        is_drawable: &dyn Fn(f64, f32) -> bool,
    ) {
        if let Some(view_start_dt_utc) = DateTime::from_timestamp_millis(x_min as i64) {
            if let Some(view_end_dt_utc) = DateTime::from_timestamp_millis(x_max as i64) {
                let mut current_year_iter_utc = reset_to_start_of_year_utc(view_start_dt_utc);

                if current_year_iter_utc.timestamp_millis() < x_min as i64 {
                    if let Some(next_year_candidate) =
                        current_year_iter_utc.checked_add_months(Months::new(12))
                    {
                        current_year_iter_utc = reset_to_start_of_year_utc(next_year_candidate);
                    } else {
                        current_year_iter_utc = view_end_dt_utc
                            .checked_add_signed(Duration::days(365 * 2))
                            .unwrap_or(view_end_dt_utc);
                    }
                }

                while current_year_iter_utc.timestamp_millis() as u64 <= x_max {
                    let year_ts_millis = current_year_iter_utc.timestamp_millis() as u64;
                    if year_ts_millis >= x_min {
                        let x_position_year =
                            calculate_x_pos(year_ts_millis, x_min, x_max, bounds.width);

                        if is_drawable(x_position_year, bounds.width) {
                            let year_label_text = current_year_iter_utc.format("%Y").to_string();
                            all_labels.push(AxisLabelsX::create_label(
                                x_position_year as f32,
                                year_label_text,
                                bounds,
                                false,
                                palette,
                            ));
                        }
                    }

                    if let Some(next_year_dt) =
                        current_year_iter_utc.checked_add_months(Months::new(12))
                    {
                        current_year_iter_utc = reset_to_start_of_year_utc(next_year_dt);
                    } else {
                        break;
                    }
                }
            }
        }
    }

    fn generate_sub_daily_view_labels(
        &self,
        all_labels: &mut Vec<AxisLabel>,
        bounds: Rectangle,
        x_min: u64,
        x_max: u64,
        palette: &Extended,
        time_step: u64,
        initial_rounded_earliest: u64,
        timeframe: exchange::Timeframe,
        calculate_x_pos: &dyn Fn(u64, u64, u64, f32) -> f64,
        is_drawable: &dyn Fn(f64, f32) -> bool,
    ) {
        let mut current_time = initial_rounded_earliest;
        while current_time <= x_max {
            if current_time >= x_min {
                let x_position = calculate_x_pos(current_time, x_min, x_max, bounds.width);

                if is_drawable(x_position, bounds.width) {
                    let label_text = self
                        .timezone
                        .format_timestamp((current_time / 1000) as i64, timeframe);
                    all_labels.push(AxisLabelsX::create_label(
                        x_position as f32,
                        label_text,
                        bounds,
                        false,
                        palette,
                    ));
                }
            }
            let prev_current_time = current_time;
            current_time = current_time.saturating_add(time_step);
            if current_time <= prev_current_time && time_step > 0 {
                break;
            }

            if current_time > x_max && prev_current_time < x_min {
                break;
            }
        }
    }

    fn calc_crosshair_pos(&self, cursor_pos: Point, region: Rectangle) -> (f32, f32, i32) {
        let crosshair_ratio = f64::from(cursor_pos.x) / f64::from(self.chart_bounds.width);
        let chart_x_min = region.x;
        let crosshair_pos = chart_x_min + crosshair_ratio as f32 * region.width;
        let cell_index = (crosshair_pos / self.cell_width).round();

        (crosshair_pos, crosshair_ratio as f32, cell_index as i32)
    }

    fn generate_crosshair(
        &self,
        cursor_pos: Point,
        region: Rectangle,
        bounds: Rectangle,
        palette: &Extended,
    ) -> Option<AxisLabel> {
        if !self.crosshair {
            return None;
        }

        match self.basis {
            Basis::Tick(interval) => {
                let Some(interval_keys) = &self.interval_keys else {
                    return None;
                };

                let (crosshair_pos, _, cell_index) = self.calc_crosshair_pos(cursor_pos, region);

                let chart_x_min = region.x;
                let chart_x_max = region.x + region.width;

                let snapped_position = (crosshair_pos / self.cell_width).round() * self.cell_width;
                let snap_ratio = (snapped_position - chart_x_min) / (chart_x_max - chart_x_min);
                let snap_x = snap_ratio * bounds.width;

                if snap_x.is_nan() || snap_x < 0.0 || snap_x > bounds.width {
                    return None;
                }

                let last_index = interval_keys.len() - 1;
                let offset = i64::from(-cell_index) as usize;
                if offset > last_index {
                    return None;
                }

                let array_index = last_index - offset;

                if let Some(timestamp) = interval_keys.get(array_index) {
                    let text_content = self
                        .timezone
                        .format_crosshair_timestamp(*timestamp as i64, interval);

                    return Some(AxisLabelsX::create_label(
                        snap_x,
                        text_content,
                        bounds,
                        true,
                        palette,
                    ));
                }
            }
            Basis::Time(timeframe) => {
                let (_, crosshair_ratio, _) = self.calc_crosshair_pos(cursor_pos, region);

                let x_min = self.x_to_interval(region.x);
                let x_max = self.x_to_interval(region.x + region.width);

                let crosshair_millis =
                    x_min as f64 + f64::from(crosshair_ratio) * (x_max as f64 - x_min as f64);

                let interval = timeframe.to_milliseconds();

                let crosshair_time = DateTime::from_timestamp_millis(crosshair_millis as i64)?;
                let rounded_timestamp =
                    (crosshair_time.timestamp_millis() as f64 / (interval as f64)).round() as u64
                        * interval;

                let snap_ratio =
                    (rounded_timestamp as f64 - x_min as f64) / (x_max as f64 - x_min as f64);

                let snap_x = snap_ratio * f64::from(bounds.width);
                if snap_x.is_nan() || snap_x < 0.0 || snap_x > f64::from(bounds.width) {
                    return None;
                }

                let text_content = self
                    .timezone
                    .format_crosshair_timestamp(rounded_timestamp as i64, interval);

                return Some(AxisLabelsX::create_label(
                    snap_x as f32,
                    text_content,
                    bounds,
                    true,
                    palette,
                ));
            }
        }

        None
    }

    fn visible_region(&self, size: Size) -> Rectangle {
        let width = size.width / self.scaling;
        let height = size.height / self.scaling;

        Rectangle {
            x: -self.translation_x - width / 2.0,
            y: 0.0,
            width,
            height,
        }
    }

    fn x_to_interval(&self, x: f32) -> u64 {
        match self.basis {
            Basis::Time(timeframe) => {
                let interval = timeframe.to_milliseconds() as f64;

                if x <= 0.0 {
                    let diff = ((-x / self.cell_width) as f64 * interval) as u64;
                    self.max.saturating_sub(diff)
                } else {
                    let diff = ((x / self.cell_width) as f64 * interval) as u64;
                    self.max.saturating_add(diff)
                }
            }
            Basis::Tick(_) => {
                let tick = -(x / self.cell_width);
                tick.round() as u64
            }
        }
    }
}

impl canvas::Program<Message> for AxisLabelsX<'_> {
    type State = Interaction;

    fn update(
        &self,
        interaction: &mut Interaction,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
            *interaction = Interaction::None;
        }

        let cursor_position = cursor.position_in(bounds)?;

        if let Event::Mouse(mouse_event) = event {
            match mouse_event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    *interaction = Interaction::Zoomin {
                        last_position: cursor_position,
                    };
                }
                mouse::Event::CursorMoved { .. } => {
                    if let Interaction::Zoomin {
                        ref mut last_position,
                    } = *interaction
                    {
                        let difference_x = last_position.x - cursor_position.x;

                        if difference_x.abs() > 1.0 {
                            *last_position = cursor_position;

                            let message = Message::XScaling(difference_x * 0.2, 0.0, false);

                            return Some(canvas::Action::publish(message).and_capture());
                        }
                    }
                }
                mouse::Event::WheelScrolled { delta } => match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        let message = Message::XScaling(
                            *y,
                            {
                                if let Some(cursor_to_center) =
                                    cursor.position_from(bounds.center())
                                {
                                    cursor_to_center.x
                                } else {
                                    0.0
                                }
                            },
                            true,
                        );

                        return Some(canvas::Action::publish(message).and_capture());
                    }
                },
                _ => {}
            }
        }

        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let palette = theme.extended_palette();

        let labels = self.labels_cache.draw(renderer, bounds.size(), |frame| {
            let region = self.visible_region(frame.size());

            let x_labels_can_fit = (bounds.width / (TEXT_SIZE * 16.0)) as i32;
            let mut all_labels: Vec<AxisLabel> = Vec::with_capacity(x_labels_can_fit as usize + 1);

            let x_min = self.x_to_interval(region.x);
            let x_max = self.x_to_interval(region.x + region.width);

            match self.basis {
                Basis::Tick(_) => {
                    all_labels.extend(self.generate_tick_labels(
                        region,
                        bounds,
                        palette,
                        x_labels_can_fit,
                    ));
                }
                Basis::Time(_) => {
                    all_labels.extend(self.generate_time_labels(
                        bounds,
                        x_min,
                        x_max,
                        palette,
                        x_labels_can_fit,
                    ));
                }
            }

            if let Some(cursor_pos) = cursor.position_in(self.chart_bounds) {
                if let Some(label) = self.generate_crosshair(cursor_pos, region, bounds, palette) {
                    all_labels.push(label);
                }
            }

            AxisLabel::filter_and_draw(&all_labels, frame);
        });

        vec![labels]
    }

    fn mouse_interaction(
        &self,
        interaction: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match interaction {
            Interaction::Panning { .. } => mouse::Interaction::None,
            Interaction::Zoomin { .. } => mouse::Interaction::ResizingHorizontally,
            Interaction::None if cursor.is_over(bounds) => mouse::Interaction::ResizingHorizontally,
            _ => mouse::Interaction::default(),
        }
    }
}

// Y-AXIS LABELS
pub struct AxisLabelsY<'a> {
    pub labels_cache: &'a Cache,
    pub crosshair: bool,
    pub translation_y: f32,
    pub scaling: f32,
    pub min: f32,
    pub last_price: Option<PriceInfoLabel>,
    pub tick_size: f32,
    pub decimals: usize,
    pub cell_height: f32,
    pub basis: Basis,
    pub chart_bounds: Rectangle,
}

impl AxisLabelsY<'_> {
    fn visible_region(&self, size: Size) -> Rectangle {
        let width = size.width / self.scaling;
        let height = size.height / self.scaling;

        Rectangle {
            x: 0.0,
            y: -self.translation_y - height / 2.0,
            width,
            height,
        }
    }

    fn y_to_price(&self, y: f32) -> f32 {
        self.min - (y / self.cell_height) * self.tick_size
    }
}

impl canvas::Program<Message> for AxisLabelsY<'_> {
    type State = Interaction;

    fn update(
        &self,
        interaction: &mut Interaction,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
            *interaction = Interaction::None;
        }

        let cursor_position = cursor.position_in(bounds)?;

        if let Event::Mouse(mouse_event) = event {
            match mouse_event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    *interaction = Interaction::Zoomin {
                        last_position: cursor_position,
                    };
                }
                mouse::Event::CursorMoved { .. } => {
                    if let Interaction::Zoomin {
                        ref mut last_position,
                    } = *interaction
                    {
                        let difference_y = last_position.y - cursor_position.y;

                        if difference_y.abs() > 1.0 {
                            *last_position = cursor_position;

                            let message = Message::YScaling(difference_y * 0.4, 0.0, false);

                            return Some(canvas::Action::publish(message).and_capture());
                        }
                    }
                }
                mouse::Event::WheelScrolled { delta } => match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        let message = Message::YScaling(
                            *y,
                            {
                                if let Some(cursor_to_center) =
                                    cursor.position_from(bounds.center())
                                {
                                    cursor_to_center.y
                                } else {
                                    0.0
                                }
                            },
                            true,
                        );

                        return Some(canvas::Action::publish(message).and_capture());
                    }
                },
                _ => {}
            }
        }

        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let text_size = 12.0;
        let palette = theme.extended_palette();

        let labels = self.labels_cache.draw(renderer, bounds.size(), |frame| {
            let region = self.visible_region(frame.size());

            let highest = self.y_to_price(region.y);
            let lowest = self.y_to_price(region.y + region.height);

            let range = highest - lowest;

            let mut all_labels = linear::generate_labels(
                bounds,
                lowest,
                highest,
                text_size,
                palette.background.base.text,
                Some(self.decimals),
            );

            // Last price (priority 2)
            if let Some(label) = self.last_price {
                let candle_close_label = match self.basis {
                    Basis::Time(timeframe) => {
                        let interval = timeframe.to_milliseconds();

                        let current_time = chrono::Utc::now().timestamp_millis() as u64;
                        let next_kline_open = (current_time / interval + 1) * interval;

                        let remaining_seconds = (next_kline_open - current_time) / 1000;

                        if remaining_seconds > 0 {
                            let hours = remaining_seconds / 3600;
                            let minutes = (remaining_seconds % 3600) / 60;
                            let seconds = remaining_seconds % 60;

                            let time_format = if hours > 0 {
                                format!("{hours:02}:{minutes:02}:{seconds:02}")
                            } else {
                                format!("{minutes:02}:{seconds:02}")
                            };

                            Some(Label {
                                content: time_format,
                                background_color: Some(palette.background.strong.color),
                                text_color: if palette.is_dark {
                                    Color::BLACK.scale_alpha(0.8)
                                } else {
                                    Color::WHITE.scale_alpha(0.8)
                                },
                                text_size: 11.0,
                            })
                        } else {
                            None
                        }
                    }
                    Basis::Tick(_) => None,
                };

                let (price, color) = label.get_with_color(palette);

                let price_label = Label {
                    content: format!("{:.*}", self.decimals, price),
                    background_color: Some(color),
                    text_color: {
                        if candle_close_label.is_some() {
                            if palette.is_dark {
                                Color::BLACK
                            } else {
                                Color::WHITE
                            }
                        } else {
                            palette.primary.strong.text
                        }
                    },
                    text_size: 12.0,
                };

                let y_pos = bounds.height - ((price - lowest) / range * bounds.height);
                let content_amt = if candle_close_label.is_some() { 2 } else { 1 };

                all_labels.push(AxisLabel::Y(
                    calc_label_rect(y_pos, content_amt, text_size, bounds),
                    price_label,
                    candle_close_label,
                ));
            }

            // Crosshair price (priority 3)
            if self.crosshair {
                if let Some(crosshair_pos) = cursor.position_in(self.chart_bounds) {
                    let rounded_price = round_to_tick(
                        lowest + (range * (bounds.height - crosshair_pos.y) / bounds.height),
                        self.tick_size,
                    );
                    let y_position =
                        bounds.height - ((rounded_price - lowest) / range * bounds.height);

                    let label = Label {
                        content: format!("{:.*}", self.decimals, rounded_price),
                        background_color: Some(palette.secondary.base.color),
                        text_color: palette.secondary.base.text,
                        text_size: 12.0,
                    };

                    all_labels.push(AxisLabel::Y(
                        calc_label_rect(y_position, 1, text_size, bounds),
                        label,
                        None,
                    ));
                }
            }

            AxisLabel::filter_and_draw(&all_labels, frame);
        });

        vec![labels]
    }

    fn mouse_interaction(
        &self,
        interaction: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match interaction {
            Interaction::Zoomin { .. } => mouse::Interaction::ResizingVertically,
            Interaction::Panning { .. } => mouse::Interaction::None,
            Interaction::None if cursor.is_over(bounds) => mouse::Interaction::ResizingVertically,
            _ => mouse::Interaction::default(),
        }
    }
}

// other helpers
#[derive(Debug, Clone, Copy)]
pub enum PriceInfoLabel {
    Up(f32),
    Down(f32),
    Neutral(f32),
}

impl PriceInfoLabel {
    pub fn new(close_price: f32, open_price: f32) -> Self {
        if close_price >= open_price {
            PriceInfoLabel::Up(close_price)
        } else {
            PriceInfoLabel::Down(close_price)
        }
    }

    pub fn get_with_color(self, palette: &Extended) -> (f32, iced::Color) {
        match self {
            PriceInfoLabel::Up(p) => (p, palette.success.base.color),
            PriceInfoLabel::Down(p) => (p, palette.danger.base.color),
            PriceInfoLabel::Neutral(p) => (p, palette.secondary.strong.color),
        }
    }
}
