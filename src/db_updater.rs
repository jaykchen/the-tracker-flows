use dotenv::dotenv;
use mysql_async::prelude::*;
pub use mysql_async::*;
use serde_json::{json, Value};

pub async fn get_pool() -> Pool {
    dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("not url db url found");

    let opts = Opts::from_url(&url).unwrap();
    let builder = OptsBuilder::from_opts(opts);
    // The connection pool will have a min of 5 and max of 10 connections.
    let constraints = PoolConstraints::new(5, 10).unwrap();
    let pool_opts = PoolOpts::default().with_constraints(constraints);

    Pool::new(builder.pool_opts(pool_opts))
}

pub async fn project_exists(pool: &mysql_async::Pool, project_id: &str) -> Result<bool> {
    let mut conn = pool.get_conn().await?;
    let result: Option<(i32,)> = conn
        .query_first(format!(
            "SELECT 1 FROM projects WHERE project_id = '{}'",
            project_id
        ))
        .await?;
    Ok(result.is_some())
}

pub async fn add_issues_open(
    pool: &Pool,
    issue_id: &str,
    project_id: &str,
    issue_title: &str,
    issue_description: &str,
    repo_stars: i32,
    repo_avatar: &str,
) -> Result<()> {
    let mut conn = pool.get_conn().await?;

    let query = r"INSERT INTO issues_open (issue_id, project_id, issue_title, issue_description, repo_stars, repo_avatar)
                  VALUES (:issue_id, :project_id, :issue_title, :issue_description, :repo_stars, :repo_avatar)";

    conn.exec_drop(
        query,
        params! {
            "issue_id" => issue_id,
            "project_id" => project_id,
            "issue_title" => issue_title,
            "issue_description" => issue_description,
            "repo_stars" => repo_stars,
            "repo_avatar" => repo_avatar,
        },
    )
    .await?;

    Ok(())
}

pub async fn add_issues_closed(
    pool: &Pool,
    issue_id: &str,
    issue_assignees: &Vec<String>,
    issue_linked_pr: &str,
) -> Result<()> {
    let mut conn = pool.get_conn().await?;

    let issue_assignees_json: Value = json!(issue_assignees).into();

    let query = r"INSERT INTO issues_closed (issue_id,  issue_assignees, issue_linked_pr)
                  VALUES (:issue_id, :issue_assignees, :issue_linked_pr)";

    conn.exec_drop(
        query,
        params! {
            "issue_id" => issue_id,
            "issue_assignees" => &issue_assignees_json,
            "issue_linked_pr" => issue_linked_pr,
        },
    )
    .await?;

    Ok(())
}

pub async fn add_issues_comments(
    pool: &Pool,
    issue_id: &str,
    comments: &Vec<String>,
) -> Result<()> {
    let mut conn = pool.get_conn().await?;

    // let issue_status = todo!(comments);
    let issue_status = comments[0].clone();

    let query = r"INSERT INTO issues_comments (issue_id, issue_status)
                  VALUES (:issue_id, :issue_status)";

    conn.exec_drop(
        query,
        params! {
            "issue_id" => issue_id,
            "issue_status" => issue_status,
        },
    )
    .await?;

    Ok(())
}
pub async fn add_pull_request(
    pool: &Pool,
    pull_id: &str,
    title: &str,
    author: &str,
    project_id: &str,
    merged_by: &str,
    connected_issues: &Vec<String>,
    pull_status: &str,
) -> Result<()> {
    let mut conn = pool.get_conn().await?;

    let connected_issues_json: Value = json!(connected_issues).into();

    let query = r"INSERT INTO pull_requests (pull_id, title, author, project_id, merged_by, connected_issues, pull_status)
                  VALUES (:pull_id, :title, :author, :project_id, :merged_by, :connected_issues, :pull_status)";

    match conn
        .exec_drop(
            query,
            params! {
                "pull_id" => pull_id,
                "title" => title,
                "author" => author,
                "project_id" => project_id,
                "connected_issues" => &connected_issues_json,
                "merged_by" => merged_by,
                "pull_status" => pull_status,
            },
        )
        .await
    {
        Ok(()) => println!("Pull request added successfully"),
        Err(e) => println!("Error adding pull request: {:?}", e),
    }

    Ok(())
}
