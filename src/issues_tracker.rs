use chrono::{Datelike, NaiveDate, Timelike, Utc};
use dotenv::dotenv;
use flowsnet_platform_sdk::logger;
use github_flows::{get_octo, GithubLogin};
use octocrab_wasi::{models::issues::Issue, params::issues::Sort, params::Direction, Octocrab};
use openai_flows::{
    chat::{ChatModel, ChatOptions},
    OpenAIFlows,
};
use serde_json::{json, to_string_pretty, Value};
use std::env;

use anyhow::anyhow;
use chrono::Duration;
use http_req::{
    request::{Method, Request},
    uri::Uri,
};
use serde::{Deserialize, Serialize};

pub async fn search_for_initial_hits() -> anyhow::Result<()> {
    let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN env variable is required");
    // let octocrab = Octocrab::builder()
    //     .personal_token(token)
    //     .build()
    //     .expect("token invalid");
    // let one_hour_ago = (Utc::now() - Duration::hours(100i64))
    //     .format("%Y-%m-%dT%H:%M:%SZ")
    //     .to_string();
    let one_year_ago = (Utc::now() - Duration::weeks(52i64))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    // label:hacktoberfest is:issue is:open no:assignee
    let query = format!("label:hacktoberfest is:issue is:open no:assignee updated:>{one_year_ago}");
    let encoded_query = urlencoding::encode(&query);

    let query_url = format!("https://api.github.com/search/issues?q={encoded_query}");
    log::error!("query: {:?}", query_url.clone());

    // let issues = octocrab
    //     .search()
    //     .issues_and_pull_requests(&query)
    //     .sort("comments")
    //     .order("desc")
    //     .send()
    //     .await?;

    if let Ok(writer) = github_http_get(&query_url).await {
        let issues: Vec<Issue> = serde_json::from_slice(&writer).unwrap();

        for issue in issues {
            log::error!("issue: {:?}", issue.title);
        }
    }

    Ok(())
}

