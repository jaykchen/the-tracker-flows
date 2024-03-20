CREATE TABLE projects (
    project_id VARCHAR(255) PRIMARY KEY,  -- url of a project repo
    project_logo VARCHAR(255) ,
    issues_list JSON
);

CREATE TABLE issues (
    issue_id VARCHAR(255) PRIMARY KEY,  -- url of an issue
    project_id VARCHAR(255) NOT NULL,
    issue_title VARCHAR(255) NOT NULL,
    issue_description TEXT NOT NULL,  -- description of the issue, could be truncated body text
    issue_budget INT,
    issue_assignee VARCHAR(255),
    issue_linked_pr VARCHAR(255),    -- url of the pull_request that closed the issue, if any, or the pull_request that is linked to the issue
    issue_status VARCHAR(255),    -- open, closed, in progress, or some signal of the status identified by the bot summarizing the issue's comments
    review_status ENUM('queue', 'approve', 'decline'),
    issue_budget_approved BOOLEAN
);

CREATE TABLE comments (
    comment_id VARCHAR(255) PRIMARY KEY,
    issue_id VARCHAR(255) NOT NULL,
    creator VARCHAR(50) NOT NULL,
    time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    content TEXT 
);

CREATE TABLE pull_requests (
    pull_id VARCHAR(255) PRIMARY KEY,  -- url of pull_request
    title VARCHAR(255) NOT NULL,
    author VARCHAR(50) ,
    repository VARCHAR(255) NOT NULL,
    merged_by VARCHAR(50) ,
    cross_referenced_issues JSON, 
    connected_issues JSON
);