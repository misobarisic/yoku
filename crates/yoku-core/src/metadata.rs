use chrono::{Datelike, Days, Months, NaiveDate};
use pulldown_cmark::{Event, Parser, Tag};
use std::fmt;
use std::ops::Range;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    High,
    #[default]
    Normal,
    Low,
}

impl Priority {
    pub fn label(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Normal => "normal",
            Self::Low => "low",
        }
    }
}

impl FromStr for Priority {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "high" => Ok(Self::High),
            "normal" => Ok(Self::Normal),
            "low" => Ok(Self::Low),
            _ => Err("Priority must be high, normal, or low".into()),
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

pub fn parse_date(value: &str) -> Result<NaiveDate, String> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| "Use a valid date in YYYY-MM-DD format".to_owned())?;
    if date.format("%Y-%m-%d").to_string() != value || value.len() != 10 {
        return Err("Use a valid date in YYYY-MM-DD format".into());
    }
    Ok(date)
}

pub fn valid_tag(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '/'))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RepeatUnit {
    Day,
    Week,
    Month,
    Year,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recurrence {
    every: u32,
    unit: RepeatUnit,
}

impl FromStr for Recurrence {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (every, unit) = match value {
            "daily" => (1, RepeatUnit::Day),
            "weekly" => (1, RepeatUnit::Week),
            "monthly" => (1, RepeatUnit::Month),
            "yearly" => (1, RepeatUnit::Year),
            _ => {
                let split = value
                    .len()
                    .checked_sub(1)
                    .ok_or("Use daily, weekly, monthly, yearly, or an interval such as 2w")?;
                let every = value
                    .get(..split)
                    .and_then(|number| number.parse::<u32>().ok())
                    .filter(|number| *number > 0)
                    .ok_or("Repeat intervals must be positive, such as 2w")?;
                let unit = match value.get(split..) {
                    Some("d") => RepeatUnit::Day,
                    Some("w") => RepeatUnit::Week,
                    Some("m") => RepeatUnit::Month,
                    Some("y") => RepeatUnit::Year,
                    _ => return Err("Use d, w, m, or y for a repeat interval".into()),
                };
                (every, unit)
            }
        };
        Ok(Self { every, unit })
    }
}

impl fmt::Display for Recurrence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = match self.unit {
            RepeatUnit::Day => "d",
            RepeatUnit::Week => "w",
            RepeatUnit::Month => "m",
            RepeatUnit::Year => "y",
        };
        write!(f, "{}{unit}", self.every)
    }
}

