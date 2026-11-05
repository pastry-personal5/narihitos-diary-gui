//! Narrow civil-date inputs; no arbitrary query, path or clock in the WebView.
use super::{AuthenticError, Bridge};
use reqwest::Method;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarAction {
    Month,
    PreviousMonth,
    NextMonth,
    PreviousDay,
    NextDay,
    PreviousWeek,
    NextWeek,
    Today,
}
impl CalendarAction {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Month => "month",
            Self::PreviousMonth => "previous_month",
            Self::NextMonth => "next_month",
            Self::PreviousDay => "previous_day",
            Self::NextDay => "next_day",
            Self::PreviousWeek => "previous_week",
            Self::NextWeek => "next_week",
            Self::Today => "today",
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarDay {
    pub date: String,
    pub day: u8,
    pub weekday: u8,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Calendar {
    pub anchor: String,
    pub today: String,
    pub year: u16,
    pub month: u8,
    pub days: Vec<CalendarDay>,
    pub previous_month: Option<String>,
    pub next_month: Option<String>,
}
impl Bridge {
    pub async fn get_calendar(
        &self,
        anchor: Option<String>,
        action: CalendarAction,
    ) -> Result<Calendar, AuthenticError> {
        if anchor
            .as_ref()
            .is_some_and(|s| !super::authentic::date_shape(s))
        {
            return Err(AuthenticError::new("invalid_input"));
        }
        let mut path = format!("/calendar?action={}", action.as_str());
        if let Some(date) = anchor {
            path.push_str(&format!("&anchor={date}"));
        }
        self.authentic(Method::GET, &path, None).await
    }
}
