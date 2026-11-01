//! The decision loop: the hands that carry a short task out on a view with a decision model
//! alone. It works with any view (`crate::view::View`) and any decision model
//! (`crate::model::Decider`), and no particular one. [`decision_loop::DecisionLoop`] implements
//! `crate::hands::Hands`: it resumes [`goal::GoalRun`] until the task is done; [`ask`] builds
//! each step's question.

mod ask;
mod choice;
pub mod commit;
pub mod decision_loop;
mod heads;
mod offer;
mod run;
mod settle;
pub mod step;
