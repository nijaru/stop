pub mod inspect;
pub mod list;
pub mod top;

/// Command outcomes mapped to process exit codes by `main`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    NoMatch,
    Ambiguous,
}

impl Outcome {
    pub fn exit_code(self) -> i32 {
        match self {
            Outcome::Success => 0,
            Outcome::NoMatch => 2,
            Outcome::Ambiguous => 3,
        }
    }
}
