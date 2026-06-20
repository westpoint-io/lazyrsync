use ratatui::style::Color;

const AGE_PERIODS: [(&str, i64); 7] = [
    ("s", 1),
    ("m", 60),
    ("h", 3600),
    ("d", 86400),
    ("w", 604800),
    ("M", 31536000 / 12),
    ("y", 31536000),
];

fn time_ago(secs: i64) -> String {
    if secs < AGE_PERIODS[0].1 {
        return "now".to_string();
    }
    for w in AGE_PERIODS.windows(2) {
        if secs < w[1].1 {
            return format!("{}{}", secs / w[0].1, w[0].0);
        }
    }
    let (label, unit) = AGE_PERIODS[AGE_PERIODS.len() - 1];
    format!("{}{}", secs / unit, label)
}

pub(crate) fn task_age(created: Option<i64>) -> (String, Color) {
    let Some(ts) = created else {
        return ("-".to_string(), super::muted());
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let secs = (now - ts).max(0);
    let color = if secs < 604_800 {
        super::added()
    } else if secs < 2_628_000 {
        super::secondary()
    } else {
        super::muted()
    };
    (time_ago(secs), color)
}

#[cfg(test)]
mod tests {
    use super::time_ago;

    #[test]
    fn time_ago_picks_single_largest_unit() {
        assert_eq!(time_ago(0), "now");
        assert_eq!(time_ago(45), "45s");
        assert_eq!(time_ago(90), "1m");
        assert_eq!(time_ago(3 * 3600), "3h");
        assert_eq!(time_ago(4 * 86400), "4d");
        assert_eq!(time_ago(70 * 86400), "2M");
        assert_eq!(time_ago(5 * 31536000), "5y");
    }
}
