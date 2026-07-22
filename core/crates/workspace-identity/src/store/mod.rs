//! Five typed store sub-handles. All take `&PgPool` by reference
//! so they live no longer than the parent [`crate::Identity`].

mod invites;
mod members;
mod projects;
mod users;
mod visibility;

pub use invites::Invites;
pub use members::{Members, UserWorkspace};
pub use projects::Projects;
pub use users::Users;
pub use visibility::Visibility;
