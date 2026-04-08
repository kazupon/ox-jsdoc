// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteKind {
    Single,
    Double,
    Backtick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FenceState {
    pub tick_count: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkpoint {
    pub offset: u32,
    pub brace_depth: u16,
    pub bracket_depth: u16,
    pub paren_depth: u16,
    pub quote: Option<QuoteKind>,
    pub fence: Option<FenceState>,
    pub diagnostics_len: usize,
}
