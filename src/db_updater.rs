/* use crate::issues_tracker::get_project_logo;
use sqlx::{postgres::PgPool, sqlx_macros};
use std::env;

pub async fn project_exists(pool: &PgPool, project_id: &str) -> anyhow::Result<bool> {
    let exists = sqlx::query!(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE project_id = $1) AS 'exists!'",
        project_id
    )
    .fetch_one(pool)
    .await?
    .exists;

    Ok(exists)
}
pub async fn add_project_with_check(
    pool: &PgPool,
    project_id: &str,
    project_logo: &str,
) -> anyhow::Result<()> {
    if project_exists(pool, project_id).await? {
        return Err(anyhow::anyhow!(
            "Project with ID '{}' already exists.",
            project_id
        ));
    }

    sqlx::query!(
        r#"
        INSERT INTO projects (project_id, project_logo)
        VALUES ($1, $2)
        "#,
        project_id,
        project_logo
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_project(
    pool: &PgPool,
    project_id: &str,
    project_logo: &str,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO projects (project_id, project_logo)
        VALUES ($1, $2)
        "#,
        project_id,
        project_logo
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_project_test_1(pool: &PgPool) -> anyhow::Result<()> {
    let project_id = "https://github.com/jaykchen/issue-labeler";
    let project_logo = "https://avatars.githubusercontent.com/u/112579101?v=4";

    let _ = add_project(pool, project_id, project_logo).await?;

    Ok(())
}

pub async fn list_projects(pool: &PgPool) -> anyhow::Result<()> {
    let recs = sqlx::query!(
        r#"
        SELECT project_id, project_logo
        FROM projects
        ORDER BY project_id
        "#
    )
    .fetch_all(pool)
    .await?;

    for rec in recs {
        println!("{}: {}", rec.project_id, rec.project_logo);
    }

    Ok(())
}

pub async fn issue_exists(pool: &PgPool, issue_id: &str) -> anyhow::Result<bool> {
    let exists = sqlx::query!(
        r#"
        SELECT EXISTS(SELECT 1 FROM issues WHERE issue_id = $1) AS "exists!"
        "#,
        issue_id
    )
    .fetch_one(pool)
    .await?
    .exists;

    Ok(exists)
}

pub async fn add_issue_with_check(
    pool: &PgPool,
    issue_id: &str,
    title: &str,
    description: &str,
) -> anyhow::Result<()> {
    // let issue_id = "https://github.com/jaykchen/issue-labeler/issues/24";

    let project_id = issue_id.rsplitn(3, '/').nth(2).unwrap();

    let owner_repo = project_id.rsplitn(3, '/').take(2).collect::<Vec<_>>();
    let owner = owner_repo[1];
    let repo = owner_repo[0];
    if project_exists(pool, project_id).await? {
    } else {
        let project_logo: String = get_project_logo(owner, repo).await?;

        add_project(pool, project_id, &project_logo).await?;
    }

    if issue_exists(pool, issue_id).await? {
    } else {
        sqlx::query!(
            r#"
            INSERT INTO issues (issue_id, project_id, issue_title, issue_description)
            VALUES ($1, $2, $3, $4)
            "#,
            issue_id,
            project_id,
            title,
            description,
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn add_issue(
    pool: &PgPool,
    issue_id: &str,
    project_id: &str,
    title: &str,
    description: &str,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO issues (issue_id, project_id, issue_title, issue_description)
        VALUES ($1, $2, $3, $4)
        "#,
        issue_id,
        project_id,
        title,
        description,
    )
    .execute(pool)
    .await?;
    Ok(())
}
pub async fn add_issue_test_1(pool: &PgPool) -> anyhow::Result<()> {
    let issue_id = "https://github.com/jaykchen/issue-labeler/issues/24";
    let project_id = "https://github.com/jaykchen/issue-labeler";
    let title = "WASI-NN with GPU on Jetson Orin Nano";
    let description = "demo";

    sqlx::query!(
        r#"
        INSERT INTO issues (issue_id, project_id, issue_title, issue_description)
        VALUES ($1, $2, $3, $4)
        "#,
        issue_id,
        project_id,
        title,
        description,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_issues(pool: &PgPool, project_id: &str) -> anyhow::Result<()> {
    let recs = sqlx::query!(
        r#"
        SELECT issue_id, issue_title, issue_description, issue_budget
        FROM issues
        WHERE project_id = $1
        ORDER BY issue_id
        "#,
        project_id
    )
    .fetch_all(pool)
    .await?;

    for rec in recs {
        println!(
            "- [{}] {}: {} (${:?})",
            rec.issue_id, rec.issue_title, rec.issue_description, rec.issue_budget
        );
    }

    Ok(())
}

pub async fn get_issue(pool: &PgPool, issue_id: &str) -> anyhow::Result<()> {
    let recs = sqlx::query!(
        r#"
        SELECT issue_id, issue_title, issue_description, issue_budget
        FROM issues
        WHERE issue_id = $1
        ORDER BY issue_id
        "#,
        issue_id
    )
    .fetch_all(pool)
    .await?;

    for rec in recs {
        println!(
            "- [{}] {}: {} (${:?})",
            rec.issue_id, rec.issue_title, rec.issue_description, rec.issue_budget
        );
    }

    Ok(())
}

pub async fn add_comment(
    pool: &PgPool,
    comment_id: &str,
    issue_id: &str,
    creator: &str,
    content: &str,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO comments (comment_id, issue_id, creator, content)
        VALUES ($1, $2, $3, $4)
        "#,
        comment_id,
        issue_id,
        creator,
        content
    )
    .execute(pool)
    .await?;
    Ok(())
}
pub async fn add_comment_with_check(
    pool: &PgPool,
    comment_id: &str,
    issue_id: &str,
    creator: &str,
    content: &str,
) -> anyhow::Result<()> {
    if issue_exists(pool, issue_id).await? {
    } else {
        let _ = add_issue_with_check(pool, issue_id, "title", "description").await?;
    }

    sqlx::query!(
        r#"
            INSERT INTO comments (comment_id, issue_id, creator, content)
            VALUES ($1, $2, $3, $4)
            "#,
        comment_id,
        issue_id,
        creator,
        content
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_comment_test_1(pool: &PgPool) -> anyhow::Result<()> {
    let comment_id = "https://github.com/jaykchen/issue-labeler/issues/24#issuecomment-1979927212";
    let issue_id = "https://github.com/jaykchen/issue-labeler/issues/24";
    let creator = "jaykchen";
    let content = "This is a placeholder comment on this issue.";
    sqlx::query!(
        r#"
        INSERT INTO comments (comment_id, issue_id, creator, content)
        VALUES ($1, $2, $3, $4)
        "#,
        comment_id,
        issue_id,
        creator,
        content
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_comments(pool: &PgPool, issue_id: &str) -> anyhow::Result<()> {
    let recs = sqlx::query!(
        r#"
        SELECT comment_id, content
        FROM comments
        WHERE issue_id = $1
        ORDER BY comment_id
        "#,
        issue_id
    )
    .fetch_all(pool)
    .await?;

    for rec in recs {
        println!("- [{}] {}", rec.comment_id, rec.content);
    }

    Ok(())
}

pub async fn list_pull_requests(
    pool: &sqlx::PgPool,
) -> anyhow::Result<Vec<(String, String, String, String, String, Vec<String>)>> {
    let pull_requests = sqlx::query!(
        r#"
        SELECT pull_id, title, author, repository, merged_by, cross_referenced_issues
        FROM pull_requests
        "#
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| {
        (
            r.pull_id.clone(),
            r.title.clone(),
            r.author.clone(),
            r.repository.clone(),
            r.merged_by.clone(),
            r.cross_referenced_issues.clone().unwrap_or_default(), // Handle Option<Vec<String>>
        )
    })
    .collect();

    Ok(pull_requests)
}

pub async fn add_pull_request_with_check(
    pool: &sqlx::PgPool,
    pull_id: &str,
    title: &str,
    author: &str,
    repository: &str,
    merged_by: &str,
    cross_referenced_issues: Vec<String>,
) -> anyhow::Result<()> {
    let exists = sqlx::query!(
        r#"
        SELECT EXISTS(SELECT 1 FROM pull_requests WHERE pull_id = $1)
        "#,
        pull_id
    )
    .fetch_one(pool)
    .await?
    .exists
    .unwrap_or(false);

    if !exists {
        add_pull_request(
            pool,
            pull_id,
            title,
            author,
            repository,
            merged_by,
            cross_referenced_issues,
        )
        .await?;
    }

    Ok(())
}
pub async fn add_pull_request(
    pool: &sqlx::PgPool,
    pull_id: &str,
    title: &str,
    author: &str,
    repository: &str,
    merged_by: &str,
    cross_referenced_issues: Vec<String>,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO pull_requests (pull_id, title, author, repository, merged_by, cross_referenced_issues)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        pull_id,
        title,
        author,
        repository,
        merged_by,
        &cross_referenced_issues,
    )
    .execute(pool)
    .await?;

    Ok(())
}
 */
