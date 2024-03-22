use crate::db_updater::*;
use crate::issue_tracker::*;
use crate::{END_DATE, ISSUE_LABEL, PR_LABEL, START_DATE, TODAY_PLUS_TEN_MINUTES, TODAY_THIS_HOUR};

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike, Utc};
use flowsnet_platform_sdk::logger;

pub fn inner_query_n_days(
    start_date: &str,
    n_days: i64,
    issue_label: &str,
    pr_label: &str,
    is_issue: bool,
    is_start: bool,
) -> String {
    // let start_date ="2023-10-01";
    // let issue_label = "hacktoberfest";
    // let pr_label = "hacktoberfest-accepted";
    let start_date =
        NaiveDate::parse_from_str(start_date, "%Y-%m-%d").expect("Failed to parse date");

    let end_date = (start_date + Duration::days(n_days))
        .format("%Y-%m-%d")
        .to_string();

    let date_range = format!("{}..{}", start_date, end_date);

    let query = if is_issue && is_start {
        format!("label:{issue_label} is:issue is:open no:assignee created:{date_range} -label:spam -label:invalid")
    } else if is_issue && !is_start {
        format!("label:{issue_label} is:issue is:closed created:{date_range} -label:spam -label:invalid")
    } else {
        format!("label:{pr_label} is:pr is:merged created:{date_range} review:approved -label:spam -label:invalid")
    };

    query
}

pub fn inner_query_vec_by_date_range(
    start_date: &str,
    n_days: i64,
    issue_label: &str,
    pr_label: &str,
    is_issue: bool,
    is_start: bool,
) -> Vec<String> {
    // let start_date ="2023-10-01";
    // let issue_label = "hacktoberfest";
    // let pr_label = "hacktoberfest-accepted";
    let start_date =
        NaiveDate::parse_from_str(start_date, "%Y-%m-%d").expect("Failed to parse date");

    let date_point_vec = (0..20)
        .map(|i| {
            (start_date + Duration::days(n_days * i as i64))
                .format("%Y-%m-%d")
                .to_string()
        })
        .collect::<Vec<_>>();

    let date_range_vec = date_point_vec
        .windows(2)
        .map(|x| x.join(".."))
        .collect::<Vec<_>>();

    let mut out = Vec::new();
    for date_range in date_range_vec {
        let query = if is_issue && is_start {
            format!("label:{issue_label} is:issue is:open no:assignee created:{date_range} -label:spam -label:invalid")
        } else if is_issue && !is_start {
            format!("label:{issue_label} is:issue is:closed created:{date_range} -label:spam -label:invalid")
        } else {
            format!("label:{pr_label} is:pr is:merged created:{date_range} review:approved -label:spam -label:invalid")
        };
        out.push(query);
    }

    out
}

pub async fn top_runner() {
    let now = Utc::now().naive_utc();

    if now < *TODAY_PLUS_TEN_MINUTES {
        let _ = run_daily(START_DATE).await;
    }
    let _ = run_hourly(START_DATE).await;
}
pub async fn run_hourly(start_date: &str) {
    // let start_date = "2023-10-01";
    let is_issue = true;
    let is_start = true;
    let query = inner_query_n_days(start_date, 2, ISSUE_LABEL, PR_LABEL, is_issue, is_start);
    // let query ="label:hacktoberfest is:issue is:open created:>=2023-10-01 updated:>=2023-10-03 -label:spam -label:invalid";

    let _ = search_issues_w_update_comments().await;
}

pub async fn run_daily(start_date: &str) {
    // let start_date = "2023-10-01";
    // let is_issue = true;
    // let is_start = true;
    let query_open = inner_query_n_days(start_date, 2, ISSUE_LABEL, PR_LABEL, true, true);
    // let query_open ="label:hacktoberfest is:issue is:open created:2023-10-01..2023-10-02 -label:spam -label:invalid";

    let _ = search_issues_open(&query_open).await;

    // let is_issue = true;
    // let is_start = false;
    // let query_closed =
    //     "label:hacktoberfest is:issue is:closed created:>=2023-10-01 -label:spam -label:invalid";
    let query_closed = inner_query_n_days(start_date, 2, ISSUE_LABEL, PR_LABEL, true, false);
    let _ = search_issues_closed(&query_closed).await;

    // let is_issue = false;
    // let is_start = false;
    // let query_pr_overall ="label:hacktoberfest-accepted is:pr is:merged created:2023-10-01..2023-10-02 review:approved -label:spam -label:invalid";
    let query_pr_overall = inner_query_n_days(start_date, 2, ISSUE_LABEL, PR_LABEL, false, false);

    let _ = overall_search_pull_requests(&query_pr_overall).await;

    // let is_issue = false;
    // let is_start = false;
    // let query_per_repo ="repo:SarthakKeshari/calc_for_everything is:pr is:merged label:hacktoberfest-accepted created:2023-10-01..2023-10-03 review:approved -label:spam -label:invalid";
    let query_per_repo = inner_query_n_days(start_date, 2, ISSUE_LABEL, PR_LABEL, false, false);

    let _ = get_per_repo_pull_requests(&query_per_repo).await;
}
