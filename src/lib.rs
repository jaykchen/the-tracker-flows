pub mod db_updater;
pub mod issue_tracker;
use anyhow::anyhow;
use chrono::{Datelike, Duration, NaiveDate, Timelike, Utc};
use dotenv::dotenv;
use flowsnet_platform_sdk::logger;
use http_req::{
    request::{Method, Request},
    response::Response,
    uri::Uri,
};
use issue_tracker::*;
use db_updater::*;
use schedule_flows::{schedule_cron_job, schedule_handler};
use serde::{Deserialize, Serialize};
use std::env;

#[no_mangle]
#[tokio::main(flavor = "current_thread")]
pub async fn on_deploy() {
    let now = Utc::now();
    let now_minute = now.minute() + 2;
    let cron_time = format!("{:02} {:02} {:02} * *", now_minute, now.hour(), now.day());
    schedule_cron_job(cron_time, String::from("cron_job_evoked")).await;
}

#[schedule_handler]
async fn handler(body: Vec<u8>) {
    dotenv().ok();
    let _ = inner(body).await;
}

pub async fn inner(body: Vec<u8>) -> anyhow::Result<()> {
    // let query = "repo:SarthakKeshari/calc_for_everything is:pr is:merged label:hacktoberfest-accepted created:2023-10-01..2023-10-03 review:approved -label:spam -label:invalid";
    // let query = "label:hacktoberfest is:issue is:open no:assignee created:2023-10-01..2023-10-03 -label:spam -label:invalid";

db_updater::test_add_project().await;
db_updater::test_project_exists().await;

    // let issues = search_issues_open(&query).await?;
    // let query = "repo:SarthakKeshari/calc_for_everything is:pr is:merged label:hacktoberfest-accepted created:2023-10-01..2023-10-30 review:approved -label:spam -label:invalid";
    // let pulls = get_per_repo_pull_requests(&query).await?;


    // let mut count = 0;
    // for iss in pulls {
    //     count += 1;
    //     log::error!("pull: {:?}", iss);
    //     let content = format!("{:?}", iss);
    //     // let _ = upload_to_gist(&content).await?;
    //     if count > 5 {
    //         break;
    //     }
    // }

    Ok(())
}

pub async fn search_issue_init() -> anyhow::Result<()> {
    let start_date = "2023-10-01";
    let issue_label = "hacktoberfest";
    let pr_label = "hacktoberfest-accepted";
    let n_days = 2;
    let is_issue = true;
    let is_start = true;
    let query_vec = inner_query_by_date_range(
        start_date,
        n_days,
        issue_label,
        pr_label,
        is_issue,
        is_start,
    );

    let mut texts = String::new();
    for query in query_vec {
        //     let query =
        //         format!("label:hacktoberfest-accepted is:pr is:merged created:{date_range} review:approved -label:spam -label:invalid");
        //     let query ="label:hacktoberfest is:issue is:open no:assignee created:{date_range} review:approved -label:spam -label:invalid");
        //     let label_to_watch = "hacktoberfest";
        let pulls = search_issues_open(&query).await?;

        for pull in pulls {
            log::info!("pull: {:?}", pull.url);
            texts.push_str(&format!("{}\n", pull.url));
            break;
        }
    }

    // let _ = upload_to_gist(&texts).await?;
    Ok(())
}
