Expose methods for cross-contract calls.

This macro may be applied to a trait, a trait implementation,
an inherent implementation, or a free function.

Usage **examples** are in the top-level [documentation](mod@crate).

#### Client object

If the attribute arguments `client = ClientTypeName` are given, a client-stub object will be
created with the given name.  This object will implement a method for calling each method of
the contract.

#### Contract exports

If applied to an implementation, trait methods (or public inherent methods or public free
functions) will be exported to receive foreign calls; these symbols will be placed in a module
named "exports", unless a macro argument like `exports = "export_mod"` is provided to
rename it.

#### Errors

A contract may call [`abort()`] with a custom error type if the contract macro
includes an attribute argument `throws = ErrorType` is included.

#### Storage

A method taking a `self` parameter will be called with a reference to the stored contract state
object, whose definition must be decorated with the [storage](macro@crate::storage) macro in
order to implement traits that allow it to be serialized and stored in the key-value store.

A method may be decorated with `#[init]` to assert that, when called, no contract state has yet
been stored; the `self` parameter will refer to a default value for the state object.  If the
first method parameter is `&mut self`, the value of the state object will be stored back to the
key-value store after the method completes successfully.