impl Recurrence {
    /// Advance from the scheduled date, skipping missed occurrences after today.
    pub fn next_due(self, due: NaiveDate, today: NaiveDate) -> Option<NaiveDate> {
        let next = match self.unit {
            RepeatUnit::Day | RepeatUnit::Week => {
                let days = u64::from(self.every).checked_mul(if self.unit == RepeatUnit::Week {
                    7
                } else {
                    1
                })?;
                let elapsed = today.signed_duration_since(due).num_days().max(0) as u64;
                let steps = elapsed / days + 1;
                due.checked_add_days(Days::new(days.checked_mul(steps)?))?
            }
            RepeatUnit::Month | RepeatUnit::Year => {
                let months =
                    self.every
                        .checked_mul(if self.unit == RepeatUnit::Year { 12 } else { 1 })?;
                let elapsed = ((today.year() - due.year()) * 12 + today.month() as i32
                    - due.month() as i32)
                    .max(0) as u32;
                let steps = (elapsed / months).max(1);
                let candidate = due.checked_add_months(Months::new(months.checked_mul(steps)?))?;
                if candidate > today {
                    candidate
                } else {
                    due.checked_add_months(Months::new(months.checked_mul(steps.checked_add(1)?)?))?
                }
            }
        };
        // Portable dates use four-digit non-negative years.
        (0..=9999).contains(&next.year()).then_some(next)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskMetadata {
    pub tags: Vec<String>,
    pub priority: Priority,
    pub due: Option<NaiveDate>,
    pub recurrence: Option<Recurrence>,
}

fn token_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (index, character) in text.char_indices() {
        if character.is_whitespace() {
            if let Some(start) = start.take() {
                ranges.push(start..index);
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(start) = start {
        ranges.push(start..text.len());
    }
    ranges
}

fn code_ranges(text: &str) -> Vec<Range<usize>> {
    Parser::new(text)
        .into_offset_iter()
        .filter_map(|(event, range)| {
            matches!(event, Event::Code(_) | Event::Start(Tag::CodeBlock(_))).then_some(range)
        })
        .collect()
}

impl TaskMetadata {
    pub fn parse(text: &str) -> Self {
        let mut metadata = Self::default();
        let code = code_ranges(text);
        for range in token_ranges(text) {
            let token = &text[range.clone()];
            if !code
                .iter()
                .any(|code| code.start < range.end && range.start < code.end)
            {
                if let Some(tag) = token.strip_prefix('#').filter(|tag| valid_tag(tag)) {
                    if !metadata
                        .tags
                        .iter()
                        .any(|existing| existing.to_lowercase() == tag.to_lowercase())
                    {
                        metadata.tags.push(tag.into());
                    }
                } else if let Some(date) = token
                    .strip_prefix("due:")
                    .and_then(|value| parse_date(value).ok())
                {
                    metadata.due = Some(date);
                } else if let Some(priority) = token
                    .strip_prefix("priority:")
                    .and_then(|value| value.parse().ok())
                {
                    metadata.priority = priority;
                } else if let Some(recurrence) = token
                    .strip_prefix("repeat:")
                    .and_then(|value| value.parse().ok())
                {
                    metadata.recurrence = Some(recurrence);
                }
            }
        }
        metadata
    }
}

/// Replace metadata tokens while retaining whitespace and unrelated task text.
pub fn set_field(text: &str, field: &str, value: &str) -> String {
    let prefix = format!("{field}:");
    let mut result = String::new();
    let mut end = 0;
    let mut replaced = false;
    let code = code_ranges(text);
    for range in token_ranges(text) {
        let token = &text[range.clone()];
        if !code
            .iter()
            .any(|code| code.start < range.end && range.start < code.end)
            && token.starts_with(&prefix)
        {
            result.push_str(&text[end..range.start]);
            result.push_str(&prefix);
            result.push_str(value);
            end = range.end;
            replaced = true;
        }
    }
    if replaced {
        result.push_str(&text[end..]);
    } else {
        result.push_str(text);
        if !text.ends_with(char::is_whitespace) {
            result.push(' ');
        }
        result.push_str(&prefix);
        result.push_str(value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_portable_tokens_and_leaves_invalid_or_code_tokens_as_text() {
        let metadata = TaskMetadata::parse("Fix bug #Work #work due:2026-10-03 priority:high repeat:2w `example #fake due:2027-01-01`");
        assert_eq!(metadata.tags, ["Work"]);
        assert_eq!(metadata.due, Some(parse_date("2026-10-03").unwrap()));
        assert_eq!(metadata.priority, Priority::High);
        assert_eq!(metadata.recurrence, Some("2w".parse().unwrap()));
        assert!(
            TaskMetadata::parse("due:2026-02-30 repeat:0d priority:unknown")
                .due
                .is_none()
        );
        assert!(parse_date("2026-1-1").is_err());
    }

    #[test]
    fn advancing_repeats_handles_overdue_tasks_and_calendar_boundaries() {
        let today = parse_date("2026-10-03").unwrap();
        assert_eq!(
            "weekly"
                .parse::<Recurrence>()
                .unwrap()
                .next_due(parse_date("2026-09-01").unwrap(), today),
            Some(parse_date("2026-10-06").unwrap())
        );
        assert_eq!(
            "monthly".parse::<Recurrence>().unwrap().next_due(
                parse_date("2026-01-31").unwrap(),
                parse_date("2026-01-31").unwrap()
            ),
            Some(parse_date("2026-02-28").unwrap())
        );
        assert_eq!(
            "yearly".parse::<Recurrence>().unwrap().next_due(
                parse_date("2024-02-29").unwrap(),
                parse_date("2024-02-29").unwrap()
            ),
            Some(parse_date("2025-02-28").unwrap())
        );
        assert!("4294967295y"
            .parse::<Recurrence>()
            .unwrap()
            .next_due(today, today)
            .is_none());
    }

    #[test]
    fn updating_fields_preserves_spacing_and_inline_examples() {
        let source = "task  due:2026-01-01\t#work `example due:2026-02-02`";
        assert_eq!(
            set_field(source, "due", "2026-10-04"),
            source.replace("due:2026-01-01", "due:2026-10-04")
        );
    }
}
