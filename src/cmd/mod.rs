pub mod inspect;
pub mod query;
pub mod resolve;
pub mod sample;
pub mod tree;

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Success,
    NoMatch,
}
impl Outcome {
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::NoMatch => 2,
        }
    }
}
