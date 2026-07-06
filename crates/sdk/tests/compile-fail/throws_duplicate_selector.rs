use sdk::*;

// Explicitly re-declaring the SDK's own `LibraryExn` in a `throws(...)` list
// doesn't collide by variant name (its last-path-segment variant is
// `LibraryExn`, not the reserved `Library`), so the name-based
// duplicate/reserved checks let it through — but it aliases the same
// selector as the implicit `Library` arm every generated client already
// carries. The compile-time selector-uniqueness assert should catch this.
#[contract(throws(sdk::exn::LibraryExn))]
pub trait ThrowsDuplicateSelectorTrait {
    fn ping();
}

fn main() {}
