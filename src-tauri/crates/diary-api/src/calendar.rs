//! Server-clock calendar transport; all navigation is Gregorian civil arithmetic.
use super::*;
use diary_domain::{
    authentic::DiaryDate,
    calendar::{Movement, move_date, weekday},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CalendarQuery {
    anchor: Option<String>,
    action: Option<String>,
}

pub(super) fn valid_query(query: &str) -> bool {
    let mut keys = std::collections::BTreeSet::new();
    query.split('&').all(|part| {
        let Some((key, value)) = part.split_once('=') else {
            return false;
        };
        keys.insert(key)
            && match key {
                "anchor" => DiaryDate::parse(value).is_some(),
                "action" => Movement::parse(value).is_some(),
                _ => false,
            }
    })
}
fn model(anchor: DiaryDate, today: DiaryDate) -> Value {
    let days: Vec<_> = (1..=31)
        .filter_map(|day| DiaryDate::from_ymd(anchor.year(), anchor.month(), day))
        .map(|date| json!({"date":date.to_string(),"day":date.day(),"weekday":weekday(date)}))
        .collect();
    json!({"anchor":anchor.to_string(),"today":today.to_string(),"year":anchor.year(),"month":anchor.month(),"days":days,
        "previous_month":move_date(anchor,today,Movement::PreviousMonth).map(|d|d.to_string()),
        "next_month":move_date(anchor,today,Movement::NextMonth).map(|d|d.to_string())})
}
pub(super) async fn get_calendar(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    Query(q): Query<CalendarQuery>,
) -> Response {
    let session = match imports::authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let st = match imports::store(&s) {
        Ok(st) => st,
        Err(r) => return r,
    };
    let anchor = match q.anchor {
        Some(a) => match DiaryDate::parse(&a) {
            Some(d) => Some(d),
            None => return failure(StatusCode::BAD_REQUEST, "invalid_diary_date"),
        },
        None => None,
    };
    let movement = match q
        .action
        .as_deref()
        .map(Movement::parse)
        .unwrap_or(Some(Movement::Month))
    {
        Some(m) => m,
        None => return failure(StatusCode::BAD_REQUEST, "invalid_input"),
    };
    let permit = match s.source_slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return imports::error(diary_application::imports::ImportError::Limit),
    };
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let today = st.today()?;
        let date = move_date(anchor.unwrap_or(today), today, movement)
            .ok_or(diary_application::imports::ImportError::InvalidDiaryDate)?;
        if imports::cancelled(&s, &session) {
            return Err(diary_application::imports::ImportError::Cancelled);
        }
        Ok(model(date, today))
    })
    .await
    {
        Ok(Ok(m)) => Json(m).into_response(),
        Ok(Err(e)) => imports::error(e),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_and_query_are_bounded() {
        let d = DiaryDate::parse("2024-02-29").unwrap();
        let m = model(d, d);
        assert_eq!(m["days"].as_array().unwrap().len(), 29);
        assert_eq!(m["days"][0]["weekday"], 4);
        assert!(valid_query("anchor=2024-02-29&action=next_month"));
        for q in [
            "anchor=2023-02-29",
            "action=next_month&action=month",
            "anchor=../../etc",
            "action=1000",
            "timezone=UTC",
        ] {
            assert!(!valid_query(q));
        }
        let d = DiaryDate::parse("0001-01-01").unwrap();
        assert!(model(d, d)["previous_month"].is_null());
    }
}
