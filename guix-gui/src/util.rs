use std::time::{Duration, SystemTime};

/// Future timestamps (clock skew) collapse to "Just now".
#[must_use]
pub fn humanize_age(t: SystemTime) -> String {
    let now = SystemTime::now();
    let delta = now.duration_since(t).unwrap_or(Duration::ZERO);
    humanize_duration(delta)
}

#[must_use]
pub fn humanize_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        return "Just now".into();
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins} minute{} ago", plural(mins));
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours} hour{} ago", plural(hours));
    }
    let days = hours / 24;
    if days < 30 {
        return format!("{days} day{} ago", plural(days));
    }
    let months = days / 30;
    if months < 12 {
        return format!("{months} month{} ago", plural(months));
    }
    "over a year ago".into()
}

fn plural(n: u64) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

#[must_use]
pub fn short_hash(commit: &str) -> &str {
    commit.get(..commit.len().min(7)).unwrap_or(commit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanize_just_now() {
        assert_eq!(humanize_duration(Duration::from_secs(0)), "Just now");
        assert_eq!(humanize_duration(Duration::from_secs(59)), "Just now");
    }

    #[test]
    fn humanize_minutes() {
        assert_eq!(humanize_duration(Duration::from_secs(60)), "1 minute ago");
        assert_eq!(humanize_duration(Duration::from_secs(120)), "2 minutes ago");
        assert_eq!(
            humanize_duration(Duration::from_secs(59 * 60)),
            "59 minutes ago"
        );
    }

    #[test]
    fn humanize_hours() {
        assert_eq!(
            humanize_duration(Duration::from_secs(60 * 60)),
            "1 hour ago"
        );
        assert_eq!(
            humanize_duration(Duration::from_secs(3 * 60 * 60)),
            "3 hours ago"
        );
        assert_eq!(
            humanize_duration(Duration::from_secs(23 * 60 * 60)),
            "23 hours ago"
        );
    }

    #[test]
    fn humanize_days() {
        assert_eq!(
            humanize_duration(Duration::from_secs(24 * 60 * 60)),
            "1 day ago"
        );
        assert_eq!(
            humanize_duration(Duration::from_secs(3 * 86_400)),
            "3 days ago"
        );
        assert_eq!(
            humanize_duration(Duration::from_secs(29 * 86_400)),
            "29 days ago"
        );
    }

    #[test]
    fn humanize_months() {
        assert_eq!(
            humanize_duration(Duration::from_secs(30 * 86_400)),
            "1 month ago"
        );
        assert_eq!(
            humanize_duration(Duration::from_secs(6 * 30 * 86_400)),
            "6 months ago"
        );
        assert_eq!(
            humanize_duration(Duration::from_secs(11 * 30 * 86_400)),
            "11 months ago"
        );
    }

    #[test]
    fn humanize_over_a_year() {
        assert_eq!(
            humanize_duration(Duration::from_secs(12 * 30 * 86_400)),
            "over a year ago"
        );
        assert_eq!(
            humanize_duration(Duration::from_secs(5 * 365 * 86_400)),
            "over a year ago"
        );
    }

    #[test]
    fn humanize_age_handles_future_timestamps() {
        let future = SystemTime::now() + Duration::from_secs(3600);
        assert_eq!(humanize_age(future), "Just now");
    }

    #[test]
    fn short_hash_truncates() {
        assert_eq!(short_hash("abcdef1234567890"), "abcdef1");
        assert_eq!(short_hash("abc"), "abc");
        assert_eq!(short_hash(""), "");
    }
}
