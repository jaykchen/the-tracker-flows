pub mod db_updater;
pub mod issues_tracker;
use chrono::{Datelike, NaiveDate, Timelike, Utc};
use dotenv::dotenv;
use flowsnet_platform_sdk::logger;
use github_flows::{get_octo, GithubLogin};
use octocrab_wasi::{
    models::{issues::Issue, pulls},
    params::{issues::Sort, Direction},
};
use openai_flows::{
    chat::{ChatModel, ChatOptions},
    OpenAIFlows,
};
use schedule_flows::{schedule_cron_job, schedule_handler};
use slack_flows::send_message_to_channel;

use chrono::Duration;
pub use db_updater::*;
pub use issues_tracker::*;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;

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
    let _   = inner(body).await;
}

pub async fn inner(body: Vec<u8>) -> anyhow::Result<()> {
    let query = "repo:SarthakKeshari/calc_for_everything is:pr is:merged label:hacktoberfest-accepted created:2023-10-01..2023-10-03 review:approved -label:spam -label:invalid";

    let octocrab = get_octo(&GithubLogin::Default);

    // let pulls = octocrab
    //     .search()
    //     .issues_and_pull_requests(&query)
    //     .send()
    //     .await?;
    let pulls = octocrab
        .repos("SarthakKeshari", "calc_for_everything")
        .list_stargazers()
        .send()
        .await?;

    for pull in pulls.items {
        log::error!("pulls: {:?}", pull);
        let content = format!("{:?}", pull);
        let _ = upload_to_gist(&content).await?;
    }

    let _=   send_message_to_channel("ik8", "general", "text".to_string()).await;

    // let pulls = get_per_repo_pull_requests(&query).await?;
    // for pull in pulls {
    //     log::info!("pulls: {:?}", pull.url);
    // }

    Ok(())
}
/* pub async fn inner(body: Vec<u8>) -> anyhow::Result<()> {
    dotenv().ok();
    logger::init();

    // let _ = search_for_initial_hits().await;
    let pool = PgPool::connect(&env::var("DATABASE_URL")?).await?;

    let issue_id = "https://github.com/jaykchen/issue-labeler/issues/24";

    let res = list_comments(&pool, issue_id).await?;
    log::info!("Comments: {:?}", res);

    Ok(())
} */

/* async fn run_db() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&env::var("DATABASE_URL")?).await?;

    let _ = add_project_test_1(&pool).await?;
    //
    // let project_id = "jaykchen/issue-labeler";

    // let res = list_projects(&pool).await?;
    // println!("Projects: {:?}", res);
    // let res = list_issues(&pool, project_id).await?;
    let _ = add_issue_test_1(&pool).await?;
    let _ = add_comment_test_1(&pool).await?;

    let issue_id = "https://github.com/jaykchen/issue-labeler/issues/24";

    let res = list_comments(&pool, issue_id).await?;
    println!("Comments: {:?}", res);
    Ok(())
} */

pub async fn search_issue_init() -> anyhow::Result<()> {
    let start_date =
        NaiveDate::parse_from_str("2023-10-01", "%Y-%m-%d").expect("Failed to parse date");

    let mut date_point_vec = Vec::new();

    for i in 0..20 {
        let three_days_str = (start_date + Duration::days(2 * i as i64))
            .format("%Y-%m-%d")
            .to_string();

        date_point_vec.push(three_days_str);
    }

    let mut date_range_vec = date_point_vec
        .windows(2)
        .map(|x| x.join(".."))
        .collect::<Vec<_>>();

    let mut texts = String::new();
    for date_range in date_range_vec {
        let query =
            format!("label:hacktoberfest-accepted is:pr is:merged created:{date_range} review:approved -label:spam -label:invalid");
        let label_to_watch = "hacktoberfest";
        let pulls = get_pull_requests(&query, label_to_watch).await?;

        for pull in pulls {
            log::info!("pull: {:?}", pull.url);
            texts.push_str(&format!("{}\n", pull.url));
            break;
        }
    }

    let _ = upload_to_gist(&texts).await?;
    Ok(())
}

/* pub async fn github_to_db() -> anyhow::Result<()> {
    let start_date =
        NaiveDate::parse_from_str("2023-10-01", "%Y-%m-%d").expect("Failed to parse date");

    let mut date_point_vec = Vec::new();

    for i in 0..20 {
        let three_days_str = (start_date + Duration::days(2 * i as i64))
            .format("%Y-%m-%d")
            .to_string();

        date_point_vec.push(three_days_str);
    }

    let mut date_range_vec = date_point_vec
        .windows(2)
        .map(|x| x.join(".."))
        .collect::<Vec<_>>();

    let pool = PgPool::connect(&env::var("DATABASE_URL")?).await?;

    for date_range in date_range_vec {
        let query =
            format!("label:hacktoberfest-accepted is:pr is:merged created:{date_range} review:approved -label:spam -label:invalid");
        println!("query: {:?}", query.clone());
        let label_to_watch = "hacktoberfest-accepted";
        let pulls = get_pull_requests(&query, label_to_watch).await?;

        for pull in pulls {
            println!("pull: {:?}", pull.url);
            println!("pull: {:?}", pull.repository);

            let _ = add_pull_request_with_check(
                &pool,
                &pull.url,
                &pull.title,
                &pull.author,
                &pull.repository,
                &pull.merged_by,
                pull.cross_referenced_issues,
            )
            .await?;

            // pub async fn add_pull_request_with_check(
            //     pool: &sqlx::PgPool,
            //     pull_id: &str,
            //     title: &str,
            //     author: &str,
            //     repository: &str,
            //     merged_by: &str,
            //     cross_referenced_issues: Vec<String>,
            // let body = issue.body.chars().take(200).collect::<String>();
            // let title = issue.title.chars().take(200).collect::<String>();
            // let _ = (&pool, &issue.url, &title, &body).await?;
        }
    }
    Ok(())
} */
