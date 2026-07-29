use super::*;

pub const NETWORK_MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    statements: &[
        "CREATE TABLE ir_accounts (
            provider TEXT NOT NULL,
            account_id TEXT NOT NULL,
            account_display_name TEXT NOT NULL DEFAULT '',
            role TEXT NOT NULL DEFAULT 'submit_only',
            enabled INTEGER NOT NULL DEFAULT 1,
            last_login_at INTEGER,
            last_success_at INTEGER,
            PRIMARY KEY(provider, account_id)
        );",
        "CREATE TABLE ir_score_jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            provider TEXT NOT NULL,
            account_id TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT 'score',
            local_score_id INTEGER NOT NULL,
            chart_sha256 TEXT NOT NULL,
            ln_policy TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            attempt_count INTEGER NOT NULL DEFAULT 0,
            next_attempt_at INTEGER NOT NULL DEFAULT 0,
            last_error TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            UNIQUE(provider, account_id, kind, local_score_id)
        );",
        "CREATE INDEX idx_ir_score_jobs_status_next_attempt
            ON ir_score_jobs(status, next_attempt_at);",
        "CREATE INDEX idx_ir_score_jobs_local_score
            ON ir_score_jobs(kind, local_score_id);",
        "CREATE TABLE ir_score_submissions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            job_id INTEGER NOT NULL,
            provider TEXT NOT NULL,
            account_id TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT 'score',
            local_score_id INTEGER NOT NULL,
            remote_score_id TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL,
            submitted_at INTEGER NOT NULL,
            log_path TEXT NOT NULL DEFAULT '',
            error TEXT NOT NULL DEFAULT '',
            FOREIGN KEY(job_id) REFERENCES ir_score_jobs(id) ON DELETE CASCADE
        );",
        "CREATE INDEX idx_ir_score_submissions_local_score
            ON ir_score_submissions(kind, local_score_id);",
        "CREATE INDEX idx_ir_score_submissions_submitted_at
            ON ir_score_submissions(submitted_at);",
    ],
}];
