//! `uncomment lint` — comment linting that never removes anything.
//!
//! The command this crate is named for deletes comments. This one does the opposite: it reads every
//! comment, checks that the ones claiming to track work actually name an issue, and reports. Without
//! `--fix` it writes nothing at all, which is what lets it run as a pre-commit hook and in CI beside
//! the removal hook rather than instead of it.
//!
//! Adoption on a large codebase is a first-class concern, because a tool that reports a thousand
//! pre-existing violations on day one gets disabled on day one. `--changed-only` narrows the run to
//! what a branch touched, and `--baseline` records what is already there — keyed by
//! [`crate::scan::id`], which excludes line and byte offsets precisely so that an unrelated edit
//! above a violating comment does not resurrect it.

pub mod config;
pub mod rules;

#[cfg(feature = "cli")]
mod command;
#[cfg(feature = "cli")]
pub use command::*;
