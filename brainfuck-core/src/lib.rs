pub const MAX_TAPE_SIZE: usize = 4;

mod data;
mod run;
mod search;
pub mod util;
pub use run::run_program_fragment_no_target;
