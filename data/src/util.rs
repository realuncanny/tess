const DECIMAL_PRECISION: usize = 3;

pub fn abbr_large_numbers(value: f32) -> String {
    let abs_value = value.abs();
    let sign = if value < 0.0 { "-" } else { "" };

    match abs_value {
        v if v >= 1_000_000_000.0 => format!("{}{:.3}b", sign, v / 100_000_000.0),
        v if v >= 1_000_000.0 => format!("{}{:.2}m", sign, v / 1_000_000.0),
        v if v >= 1_000.0 => format!("{}{:.1}k", sign, v / 1_000.0),
        v if v >= 100.0 => format!("{}{:.0}", sign, v),
        v if v >= 10.0 => format!("{}{:.1}", sign, v),
        v if v >= 1.0 => format!("{}{:.2}", sign, v),
        _ => {
            let rounded = (abs_value * 10_f32.powi(DECIMAL_PRECISION as i32)).round();
            if rounded == 0.0 {
                "0".to_string()
            } else {
                format!("{}{:.DECIMAL_PRECISION$}", sign, abs_value)
            }
        }
    }
}

pub fn count_decimals(value: f32) -> usize {
    let value_str = value.to_string();
    if let Some(pos) = value_str.find('.') {
        value_str.len() - pos - 1
    } else {
        0
    }
}

pub fn format_with_commas(num: f32) -> String {
    if num == 0.0 {
        return "0".to_string();
    }

    let abs_num = num.abs();
    let decimals = match abs_num {
        n if n >= 1000.0 => 0,
        n if n >= 100.0 => 1,
        n if n >= 10.0 => 2,
        _ => 3,
    };

    let is_negative = num < 0.0;

    if abs_num < 1000.0 {
        return format!(
            "{}{:.*}",
            if is_negative { "-" } else { "" },
            decimals,
            abs_num
        );
    }

    let s = format!("{:.*}", decimals, abs_num);

    let (integer_part, decimal_part) = match s.find('.') {
        Some(pos) => (&s[..pos], Some(&s[pos..])),
        None => (s.as_str(), None),
    };

    let mut result = {
        let num_commas = (integer_part.len() - 1) / 3;
        let decimal_len = decimal_part.map_or(0, str::len);

        String::with_capacity(
            usize::from(is_negative) + integer_part.len() + num_commas + decimal_len,
        )
    };

    if is_negative {
        result.push('-');
    }

    let digits_len = integer_part.len();
    for (i, ch) in integer_part.chars().enumerate() {
        result.push(ch);

        let pos_from_right = digits_len - i - 1;
        if i < digits_len - 1 && pos_from_right % 3 == 0 {
            result.push(',');
        }
    }

    if let Some(decimal) = decimal_part {
        result.push_str(decimal);
    }

    result
}

pub fn round_to_tick(value: f32, tick_size: f32) -> f32 {
    (value / tick_size).round() * tick_size
}

pub fn currency_abbr(price: f32) -> String {
    match price {
        p if p > 1_000_000_000.0 => format!("${:.2}b", p / 1_000_000_000.0),
        p if p > 1_000_000.0 => format!("${:.1}m", p / 1_000_000.0),
        p if p > 1000.0 => format!("${:.2}k", p / 1000.0),
        _ => format!("${:.2}", price),
    }
}

pub fn pct_change(change: f32) -> String {
    match change {
        c if c > 0.0 => format!("+{:.2}%", c),
        _ => format!("{:.2}%", change),
    }
}

pub fn guesstimate_ticks(range: f32) -> f32 {
    match range {
        r if r > 1_000_000.0 => 10.0,
        r if r > 100_000.0 => 1_000.0,
        r if r > 10_000.0 => 100.0,
        r if r > 1_000.0 => 10.0,
        r if r > 100.0 => 1.0,
        r if r > 10.0 => 0.1,
        r if r > 1.0 => 0.01,
        r if r > 0.1 => 0.001,
        r if r > 0.01 => 0.0001,
        _ => 0.00001,
    }
}
