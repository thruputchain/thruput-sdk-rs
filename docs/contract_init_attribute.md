NOTE: Although perhaps convenient, there is no burning need for `#[init]` at
the moment.  The erc20 contract shows an easy pattern for initialization.

---

The `#[contract]` macro accepts an attribute `#[init(..)]` on trait or impl
methods:
    - `#[init(once)]` means the method should be called at most once.  Note: it
        does not enforce that the initialization occurs before other methods
        are called.
    - The attribute can be placed only on implementations (inherent impls or
      trait impls).
