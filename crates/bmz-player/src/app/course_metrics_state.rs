use super::*;

pub(super) struct PendingCourseMetrics {
    pub(super) generation: u64,
    pub(super) course_id: i64,
    pub(super) first_chart_id: i64,
    pub(super) requested_at: Instant,
    pub(super) update_ln_policy: bool,
    pub(super) worker_started_at: Option<Instant>,
    pub(super) rx: Option<Receiver<std::result::Result<CoursePlayMetrics, String>>>,
}
