Expose methods for cross-contract calls.

This macro may be applied to a trait, a trait implementation,
an inherent implementation, or a free function.

Usage **examples** are in the top-level [documentation](mod@crate).

#### Client object

If the attribute argument `client = Client` is given, a client-stub type will
be created with the given name. This type implements a method for calling each
method of the contract. The type also implements
[`ContractClient`](crate::ContractClient), and an instance can be created by
passing a contract address to `Client::new()`.

#### Contract exports

If the `contract` macro is applied to an implementation, its trait methods (or
public inherent methods, or public free functions) will wrapped and exported to
receive foreign calls.  The symbols will be placed in submodule named
"exports", unless a macro argument like `exports = "export_mod"` is provided to
rename it.

#### Errors

If the `contract` macro includes the argument `throws = ErrorType`, the
contract implementation may pass values of that type to [`throw`].

The `throws` argument may be used with a trait or an inherent implementation,
but not with a trait implementation: the annotation is part of the contract
interface, so it must instead be placed on the trait being implemented.

#### Storage

A method taking a `self` parameter will be called with a reference to the
contract state object that has previously been stored at a well-known key.  The
definition of the state type should be decorated with the
[storage](macro@crate::storage) macro in order to implement traits that allow
it to be serialized and stored in the key-value store.

If the method receiver is `&mut self`, the value of the state object will be
stored back to the key-value store after the method completes successfully.

#### Endpoint identifier customization

Each exported contract entrypoint is named with an identifier derived from the
name of the interface (usually the trait name), a version, and the name of the
method.

The `contract` macro (applied to a trait, trait implementation, inherent
implementation, or free function) may include the argument `interface =
"InterfaceName"` to specify the interface name to be used in constructing
method endpoint identifiers.

Further, the argument `version = "0.3.0"` gives an explicit version for
endpoint identifiers; the default is the version of the current cargo package.
