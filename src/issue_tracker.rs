use anyhow::anyhow;
use chrono::{Datelike, Duration, NaiveDate, Timelike, Utc};
use dotenv::dotenv;
use flowsnet_platform_sdk::logger;
use github_flows::{get_octo, GithubLogin};
use http_req::{
    request::{Method, Request},
    response::Response,
    uri::Uri,
};
// use octocrab_wasi::{
//     models::{issues::Issue, pulls},
//     params::{issues::Sort, Direction},
//     search,
// };

use schedule_flows::{schedule_cron_job, schedule_handler};
use serde::{Deserialize, Serialize};
use std::env;


pub fn inner_query_by_date_range(
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

pub async fn github_http_post_gql(query: &str) -> anyhow::Result<Vec<u8>> {
    let token = env::var("GITHUB_TOKEN").expect("github_token is required");
    let base_url = "https://api.github.com/graphql";
    let base_url = Uri::try_from(base_url).unwrap();
    let mut writer = Vec::new();

    let query = serde_json::json!({"query": query});
    match Request::new(&base_url)
        .method(Method::POST)
        .header("User-Agent", "flows-network connector")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {}", token))
        .header("Content-Length", &query.to_string().len())
        .body(&query.to_string().into_bytes())
        .send(&mut writer)
    {
        Ok(res) => {
            if !res.status_code().is_success() {
                log::error!("Github http error {:?}", res.status_code());
                return Err(anyhow::anyhow!("Github http error {:?}", res.status_code()));
            }
            Ok(writer)
        }
        Err(_e) => {
            log::error!("Error getting response from Github: {:?}", _e);
            Err(anyhow::anyhow!(_e))
        }
    }
}

pub async fn upload_to_gist(content: &str) -> anyhow::Result<()> {
    let octocrab = get_octo(&GithubLogin::Default);

    let filename = format!("gh_search_{}.txt", Utc::now().format("%H:%M:%S%.f"));

    let _ = octocrab
        .gists()
        .create()
        .description("Daily Tracking Report")
        .public(false) // set to true if you want the gist to be public
        .file(filename, content)
        .send()
        .await?;

    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OuterIssue {
    pub title: String,
    pub url: String,
    pub author: String,
    pub body: String,
    pub repository: String,
    pub repository_stars: i64,
    pub repository_avatar: String,
    pub issue_labels: Vec<String>,
    pub comments: Vec<String>,
}

pub async fn search_issues_open(query: &str) -> anyhow::Result<Vec<OuterIssue>> {
    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct GraphQLResponse {
        data: Option<Data>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Data {
        search: Option<Search>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Search {
        issueCount: Option<i32>,
        edges: Option<Vec<Edge>>,
        pageInfo: PageInfo,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct PageInfo {
        endCursor: Option<String>,
        hasNextPage: bool,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Edge {
        node: Option<Issue>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Issue {
        title: Option<String>,
        url: Option<String>,
        body: Option<String>,
        author: Option<Author>,
        repository: Option<Repository>,
        labels: Option<Labels>,
        comments: Option<Comments>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Author {
        login: Option<String>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Owner {
        avatarUrl: Option<String>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Repository {
        url: Option<String>,
        stargazers: Option<Stargazers>,
        owner: Option<Owner>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Stargazers {
        totalCount: Option<i64>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Labels {
        edges: Option<Vec<LabelEdge>>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct LabelEdge {
        node: Option<Label>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Label {
        name: Option<String>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Comments {
        edges: Option<Vec<CommentEdge>>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct CommentEdge {
        node: Option<Comment>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Comment {
        author: Option<Author>,
        body: Option<String>,
    }

    let first_comments = 10;
    let first_timeline_items = 10;
    let mut all_issues = Vec::new();
    let mut after_cursor: Option<String> = None;
    let file_path = "issues.txt";
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)?;
    let mut count = 0;

    for _ in 0..10 {
        let query_str = format!(
            r#"
            query {{
                search(query: "{}", type: ISSUE, first: 100, after: {}) {{
                    issueCount
                    edges {{
                        node {{
                            ... on Issue {{
                                title
                                url
                                body
                                author {{
                                    login
                                }}
                                repository {{
                                    url
                                    stargazers {{
                                        totalCount
                                    }}
                                    owner {{
                                        avatarUrl
                                    }}
                                }}
                                labels(first: 10) {{
                                    edges {{
                                        node {{
                                            name
                                        }}
                                    }}
                                }}
                                comments(first: 10) {{
                                    edges {{
                                        node {{
                                            author {{
                                                login
                                            }}
                                            body
                                        }}
                                    }}
                                }}
                            }}
                        }}
                    }}
                    pageInfo {{
                        endCursor
                        hasNextPage
                    }}
                }}
            }}
            "#,
            query.replace("\"", "\\\""),
            after_cursor
                .as_ref()
                .map_or(String::from("null"), |c| format!("\"{}\"", c)),
        );

        let response_body = github_http_post_gql(&query_str)
            .await
            .map_err(|e| anyhow!("Failed to post GraphQL query: {}", e))?;

        let response: GraphQLResponse = serde_json::from_slice(&response_body)
            .map_err(|e| anyhow!("Failed to deserialize response: {}", e))?;

        if let Some(data) = response.data {
            if let Some(search) = data.search {
                for edge in search.edges.unwrap_or_default() {
                    if let Some(issue) = edge.node {
                        let labels = issue.labels.map_or(Vec::new(), |labels| {
                            labels.edges.map_or(Vec::new(), |edges| {
                                edges
                                    .iter()
                                    .filter_map(|edge| {
                                        edge.node
                                            .as_ref()
                                            .map(|label| label.name.clone().unwrap_or_default())
                                    })
                                    .collect()
                            })
                        });
                        let temp_str = String::from("");
                        let comments = issue.comments.map_or(Vec::new(), |comments| {
                            comments.edges.map_or(Vec::new(), |edges| {
                                edges
                                    .iter()
                                    .filter_map(|edge| {
                                        edge.node.as_ref().map(|comment| {
                                            format!(
                                                "{}: {}",
                                                comment.author.as_ref().map_or("", |a| a
                                                    .login
                                                    .as_ref()
                                                    .unwrap_or(&temp_str)),
                                                comment.body.as_ref().unwrap_or(&"".to_string())
                                            )
                                        })
                                    })
                                    .collect()
                            })
                        });

                        all_issues.push(OuterIssue {
                            title: issue.title.unwrap_or_default(),
                            url: issue.url.unwrap_or_default(),
                            author: issue
                                .author
                                .clone()
                                .map_or(String::new(), |author| author.login.unwrap_or_default()),
                            body: issue.body.clone().unwrap_or_default(),
                            repository: issue
                                .repository
                                .clone() // Clone here
                                .map_or(String::new(), |repo| repo.url.unwrap_or_default()),
                            repository_stars: issue.repository.clone().map_or(0, |repo| {
                                repo.stargazers
                                    .map_or(0, |stars| stars.totalCount.unwrap_or(0))
                            }),
                            repository_avatar: issue.repository.map_or(String::new(), |repo| {
                                repo.owner.map_or(String::new(), |owner| {
                                    owner.avatarUrl.unwrap_or_default()
                                })
                            }),
                            issue_labels: labels,
                            comments: comments,
                        });
                    }
                }
                if search.pageInfo.hasNextPage {
                    after_cursor = search.pageInfo.endCursor
                } else {
                    break;
                }
            }
        }
    }

    Ok(all_issues)
}





#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CloseOuterIssue {
    pub title: String,
    pub url: String,
    pub author: String,
    pub body: String,
    pub repository: String,
    pub repository_stars: i64,
    pub issue_labels: Vec<String>,
    pub comments: Vec<String>, // Concat of author and comment
    pub close_reason: String,
    pub close_pull_request: String,
    pub close_author: String,
}

pub async fn search_issues_closed(query: &str) -> anyhow::Result<Vec<CloseOuterIssue>> {
    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct GraphQLResponse {
        data: Option<Data>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Data {
        search: Option<Search>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Search {
        issueCount: Option<i32>,
        edges: Option<Vec<Edge>>,
        pageInfo: PageInfo,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct PageInfo {
        endCursor: Option<String>,
        hasNextPage: bool,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Edge {
        node: Option<Issue>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Issue {
        title: Option<String>,
        url: Option<String>,
        body: Option<String>,
        author: Option<Author>,
        repository: Option<Repository>,
        labels: Option<Labels>,
        comments: Option<Comments>,
        timelineItems: Option<TimelineItems>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Author {
        login: Option<String>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Repository {
        url: Option<String>,
        stargazers: Option<Stargazers>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Stargazers {
        totalCount: Option<i64>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Labels {
        edges: Option<Vec<LabelEdge>>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct LabelEdge {
        node: Option<Label>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Label {
        name: Option<String>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Comments {
        edges: Option<Vec<CommentEdge>>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct CommentEdge {
        node: Option<Comment>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Comment {
        author: Option<Author>,
        body: Option<String>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct TimelineItems {
        edges: Option<Vec<TimelineEdge>>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct TimelineEdge {
        node: Option<ClosedEvent>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct ClosedEvent {
        stateReason: Option<String>,
        closer: Option<Closer>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Closer {
        title: Option<String>,
        url: Option<String>,
        author: Option<Author>,
    }

    let first_comments = 10;
    let first_timeline_items = 10;
    let mut all_issues = Vec::new();
    let mut after_cursor: Option<String> = None;
    let file_path = "issues.txt";
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)?;
    let mut count = 0;

    for _ in 0..10 {
        let query_str = format!(
            r#"
            query {{
                search(query: "{}", type: ISSUE, first: 100, after: {}) {{
                    issueCount
                    edges {{
                        node {{
                            ... on Issue {{
                                title
                                url
                                body
                                author {{
                                    login
                                }}
                                repository {{
                                    url
                                    stargazers {{
                                        totalCount
                                    }}
                                }}
                                labels(first: 10) {{
                                    edges {{
                                        node {{
                                            name
                                        }}
                                    }}
                                }}
                                comments(first: 10) {{
                                    edges {{
                                        node {{
                                            author {{
                                                login
                                            }}
                                            body
                                        }}
                                    }}
                                }}
                                timelineItems(first: 10, itemTypes: [CLOSED_EVENT]) {{
                                    edges {{
                                        node {{
                                            ... on ClosedEvent {{
                                                stateReason
                                                closer {{
                                                    __typename
                                                    ... on PullRequest {{
                                                        title
                                                        url
                                                        author {{
                                                            login
                                                        }}
                                                    }}
                                                }}
                                            }}
                                        }}
                                    }}
                                }}
                            }}
                        }}
                    }}
                    pageInfo {{
                        endCursor
                        hasNextPage
                    }}
                }}
            }}
            "#,
            query.replace("\"", "\\\""),
            after_cursor
                .as_ref()
                .map_or(String::from("null"), |c| format!("\"{}\"", c)),
        );

        let response_body = github_http_post_gql(&query_str)
            .await
            .map_err(|e| anyhow!("Failed to post GraphQL query: {}", e))?;

        let response: GraphQLResponse = serde_json::from_slice(&response_body)
            .map_err(|e| anyhow!("Failed to deserialize response: {}", e))?;

        if let Some(data) = response.data {
            if let Some(search) = data.search {
                for edge in search.edges.unwrap_or_default() {
                    if let Some(issue) = edge.node {
                        let labels = issue.labels.map_or(Vec::new(), |labels| {
                            labels.edges.map_or(Vec::new(), |edges| {
                                edges
                                    .iter()
                                    .filter_map(|edge| {
                                        edge.node
                                            .as_ref()
                                            .map(|label| label.name.clone().unwrap_or_default())
                                    })
                                    .collect()
                            })
                        });
let temp_str   = String::from("");

                        let comments = issue.comments.map_or(Vec::new(), |comments| {
                            comments.edges.map_or(Vec::new(), |edges| {
                                edges
                                    .iter()
                                    .filter_map(|edge| {
                                        edge.node.as_ref().map(|comment| {
                                            format!(
                                                "{}: {}",
                                                comment.author.as_ref().map_or("", |a| a
                                                    .login
                                                    .as_ref()
                                                    .unwrap_or(&temp_str)),
                                                comment.body.as_ref().unwrap_or(&"".to_string())
                                            )
                                        })
                                    })
                                    .collect()
                            })
                        });

                        let (close_reason, close_pull_request, close_author) = issue
                            .timelineItems
                            .map_or((String::new(), String::new(), String::new()), |items| {
                                items.edges.map_or(
                                    (String::new(), String::new(), String::new()),
                                    |edges| {
                                        edges
                                            .iter()
                                            .filter_map(|edge| {
                                                edge.node.as_ref().map(|event| {
                                                    if let Some(closer) = &event.closer {
                                                        (
                                                            event
                                                                .stateReason
                                                                .clone()
                                                                .unwrap_or_default(),
                                                            closer
                                                                .url
                                                                .clone()
                                                                .unwrap_or_default(),
                                                            closer.author.as_ref().map_or(
                                                                String::new(),
                                                                |author| {
                                                                    author
                                                                        .login
                                                                        .clone()
                                                                        .unwrap_or_default()
                                                                },
                                                            ),
                                                        )
                                                    } else {
                                                        (
                                                            String::new(),
                                                            String::new(),
                                                            String::new(),
                                                        )
                                                    }
                                                })
                                            })
                                            .next()
                                            .unwrap_or((
                                                String::new(),
                                                String::new(),
                                                String::new(),
                                            ))
                                    },
                                )
                            });

                        all_issues.push(CloseOuterIssue {
                            title: issue.title.unwrap_or_default(),
                            url: issue.url.unwrap_or_default(),
                            author: issue
                                .author
                                .map_or(String::new(), |author| author.login.unwrap_or_default()),
                            body: issue.body.unwrap_or_default(),
                            repository: issue
                                .repository
                                .clone() // Clone here
                                .map_or(String::new(), |repo| repo.url.unwrap_or_default()),
                            repository_stars: issue.repository.map_or(0, |repo| {
                                repo.stargazers
                                    .map_or(0, |stars| stars.totalCount.unwrap_or(0))
                            }),
                            issue_labels: labels,
                            comments: comments,
                            close_reason: close_reason,
                            close_pull_request: close_pull_request,
                            close_author: close_author,
                        });
                    }
                }
                if search.pageInfo.hasNextPage {
                    after_cursor = search.pageInfo.endCursor
                } else {
                    break;
                }
            }
        }
    }
    Ok(all_issues)
}



#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OuterPull {
    pub title: String,
    pub url: String,
    pub author: String,
    pub repository: String, // URL of the repository where the pull request was opened
    pub labels: Vec<String>,
    pub reviews: Vec<String>, // authors whose review state is approved
    pub merged_by: String,
}

pub async fn overall_search_pull_requests(query: &str) -> anyhow::Result<Vec<OuterPull>> {
    #[derive(Serialize, Deserialize, Debug)]
    struct GraphQLResponse {
        data: Data,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Data {
        search: Search,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Search {
        issueCount: i32,
        edges: Vec<Edge>,
        pageInfo: PageInfo,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct PageInfo {
        endCursor: Option<String>,
        hasNextPage: bool,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Edge {
        node: PullRequest,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct PullRequest {
        title: String,
        url: String,
        repository: Repository,
        author: Option<Author>,
        labels: Labels,
        hasApprovedReview: Reviews,
        mergedBy: Option<Author>,
    }
    #[derive(Serialize, Deserialize, Debug)]
    struct Repository {
        url: Option<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Author {
        login: Option<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Labels {
        edges: Option<Vec<LabelEdge>>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct LabelEdge {
        node: Option<Label>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Label {
        name: Option<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Reviews {
        edges: Option<Vec<ReviewEdge>>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct ReviewEdge {
        node: Option<Review>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Review {
        author: Option<Author>,
    }
    
    let mut all_pulls = Vec::new();
    let mut after_cursor = None;
    for _n in 0..10 {
        let query_str = format!(
            r#"
            query {{
                search(query: "{}", type: ISSUE, first: 100, after: {}) {{
                    issueCount
                    edges {{
                        node {{
                            ... on PullRequest {{
                                title
                                url
                                repository {{
                                    url
                                }}
                                author {{
                                    login
                                }}
                                labels(first: 10) {{
                                    edges {{
                                        node {{
                                            name
                                        }}
                                    }}
                                }}
                                hasApprovedReview: reviews(first: 5, states: [APPROVED]) {{
                                    edges {{
                                        node {{
                                            author {{
                                                login
                                            }}
                                            state
                                        }}
                                    }}
                                }}
                                mergedBy {{
                                    login
                                }}
                            }}
                        }}
                    }}
                    pageInfo {{
                        endCursor
                        hasNextPage
                    }}
                }}
            }}
            "#,
            query,
            after_cursor
                .as_ref()
                .map_or(String::from("null"), |c| format!("\"{}\"", c))
        );

        let response_body = github_http_post_gql(&query_str).await?;
        let response: GraphQLResponse = serde_json::from_slice(&response_body)?;

        for edge in response.data.search.edges {
            let pull = edge.node;

            let labels = pull
                .labels
                .edges
                .as_ref()
                .unwrap_or(&Vec::new())
                .into_iter()
                .filter_map(|edge| edge.node.as_ref())
                .map(|node| node.name.clone())
                .collect::<Vec<Option<_>>>();

            let reviews = pull
                .hasApprovedReview
                .edges
                .as_ref()
                .unwrap_or(&Vec::new())
                .into_iter()
                .filter_map(|edge| edge.node.as_ref())
                .map(|node| node.author.as_ref().and_then(|author| author.login.clone()))
                .collect::<Vec<Option<_>>>();

            all_pulls.push(OuterPull {
                title: pull.title.clone(),
                url: pull.url.clone(),
                author: pull
                    .author
                    .as_ref()
                    .and_then(|author| author.login.clone())
                    .unwrap_or_else(|| String::from("Unknown author")),
                repository: pull
                    .repository
                    .url
                    .clone()
                    .unwrap_or_else(|| String::from("Unknown repository")),
                labels: labels.into_iter().filter_map(|x| x).collect(),
                reviews: reviews.into_iter().filter_map(|x| x).collect(),
                merged_by: pull
                    .mergedBy
                    .as_ref()
                    .and_then(|author| author.login.clone())
                    .unwrap_or_else(|| String::from("Unknown merged_by")),
            });
        }

        match response.data.search.pageInfo {
            PageInfo {
                hasNextPage: true,
                endCursor: Some(cursor),
            } => after_cursor = Some(cursor),
            _ => break,
        }
    }
    Ok(all_pulls)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SimplePull {
    pub title: String,
    pub url: String,
    pub author: String,
    pub connected_issues: Vec<String>,
    pub labels: Vec<String>,
    pub reviews: Vec<String>,      // authors whose review state is approved
    pub merged_by: Option<String>, // This field can be empty if the PR is not merged
}

pub async fn get_per_repo_pull_requests(query: &str) -> anyhow::Result<Vec<SimplePull>> {
    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct GraphQLResponse {
        data: Data,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Data {
        search: Search,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Search {
        issueCount: i32,
        nodes: Vec<Node>,
        pageInfo: PageInfo,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Node {
        title: String,
        url: String,
        author: Author,
        timelineItems: TimelineItems,
        labels: Labels,
        reviews: Reviews,
        mergedBy: Option<Author>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Author {
        login: String,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct TimelineItems {
        nodes: Vec<TimelineEvent>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct TimelineEvent {
        subject: Option<Subject>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Subject {
        url: String,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Labels {
        nodes: Vec<Label>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Label {
        name: String,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Reviews {
        nodes: Vec<Review>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Review {
        author: Author,
        state: String,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct PageInfo {
        endCursor: Option<String>,
        hasNextPage: bool,
    }

    let mut simplified_pulls = Vec::new();
    let mut after_cursor: Option<String> = None;

    for _n in 0..10 {
        let query_str = format!(
            r#"
            query {{
                search(query: "{}", type: ISSUE, first: 100, after: {}) {{
                    issueCount
                    nodes {{
                        ... on PullRequest {{
                            title
                            url
                            author {{
                                login
                            }}
                            timelineItems(first: 5, itemTypes: [CONNECTED_EVENT]) {{
                                nodes {{
                                    ... on ConnectedEvent {{
                                        subject {{
                                            ... on Issue {{
                                                url
                                            }}
                                        }}
                                    }}
                                }}
                            }}
                            labels(first: 10) {{
                                nodes {{
                                    name
                                }}
                            }}
                            reviews(first: 5, states: [APPROVED]) {{
                                nodes {{
                                    author {{
                                        login
                                    }}
                                    state
                                }}
                            }}
                            mergedBy {{
                                login
                            }}
                        }}
                    }}
                    pageInfo {{
                        endCursor
                        hasNextPage
                    }}
                }}
            }}
            "#,
            query,
            after_cursor
                .as_ref()
                .map_or(String::from("null"), |c| format!("\"{}\"", c))
        );

        let response_body = github_http_post_gql(&query_str).await?;
        let response: GraphQLResponse = serde_json::from_slice(&response_body)?;

        for node in response.data.search.nodes {
            let connected_issues = node
                .timelineItems
                .nodes
                .iter()
                .filter_map(|event| event.subject.as_ref().map(|subject| subject.url.clone()))
                .collect::<Vec<String>>();

            let labels = node
                .labels
                .nodes
                .iter()
                .map(|label| label.name.clone())
                .collect::<Vec<String>>();

            let reviews = node
                .reviews
                .nodes
                .iter()
                .filter(|review| review.state == "APPROVED")
                .map(|review| review.author.login.clone())
                .collect::<Vec<String>>();

            simplified_pulls.push(SimplePull {
                title: node.title,
                url: node.url,
                author: node.author.login,
                connected_issues,
                labels,
                reviews,
                merged_by: node.mergedBy.as_ref().map(|author| author.login.clone()),
            });
        }
        match response.data.search.pageInfo {
            PageInfo {
                hasNextPage: true,
                endCursor: Some(cursor),
            } => after_cursor = Some(cursor),
            _ => break,
        }
    }

    Ok(simplified_pulls)
}
