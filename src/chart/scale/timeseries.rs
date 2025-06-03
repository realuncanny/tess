use super::AxisLabel;
use chrono::{DateTime, Datelike, Duration, Months};
use data::{
    UserTimezone,
    util::{reset_to_start_of_month_utc, reset_to_start_of_year_utc},
};
use iced::theme::palette::Extended;

pub const ONE_DAY_MS: u64 = 24 * 60 * 60 * 1000;

const TEXT_SIZE: f32 = 12.0;

const M1_TIME_STEPS: [u64; 9] = [
    1000 * 60 * 720, // 12 hour
    1000 * 60 * 180, // 3 hour
    1000 * 60 * 60,  // 1 hour
    1000 * 60 * 30,  // 30 min
    1000 * 60 * 15,  // 15 min
    1000 * 60 * 10,  // 10 min
    1000 * 60 * 5,   // 5 min
    1000 * 60 * 2,   // 2 min
    1000 * 60,       // 1 min
];

const M3_TIME_STEPS: [u64; 9] = [
    1000 * 60 * 1440, // 24 hour
    1000 * 60 * 720,  // 12 hour
    1000 * 60 * 360,  // 6 hour
    1000 * 60 * 120,  // 2 hour
    1000 * 60 * 60,   // 1 hour
    1000 * 60 * 30,   // 30 min
    1000 * 60 * 15,   // 15 min
    1000 * 60 * 9,    // 9 min
    1000 * 60 * 3,    // 3 min
];

const M5_TIME_STEPS: [u64; 9] = [
    1000 * 60 * 1440, // 24 hour
    1000 * 60 * 720,  // 12 hour
    1000 * 60 * 480,  // 8 hour
    1000 * 60 * 240,  // 4 hour
    1000 * 60 * 120,  // 2 hour
    1000 * 60 * 60,   // 1 hour
    1000 * 60 * 30,   // 30 min
    1000 * 60 * 15,   // 15 min
    1000 * 60 * 5,    // 5 min
];

const HOURLY_TIME_STEPS: [u64; 8] = [
    1000 * 60 * 5760, // 96 hour
    1000 * 60 * 2880, // 48 hour
    1000 * 60 * 1440, // 24 hour
    1000 * 60 * 720,  // 12 hour
    1000 * 60 * 480,  // 8 hour
    1000 * 60 * 240,  // 4 hour
    1000 * 60 * 120,  // 2 hour
    1000 * 60 * 60,   // 1 hour
];

const MS_TIME_STEPS: [u64; 10] = [
    1000 * 120,
    1000 * 60,
    1000 * 30,
    1000 * 10,
    1000 * 5,
    1000 * 2,
    1000,
    500,
    200,
    100,
];

fn calc_time_step(
    earliest: u64,
    latest: u64,
    labels_can_fit: i32,
    timeframe: exchange::Timeframe,
) -> (u64, u64) {
    let timeframe_in_min = timeframe.to_milliseconds() / 60_000;

    let time_steps: &[u64] = match timeframe_in_min {
        0_u64..1_u64 => &MS_TIME_STEPS,
        1..=30 => match timeframe_in_min {
            1 => &M1_TIME_STEPS,
            3 => &M3_TIME_STEPS,
            5 => &M5_TIME_STEPS,
            15 => &M5_TIME_STEPS[..7],
            30 => &M5_TIME_STEPS[..6],
            _ => &HOURLY_TIME_STEPS,
        },
        31.. => &HOURLY_TIME_STEPS,
    };

    let duration = latest - earliest;
    let mut selected_step = time_steps[0];

    for &step in time_steps {
        if duration / step >= (labels_can_fit as u64) {
            selected_step = step;
            break;
        }
        if step <= duration {
            selected_step = step;
        }
    }

    let rounded_earliest = (earliest / selected_step) * selected_step;

    (selected_step, rounded_earliest)
}

pub fn generate_time_labels(
    timeframe: exchange::Timeframe,
    timezone: data::UserTimezone,
    axis_bounds: iced::Rectangle,
    x_min: u64,
    x_max: u64,
    palette: &Extended,
    x_labels_can_fit: i32,
) -> Vec<AxisLabel> {
    let (time_step, initial_rounded_earliest) =
        calc_time_step(x_min, x_max, x_labels_can_fit, timeframe);

    if time_step == 0 {
        return Vec::new();
    }

    let calculate_x_pos = |time_millis: u64, min_millis: u64, max_millis: u64, width: f32| -> f64 {
        if max_millis > min_millis {
            ((time_millis - min_millis) as f64 / (max_millis - min_millis) as f64)
                * f64::from(width)
        } else {
            0.0
        }
    };

    let is_drawable = |x_pos: f64, width: f32| -> bool {
        x_pos >= (-TEXT_SIZE * 5.0).into() && x_pos <= f64::from(width) + (TEXT_SIZE * 5.0) as f64
    };

    let mut all_labels = Vec::with_capacity(x_labels_can_fit as usize * 3);

    if time_step >= ONE_DAY_MS {
        daily_labels_gen(
            timezone,
            &mut all_labels,
            axis_bounds,
            x_min,
            x_max,
            palette,
            time_step,
            calculate_x_pos,
            is_drawable,
        );

        monthly_labels_gen(
            timezone,
            &mut all_labels,
            axis_bounds,
            x_min,
            x_max,
            palette,
            calculate_x_pos,
            is_drawable,
        );

        yearly_labels_gen(
            &mut all_labels,
            axis_bounds,
            x_min,
            x_max,
            palette,
            calculate_x_pos,
            is_drawable,
        );
    } else {
        sub_daily_labels_gen(
            timezone,
            &mut all_labels,
            axis_bounds,
            x_min,
            x_max,
            palette,
            time_step,
            initial_rounded_earliest,
            timeframe,
            calculate_x_pos,
            is_drawable,
        );
    }

    all_labels
}

