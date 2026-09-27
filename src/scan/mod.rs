//! The comment inventory: identifying comments stably enough to hand out and take back.
//!
//! `scan` emits an inventory, something else filters it, and `keep` reads the filtered list back to
//! mark the comments that survived. [`id`] supplies the identifiers that link the two runs.

pub mod command;
pub mod id;
