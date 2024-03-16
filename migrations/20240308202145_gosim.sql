CREATE TYPE review_status AS ENUM ('queue', 'approve', 'decline');

CREATE TABLE projects (
    project_id VARCHAR PRIMARY KEY,
    project_logo VARCHAR NOT NULL,
    issues_list TEXT[]
);

CREATE TABLE issues (
    issue_id VARCHAR PRIMARY KEY,
        project_id VARCHAR NOT NULL,
    issue_title VARCHAR NOT NULL,
    issue_description TEXT NOT NULL,
    issue_budget INT,
    issue_assignee VARCHAR,
    issue_linked_pr VARCHAR,
    issue_status VARCHAR,
    review_status review_status,
    issue_budget_approved BOOLEAN
);

CREATE TABLE comments (
    comment_id VARCHAR PRIMARY KEY,
    issue_id VARCHAR NOT NULL,
    creator VARCHAR NOT NULL,
    time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    content TEXT NOT NULL
);

CREATE TABLE pull_requests (
    pull_id VARCHAR PRIMARY KEY,
    title VARCHAR NOT NULL,
    author VARCHAR NOT NULL,
    repository VARCHAR NOT NULL,
    merged_by VARCHAR NOT NULL,
    cross_referenced_issues TEXT[] 
);
