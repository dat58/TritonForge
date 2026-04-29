//! Page-level route components for the application router.

pub mod groups;
pub mod home;
pub mod job_detail;
pub mod jobs;
mod timer;

pub use groups::GroupsPage;
pub use home::HomePage;
pub use job_detail::JobDetailPage;
pub use jobs::JobsPage;