fn sub_daily_labels_gen(
    timezone: data::UserTimezone,
    all_labels: &mut Vec<AxisLabel>,
    axis_bounds: iced::Rectangle,
    x_min: u64,
    x_max: u64,
    palette: &Extended,
    time_step: u64,
    initial_rounded_earliest: u64,
    timeframe: exchange::Timeframe,
    calculate_x_pos: impl Fn(u64, u64, u64, f32) -> f64,
    is_drawable: impl Fn(f64, f32) -> bool,
) {
    let mut current_time = initial_rounded_earliest;
    while current_time <= x_max {
        if current_time >= x_min {
            let x_position = calculate_x_pos(current_time, x_min, x_max, axis_bounds.width);

            if is_drawable(x_position, axis_bounds.width) {
                let label_text = timezone.format_timestamp((current_time / 1000) as i64, timeframe);
                all_labels.push(AxisLabel::new_x(
                    x_position as f32,
                    label_text,
                    axis_bounds,
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

fn daily_labels_gen(
    timezone: UserTimezone,
    all_labels: &mut Vec<AxisLabel>,
    axis_bounds: iced::Rectangle,
    x_min: u64,
    x_max: u64,
    palette: &Extended,
    time_step: u64,
    calculate_x_pos: impl Fn(u64, u64, u64, f32) -> f64,
    is_drawable: impl Fn(f64, f32) -> bool,
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
                        let dt_in_timezone = match timezone {
                            UserTimezone::Local => {
                                current_day_iter_utc.with_timezone(&chrono::Local)
                            }
                            UserTimezone::Utc => current_day_iter_utc.into(),
                        };

                        let is_jan_1st = dt_in_timezone.month() == 1 && dt_in_timezone.day() == 1;

                        if !is_jan_1st {
                            let x_pos =
                                calculate_x_pos(day_candidate_ts, x_min, x_max, axis_bounds.width);
                            if is_drawable(x_pos, axis_bounds.width) {
                                let day_text = dt_in_timezone.format("%d").to_string();
                                all_labels.push(AxisLabel::new_x(
                                    x_pos as f32,
                                    day_text,
                                    axis_bounds,
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
                        != DateTime::from_timestamp_millis(x_max as i64).map_or(0, |dt| dt.month())
                {
                    break;
                }
            } else {
                break;
            }
        }
    }
}

fn monthly_labels_gen(
    timezone: data::UserTimezone,
    all_labels: &mut Vec<AxisLabel>,
    axis_bounds: iced::Rectangle,
    x_min: u64,
    x_max: u64,
    palette: &Extended,
    calculate_x_pos: impl Fn(u64, u64, u64, f32) -> f64,
    is_drawable: impl Fn(f64, f32) -> bool,
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
                    let dt_in_timezone = match timezone {
                        UserTimezone::Local => current_month_iter_utc.with_timezone(&chrono::Local),
                        UserTimezone::Utc => current_month_iter_utc.into(),
                    };
                    let is_january = dt_in_timezone.month() == 1;

                    if !is_january {
                        let x_position_month =
                            calculate_x_pos(month_ts_millis, x_min, x_max, axis_bounds.width);
                        if is_drawable(x_position_month, axis_bounds.width) {
                            let month_label_text = dt_in_timezone.format("%b").to_string();
                            all_labels.push(AxisLabel::new_x(
                                x_position_month as f32,
                                month_label_text,
                                axis_bounds,
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

fn yearly_labels_gen(
    all_labels: &mut Vec<AxisLabel>,
    axis_bounds: iced::Rectangle,
    x_min: u64,
    x_max: u64,
    palette: &Extended,
    calculate_x_pos: impl Fn(u64, u64, u64, f32) -> f64,
    is_drawable: impl Fn(f64, f32) -> bool,
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
                        calculate_x_pos(year_ts_millis, x_min, x_max, axis_bounds.width);

                    if is_drawable(x_position_year, axis_bounds.width) {
                        let year_label_text = current_year_iter_utc.format("%Y").to_string();
                        all_labels.push(AxisLabel::new_x(
                            x_position_year as f32,
                            year_label_text,
                            axis_bounds,
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
