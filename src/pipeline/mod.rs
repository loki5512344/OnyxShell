pub mod parse;
pub mod exec;

pub use parse::parse;
pub use exec::execute;

pub const MAX_SEGMENTS: usize = 8;
pub const MAX_ARGS_PER: usize = 16;

#[derive(Copy, Clone)]
pub struct Segment {
    pub args: [(usize, usize); MAX_ARGS_PER],
    pub n_args: usize,
}

pub struct Pipeline {
    pub segments: [Segment; MAX_SEGMENTS],
    pub n_segments: usize,
    pub stdout_file: (usize, usize),
    pub stdin_file: (usize, usize),
    pub background: bool,
}
