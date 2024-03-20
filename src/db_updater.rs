use anyhow::Result;
use dotenv::dotenv;
use mysql_async::Error;
pub use mysql_async::*;
use mysql_async::{prelude::*, Pool};

async fn get_pool() -> Pool {
    dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("not url db url found");

    let opts = Opts::from_url(&url).unwrap();
    let builder = OptsBuilder::from_opts(opts);
    // The connection pool will have a min of 5 and max of 10 connections.
    let constraints = PoolConstraints::new(5, 10).unwrap();
    let pool_opts = PoolOpts::default().with_constraints(constraints);

    Pool::new(builder.pool_opts(pool_opts))
}

pub async fn project_exists(
    pool: &mysql_async::Pool,
    project_id: &str,
) -> Result<bool, mysql_async::Error> {
    let mut conn = pool.get_conn().await?;
    let result: Option<(i32,)> = conn
        .query_first(format!(
            "SELECT 1 FROM projects WHERE project_id = '{}'",
            project_id
        ))
        .await?;
    Ok(result.is_some())
}

pub async fn add_project(
    pool: &mysql_async::Pool,
    project_id: &str,
    project_logo: &str,
    issue_id: &str,
) -> Result<()> {
    let mut conn = pool.get_conn().await?;
    let issue_id_json: Value = serde_json::json!(issue_id).into();

    let query = r"INSERT INTO projects (project_id, project_logo, issues_list)
                  VALUES (:project_id, :project_logo, :issues_list)";

    conn.exec_drop(
        query,
        params! {
            "project_id" => project_id,
            "project_logo" => project_logo,
            "issues_list" => issue_id_json,
        },
    )
    .await?;

    Ok(())
}

pub async fn update_project(
    pool: &mysql_async::Pool,
    project_id: &str,
    issue_id: &str,
) -> Result<(), Error> {
    let mut conn = pool.get_conn().await?;

    let issue_id_json: Value = serde_json::json!(issue_id).into();

    let params = params! {
        "issue_id" => &issue_id_json,
        "project_id" => project_id,
    };
    "UPDATE projects
        SET issues_list = JSON_ARRAY_APPEND(issues_list, '$', :issue_id)
        WHERE project_id = :project_id"
        .with(params)
        .run(&mut conn)
        .await?;

    Ok(())
}

    use async_trait::async_trait;
    use mysql_async::prelude::Queryable;

    #[async_trait]
    trait TestDbSetup {
        async fn setup_db(&self);
    }

    #[async_trait]
    impl TestDbSetup for Pool {
        async fn setup_db(&self) {
            let mut conn = self.get_conn().await.unwrap();
            conn.query_drop("CREATE DATABASE IF NOT EXISTS test_db")
                .await
                .unwrap();
            conn.query_drop("USE test_db").await.unwrap();
            conn.query_drop(
                "CREATE TABLE IF NOT EXISTS projects (
                    project_id VARCHAR(255) PRIMARY KEY,
                    project_logo VARCHAR(255) NOT NULL,
                    issues_list JSON
                )",
            )
            .await
            .unwrap();
        }
    }

 pub   async fn test_project_exists() {
        let pool = get_pool().await;
        pool.setup_db().await;
        let project_id = "https://github.com/test/test13";

        // // Add a project
        // add_project(&pool, project_id, "test_logo", "test_issue_id")
        //     .await
        //     .unwrap();

        // Now the project should exist
        assert_eq!(project_exists(&pool, project_id).await.unwrap(), true);
    }

    pub async fn test_add_project() {
        let pool = get_pool().await;
        pool.setup_db().await;
        let project_id = "https://github.com/test/test15";

        let issue_id = "test_issue_id";
        let res = add_project(&pool, project_id, "test_logo", issue_id).await;
        println!("res: {:?}", res);
        // The project should now exist
        assert_eq!(project_exists(&pool, project_id).await.unwrap(), true);
    }

