//! Shell features: globbing, history, tab-completion, env, jobs.

pub mod buffer;
pub mod env;
pub mod history;
pub mod service;

pub use env::{
    build_envp, env_get, env_init, env_list, env_set, env_unset, expand_tilde, expand_vars,
    ENV_KEY_MAX, ENV_MAX, ENV_VAL_MAX,
};
pub use history::{
    history_expand, history_get, history_last, history_push, nav_down, nav_reset, nav_up,
    HISTORY_LINE_MAX, HISTORY_SIZE,
};
pub use service::glob::{glob_expand, glob_match, has_glob};
pub use service::jobs::{
    job_add, job_count, job_find_by_id, job_get_by_index, job_list, job_reap, job_remove_by_id,
    job_set_running, JOB_MAX,
};
pub use service::{tab_complete, TabResult};