pub async fn github_http_post_gql(query: &str) -> anyhow::Result<Vec<u8>> {
    use http_req::{request::Method, request::Request, uri::Uri};
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

pub async fn github_http_get(url: &str) -> anyhow::Result<Vec<u8>> {
    let token = std::env::var("GITHUB_TOKEN").expect("github_token is required");
    let mut writer = Vec::new();
    let url = Uri::try_from(url).unwrap();

    match Request::new(&url)
        .method(Method::GET)
        .header("User-Agent", "flows-network connector")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {}", token))
        .header("CONNECTION", "close")
        .send(&mut writer)
    {
        Ok(res) => {
            if !res.status_code().is_success() {
                println!("Github http error {:?}", res.status_code());
                return Err(anyhow::anyhow!("Github http error {:?}", res.status_code()));
            }
            Ok(writer)
        }
        Err(_e) => {
            println!("Error getting response from Github: {:?}", _e);
            Err(anyhow::anyhow!(_e))
        }
    }
}

pub async fn get_project_logo(owner: &str, repo: &str) -> anyhow::Result<String> {
    #[derive(Serialize, Deserialize)]
    struct GraphQLResponse {
        data: RepositoryData,
    }

    #[derive(Serialize, Deserialize)]
    struct RepositoryData {
        repository: OwnerData,
    }

    #[derive(Serialize, Deserialize)]
    struct OwnerData {
        owner: OwnerInfo,
    }

    #[derive(Serialize, Deserialize)]
    struct OwnerInfo {
        login: String,
        avatarUrl: String,
    }

    let query_str = format!(
        r#"
        query {{
            repository(owner: "{owner}", name: "{repo}") {{
                owner {{
                    login
                    ... on User {{
                        avatarUrl
                    }}
                    ... on Organization {{
                        avatarUrl
                    }}
                }}
            }}
        }}
        "#,
    );

    let response = github_http_post_gql(&query_str).await?;

    let parsed_response: GraphQLResponse = serde_json::from_slice(&response)?;
    let owner_info = parsed_response.data.repository.owner;
    Ok(owner_info.avatarUrl)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OuterIssue {
    pub title: String,
    pub url: String,
    pub author: String,
    pub body: String,
    pub repository: String,
    pub repository_stars: i64,
    pub issue_labels: Vec<String>,
    pub comments: Vec<String>,             // Concat of author and comment
    pub cross_referenced_prs: Vec<String>, // URLs of cross-referenced pull requests
}

pub async fn get_issues(query: &str) -> anyhow::Result<Vec<OuterIssue>> {
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
        node: Option<LocalIssue>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct LocalIssue {
        title: Option<String>,
        url: Option<String>,
        author: Option<Author>,
        body: Option<String>,
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
        node: Option<CrossReferencedEvent>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct CrossReferencedEvent {
        source: Option<Source>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Source {
        __typename: Option<String>,
        url: Option<String>,
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
                search(query: "{}", type: ISSUE, first: 10, after: {}) {{
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
                                comments(first: {}) {{
                                    edges {{
                                        node {{
                                            author {{
                                                login
                                            }}
                                            body
                                        }}
                                    }}
                                }}
                                timelineItems(first: {}, itemTypes: [CROSS_REFERENCED_EVENT]) {{
                                    edges {{
                                        node {{
                                            ... on CrossReferencedEvent {{
                                                source {{
                                                    __typename
                                                    ... on PullRequest {{
                                                        url
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
            first_comments,
            first_timeline_items
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

                        let cross_referenced_prs =
                            issue.timelineItems.map_or(Vec::new(), |items| {
                                items.edges.map_or(Vec::new(), |edges| {
                                    edges
                                        .iter()
                                        .filter_map(|edge| {
                                            edge.node.as_ref().map(|item| {
                                                item.source.as_ref().map_or(
                                                    "".to_string(),
                                                    |source| {
                                                        source
                                                            .url
                                                            .as_ref()
                                                            .unwrap_or(&"".to_string())
                                                            .clone()
                                                    },
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
                            cross_referenced_prs: cross_referenced_prs,
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
    pub cross_referenced_issues: Vec<String>, // URLs of cross-referenced issues
    pub labels: Vec<String>,
    pub reviews: Vec<String>, // authors whose review state is approved
    pub merged_by: String,
}

pub async fn get_pull_requests(
    query: &str,
    label_to_watch: &str,
) -> anyhow::Result<Vec<OuterPull>> {
    #[derive(Serialize, Deserialize, Debug)]
    struct GraphQLResponse {
        data: Option<Data>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Data {
        search: Option<Search>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Search {
        issueCount: Option<i32>,
        edges: Option<Vec<Edge>>,
        pageInfo: PageInfo,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct PageInfo {
        endCursor: Option<String>,
        hasNextPage: bool,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Edge {
        node: Option<LocalPull>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct LocalPull {
        title: Option<String>,
        url: Option<String>,
        author: Option<Author>,
        labels: Option<Labels>,
        timelineItems: Option<TimelineItems>,
        hasApprovedReview: Option<Reviews>,
        mergedBy: Option<Author>,
        repository: Option<Repository>,
    }
    #[derive(Serialize, Deserialize, Debug)]
    struct Repository {
        url: Option<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Author {
        login: Option<String>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)] // Make sure to include Clone here
    struct Labels {
        edges: Option<Vec<LabelEdge>>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)] // And here
    struct LabelEdge {
        node: Option<Label>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)] // And also here
    struct Label {
        name: Option<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Comments {
        edges: Option<Vec<CommentEdge>>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct CommentEdge {
        node: Option<Comment>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Comment {
        body: Option<String>,
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
    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct TimelineItems {
        edges: Option<Vec<TimelineEdge>>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct TimelineEdge {
        node: Option<CrossReferencedEvent>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct CrossReferencedEvent {
        source: Option<Source>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Source {
        __typename: Option<String>,
        url: Option<String>,
        labels: Option<Labels>, // Add this line to include labels
    }

    let mut all_pulls = Vec::new();
    let mut after_cursor: Option<String> = None;

    for _n in 1..11 {
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
                                timelineItems(first: 10, itemTypes: [CROSS_REFERENCED_EVENT]) {{
                                    edges {{
                                        node {{
                                            ... on CrossReferencedEvent {{
                                                source {{
                                                    __typename
                                                    ... on Issue {{
                                                        url
                                                        labels(first: 10) {{
                                                            edges {{
                                                                node {{
                                                                    name
                                                                }}
                                                            }}
                                                        }}
                                                    }}
                                                }}
                                            }}
                                        }}
                                    }}
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

        let response_body = match github_http_post_gql(&query_str).await {
            Ok(res) => res,
            Err(e) => {
                log::error!("Error getting response from Github: {:?}", e);
                panic!("Error getting response from Github: {:?}", e)
            }
        };

        let response: GraphQLResponse = serde_json::from_slice(&response_body)?;
        log::error!("loop: {_n}");

        if let Some(data) = response.data {
            if let Some(search) = data.search {
                if let Some(edges) = search.edges {
                    for edge in edges {
                        if let Some(pull) = edge.node {
                            let labels = pull
                                .labels
                                .as_ref()
                                .and_then(|l| l.edges.as_ref())
                                .map_or(vec![], |edges| {
                                    edges
                                        .iter()
                                        .filter_map(|edge| {
                                            edge.node.as_ref().and_then(|label| label.name.clone())
                                        })
                                        .collect()
                                });

                            let cross_referenced_issues = pull
                                .timelineItems
                                .as_ref()
                                .and_then(|t| t.edges.as_ref())
                                .map_or(vec![], |edges| {
                                    edges
                                        .iter()
                                        .filter_map(|edge| {
                                            edge.node.as_ref().and_then(|item| {
                                                item.source.as_ref().and_then(|source| {
                                                    if source.__typename.as_deref() == Some("Issue") {
                                                        let has_hacktoberfest_accepted_label = source.labels.as_ref().map_or(false, |labels| {
                                                            labels.edges.as_ref().map_or(false, |edges| {
                                                                edges.iter().any(|edge| {
                                                                    edge.node.as_ref().map_or(false, |label| {
                                                                        label.name == Some(String::from(label_to_watch))
                                                                    })
                                                                })
                                                            })
                                                        });
                                                        
                            
                                                        if has_hacktoberfest_accepted_label {
                                                            source.url.clone()
                                                        } else {
                                                            None
                                                        }
                                                    } else {
                                                        None
                                                    }
                                                })
                                            })
                                        })
                                        .collect()
                                });

                            let reviews = pull
                                .hasApprovedReview
                                .as_ref()
                                .and_then(|r| r.edges.as_ref())
                                .map_or(vec![], |edges| {
                                    edges
                                        .iter()
                                        .filter_map(|edge| {
                                            edge.node.as_ref().and_then(|review| {
                                                review
                                                    .author
                                                    .as_ref()
                                                    .and_then(|author| author.login.clone())
                                            })
                                        })
                                        .collect()
                                });

                            let merged_by =
                                pull.mergedBy.as_ref().map_or(String::from("N/A"), |m| {
                                    m.login.clone().unwrap_or_default()
                                });

                            let repository_url = pull
                                .repository
                                .as_ref()
                                .and_then(|repo| repo.url.clone())
                                .unwrap_or_default();

                            all_pulls.push(OuterPull {
                                title: pull.title.clone().unwrap_or_default(),
                                url: pull.url.clone().unwrap_or_default(),
                                author: pull
                                    .author
                                    .as_ref()
                                    .and_then(|a| a.login.clone())
                                    .unwrap_or_default(),
                                repository: repository_url,
                                cross_referenced_issues,
                                labels,
                                reviews,
                                merged_by,
                            });
                        }
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

    Ok(all_pulls)
}

pub async fn upload_to_gist(content: &str) -> anyhow::Result<()> {
    let octocrab = get_octo(&GithubLogin::Default);

    let filename = format!("gh_search_{}.txt", Utc::now().format("%d-%m-%Y"));

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
