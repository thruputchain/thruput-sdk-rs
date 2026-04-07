//! Internal utilities.
//!
//! Some are public because used in proc-macros.
//!
//! *These interfaces are not intended for use by SDK clients, and may change at any time.*

#[allow(unused)]
mod mu_buf;
mod serialize;
mod static_buf;
mod track;

#[allow(unused_imports)]
pub use mu_buf::*;
pub use serialize::*;
pub use track::*;

/// Constant-time string equality check for use in `const` assertions.
pub const fn const_str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Concatenate two UTF-8 byte arrays at compile time (no allocation).
///
/// `LEN` must equal `N + M`. A third parameter is required because `[u8; N + M]` needs
/// `generic_const_exprs` on stable Rust; callers pass `LEN` explicitly (e.g. `{ N + M }`).
///
/// For `&str` inputs with known lengths, use [`const_str_concat`].
#[inline]
pub const fn const_str_concat_bytes<
    const N: usize,
    const M: usize,
    const LEN: usize,
>(
    a: &[u8; N],
    b: &[u8; M],
) -> [u8; LEN] {
    assert!(N + M == LEN);
    let mut out = [0u8; LEN];
    let mut i = 0;
    while i < N {
        out[i] = a[i];
        i += 1;
    }
    while i < LEN {
        out[i] = b[i - N];
        i += 1;
    }
    out
}

/// Concatenate two strings at compile time (no allocation).
///
/// `N` and `M` must match `a.len()` and `b.len()`; `LEN` must equal `N + M`.
/// To obtain a `&str`, use `core::str::from_utf8(&bytes)` in a `const` context
/// (valid UTF-8 is preserved when both inputs are `&str`).
#[inline]
pub const fn const_str_concat<
    const N: usize,
    const M: usize,
    const LEN: usize,
>(
    a: &str,
    b: &str,
) -> [u8; LEN] {
    assert!(a.len() == N && b.len() == M);
    assert!(N + M == LEN);
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let mut out = [0u8; LEN];
    let mut i = 0;
    while i < N {
        out[i] = ab[i];
        i += 1;
    }
    while i < LEN {
        out[i] = bb[i - N];
        i += 1;
    }
    out
}

/// Build a raw interface fragment record for the staging wasm custom section:
/// `iface\tpackage\tinterface\tversion\n`.
///
/// `OUT` must equal `9 + P + I + V`.
#[doc(hidden)]
#[inline]
pub const fn const_spec_raw_interface_fragment_bytes<
    const P: usize,
    const I: usize,
    const V: usize,
    const OUT: usize,
>(
    package_name: &str,
    interface_name: &str,
    interface_version: &str,
) -> [u8; OUT] {
    assert!(package_name.len() == P);
    assert!(interface_name.len() == I);
    assert!(interface_version.len() == V);
    assert!(OUT == 9 + P + I + V);

    let package = package_name.as_bytes();
    let iface = interface_name.as_bytes();
    let vers = interface_version.as_bytes();

    let mut out = [0u8; OUT];
    let mut i = 0usize;
    let mut j = 0usize;

    let prefix = b"iface\t";
    while j < prefix.len() {
        out[i] = prefix[j];
        i += 1;
        j += 1;
    }

    j = 0;
    while j < P {
        out[i] = package[j];
        i += 1;
        j += 1;
    }
    out[i] = b'\t';
    i += 1;

    j = 0;
    while j < I {
        out[i] = iface[j];
        i += 1;
        j += 1;
    }
    out[i] = b'\t';
    i += 1;

    j = 0;
    while j < V {
        out[i] = vers[j];
        i += 1;
        j += 1;
    }
    out[i] = b'\n';
    i += 1;

    assert!(i == OUT);
    out
}

const fn xdr_padding(len: usize) -> usize {
    let rem = len % 4;
    if rem == 0 { 0 } else { 4 - rem }
}

const fn xdr_string_len(value: &str) -> usize {
    4 + value.len() + xdr_padding(value.len())
}

const fn xdr_contract_type_len(
    ty: &crate::__contract_spec::ContractTypeView<'_>,
) -> usize {
    4 + match ty {
        crate::__contract_spec::ContractTypeView::Void
        | crate::__contract_spec::ContractTypeView::Bool
        | crate::__contract_spec::ContractTypeView::U32
        | crate::__contract_spec::ContractTypeView::I32
        | crate::__contract_spec::ContractTypeView::U64
        | crate::__contract_spec::ContractTypeView::I64
        | crate::__contract_spec::ContractTypeView::U128
        | crate::__contract_spec::ContractTypeView::I128
        | crate::__contract_spec::ContractTypeView::String
        | crate::__contract_spec::ContractTypeView::Bytes
        | crate::__contract_spec::ContractTypeView::Address => 0,
        crate::__contract_spec::ContractTypeView::Option(inner)
        | crate::__contract_spec::ContractTypeView::Vec(inner) => {
            xdr_contract_type_len(inner)
        }
        crate::__contract_spec::ContractTypeView::Tuple(items) => {
            xdr_contract_type_slice_len(items)
        }
        crate::__contract_spec::ContractTypeView::Named(symbol) => {
            xdr_string_len(symbol)
        }
    }
}

const fn xdr_contract_type_slice_len(
    types: &[crate::__contract_spec::ContractTypeView<'_>],
) -> usize {
    let mut len = 4;
    let mut i = 0;
    while i < types.len() {
        len += xdr_contract_type_len(&types[i]);
        i += 1;
    }
    len
}

const fn xdr_contract_field_len(
    field: &crate::__contract_spec::ContractFieldView<'_>,
) -> usize {
    xdr_string_len(field.name) + xdr_contract_type_len(&field.ty)
}

const fn xdr_contract_field_slice_len(
    fields: &[crate::__contract_spec::ContractFieldView<'_>],
) -> usize {
    let mut len = 4;
    let mut i = 0;
    while i < fields.len() {
        len += xdr_contract_field_len(&fields[i]);
        i += 1;
    }
    len
}

const fn xdr_contract_spec_method_len(
    method: &crate::__contract_spec::ContractSpecMethodView<'_>,
) -> usize {
    xdr_string_len(method.name)
        + xdr_contract_field_slice_len(method.args)
        + xdr_contract_type_len(&method.return_type)
}

const fn xdr_contract_enum_variant_len(
    variant: &crate::__contract_spec::ContractEnumVariantView<'_>,
) -> usize {
    xdr_string_len(variant.name) + 4
}

const fn xdr_contract_enum_variant_slice_len(
    variants: &[crate::__contract_spec::ContractEnumVariantView<'_>],
) -> usize {
    let mut len = 4;
    let mut i = 0;
    while i < variants.len() {
        len += xdr_contract_enum_variant_len(&variants[i]);
        i += 1;
    }
    len
}

const fn xdr_optional_contract_type_len(
    ty: &Option<crate::__contract_spec::ContractTypeView<'_>>,
) -> usize {
    4 + match ty {
        Some(inner) => xdr_contract_type_len(inner),
        None => 0,
    }
}

const fn xdr_contract_union_case_len(
    case: &crate::__contract_spec::ContractUnionCaseView<'_>,
) -> usize {
    xdr_string_len(case.name)
        + 4
        + xdr_optional_contract_type_len(&case.case_type)
}

const fn xdr_contract_union_case_slice_len(
    cases: &[crate::__contract_spec::ContractUnionCaseView<'_>],
) -> usize {
    let mut len = 4;
    let mut i = 0;
    while i < cases.len() {
        len += xdr_contract_union_case_len(&cases[i]);
        i += 1;
    }
    len
}

const fn xdr_contract_type_def_len(
    type_def: &crate::__contract_spec::ContractTypeDefView<'_>,
) -> usize {
    4 + match type_def {
        crate::__contract_spec::ContractTypeDefView::Struct(def) => {
            xdr_string_len(def.name) + xdr_contract_field_slice_len(def.fields)
        }
        crate::__contract_spec::ContractTypeDefView::Enum(def) => {
            xdr_string_len(def.name)
                + xdr_contract_enum_variant_slice_len(def.variants)
        }
        crate::__contract_spec::ContractTypeDefView::Union(def) => {
            xdr_string_len(def.name)
                + xdr_contract_union_case_slice_len(def.cases)
        }
    }
}

#[doc(hidden)]
pub const fn const_xdr_spec_interface_fragment_len(
    fragment: &crate::__contract_spec::SpecInterfaceFragmentView<'_>,
) -> usize {
    4 + xdr_string_len(fragment.package_name)
        + xdr_string_len(fragment.interface_name)
        + xdr_string_len(fragment.interface_version)
        + xdr_contract_type_slice_len(fragment.throws)
}

#[doc(hidden)]
pub const fn const_xdr_spec_method_fragment_len(
    fragment: &crate::__contract_spec::SpecMethodFragmentView<'_>,
) -> usize {
    4 + xdr_string_len(fragment.package_name)
        + xdr_string_len(fragment.interface_name)
        + xdr_string_len(fragment.interface_version)
        + xdr_contract_spec_method_len(&fragment.method)
}

#[doc(hidden)]
pub const fn const_xdr_spec_typedef_fragment_len(
    type_def: &crate::__contract_spec::ContractTypeDefView<'_>,
) -> usize {
    4 + xdr_contract_type_def_len(type_def)
}

const fn write_u32_be(out: &mut [u8], cursor: &mut usize, value: u32) {
    out[*cursor] = ((value >> 24) & 0xff) as u8;
    out[*cursor + 1] = ((value >> 16) & 0xff) as u8;
    out[*cursor + 2] = ((value >> 8) & 0xff) as u8;
    out[*cursor + 3] = (value & 0xff) as u8;
    *cursor += 4;
}

const fn write_i32_be(out: &mut [u8], cursor: &mut usize, value: i32) {
    write_u32_be(out, cursor, value as u32);
}

const fn write_bool(out: &mut [u8], cursor: &mut usize, value: bool) {
    write_u32_be(out, cursor, if value { 1 } else { 0 });
}

const fn write_bytes(out: &mut [u8], cursor: &mut usize, bytes: &[u8]) {
    let mut i = 0;
    while i < bytes.len() {
        out[*cursor] = bytes[i];
        *cursor += 1;
        i += 1;
    }
}

const fn write_padding(out: &mut [u8], cursor: &mut usize, len: usize) {
    let mut i = 0;
    let pad = xdr_padding(len);
    while i < pad {
        out[*cursor] = 0;
        *cursor += 1;
        i += 1;
    }
}

const fn write_string(out: &mut [u8], cursor: &mut usize, value: &str) {
    let bytes = value.as_bytes();
    write_u32_be(out, cursor, bytes.len() as u32);
    write_bytes(out, cursor, bytes);
    write_padding(out, cursor, bytes.len());
}

const fn write_contract_type(
    out: &mut [u8],
    cursor: &mut usize,
    ty: &crate::__contract_spec::ContractTypeView<'_>,
) {
    match ty {
        crate::__contract_spec::ContractTypeView::Void => {
            write_i32_be(out, cursor, 0);
        }
        crate::__contract_spec::ContractTypeView::Bool => {
            write_i32_be(out, cursor, 1);
        }
        crate::__contract_spec::ContractTypeView::U32 => {
            write_i32_be(out, cursor, 2);
        }
        crate::__contract_spec::ContractTypeView::I32 => {
            write_i32_be(out, cursor, 3);
        }
        crate::__contract_spec::ContractTypeView::U64 => {
            write_i32_be(out, cursor, 4);
        }
        crate::__contract_spec::ContractTypeView::I64 => {
            write_i32_be(out, cursor, 5);
        }
        crate::__contract_spec::ContractTypeView::U128 => {
            write_i32_be(out, cursor, 6);
        }
        crate::__contract_spec::ContractTypeView::I128 => {
            write_i32_be(out, cursor, 7);
        }
        crate::__contract_spec::ContractTypeView::String => {
            write_i32_be(out, cursor, 8);
        }
        crate::__contract_spec::ContractTypeView::Bytes => {
            write_i32_be(out, cursor, 9);
        }
        crate::__contract_spec::ContractTypeView::Address => {
            write_i32_be(out, cursor, 10);
        }
        crate::__contract_spec::ContractTypeView::Option(inner) => {
            write_i32_be(out, cursor, 11);
            write_contract_type(out, cursor, inner);
        }
        crate::__contract_spec::ContractTypeView::Vec(inner) => {
            write_i32_be(out, cursor, 12);
            write_contract_type(out, cursor, inner);
        }
        crate::__contract_spec::ContractTypeView::Tuple(items) => {
            write_i32_be(out, cursor, 13);
            write_contract_type_slice(out, cursor, items);
        }
        crate::__contract_spec::ContractTypeView::Named(symbol) => {
            write_i32_be(out, cursor, 14);
            write_string(out, cursor, symbol);
        }
    }
}

const fn write_contract_type_slice(
    out: &mut [u8],
    cursor: &mut usize,
    types: &[crate::__contract_spec::ContractTypeView<'_>],
) {
    write_u32_be(out, cursor, types.len() as u32);
    let mut i = 0;
    while i < types.len() {
        write_contract_type(out, cursor, &types[i]);
        i += 1;
    }
}

const fn write_contract_field(
    out: &mut [u8],
    cursor: &mut usize,
    field: &crate::__contract_spec::ContractFieldView<'_>,
) {
    write_string(out, cursor, field.name);
    write_contract_type(out, cursor, &field.ty);
}

const fn write_contract_field_slice(
    out: &mut [u8],
    cursor: &mut usize,
    fields: &[crate::__contract_spec::ContractFieldView<'_>],
) {
    write_u32_be(out, cursor, fields.len() as u32);
    let mut i = 0;
    while i < fields.len() {
        write_contract_field(out, cursor, &fields[i]);
        i += 1;
    }
}

const fn write_contract_spec_method(
    out: &mut [u8],
    cursor: &mut usize,
    method: &crate::__contract_spec::ContractSpecMethodView<'_>,
) {
    write_string(out, cursor, method.name);
    write_contract_field_slice(out, cursor, method.args);
    write_contract_type(out, cursor, &method.return_type);
}

const fn write_contract_enum_variant(
    out: &mut [u8],
    cursor: &mut usize,
    variant: &crate::__contract_spec::ContractEnumVariantView<'_>,
) {
    write_string(out, cursor, variant.name);
    write_i32_be(out, cursor, variant.value);
}

const fn write_contract_enum_variant_slice(
    out: &mut [u8],
    cursor: &mut usize,
    variants: &[crate::__contract_spec::ContractEnumVariantView<'_>],
) {
    write_u32_be(out, cursor, variants.len() as u32);
    let mut i = 0;
    while i < variants.len() {
        write_contract_enum_variant(out, cursor, &variants[i]);
        i += 1;
    }
}

const fn write_optional_contract_type(
    out: &mut [u8],
    cursor: &mut usize,
    ty: &Option<crate::__contract_spec::ContractTypeView<'_>>,
) {
    match ty {
        Some(inner) => {
            write_bool(out, cursor, true);
            write_contract_type(out, cursor, inner);
        }
        None => write_bool(out, cursor, false),
    }
}

const fn write_contract_union_case(
    out: &mut [u8],
    cursor: &mut usize,
    case: &crate::__contract_spec::ContractUnionCaseView<'_>,
) {
    write_string(out, cursor, case.name);
    write_i32_be(out, cursor, case.discriminant);
    write_optional_contract_type(out, cursor, &case.case_type);
}

const fn write_contract_union_case_slice(
    out: &mut [u8],
    cursor: &mut usize,
    cases: &[crate::__contract_spec::ContractUnionCaseView<'_>],
) {
    write_u32_be(out, cursor, cases.len() as u32);
    let mut i = 0;
    while i < cases.len() {
        write_contract_union_case(out, cursor, &cases[i]);
        i += 1;
    }
}

const fn write_contract_type_def(
    out: &mut [u8],
    cursor: &mut usize,
    type_def: &crate::__contract_spec::ContractTypeDefView<'_>,
) {
    match type_def {
        crate::__contract_spec::ContractTypeDefView::Struct(def) => {
            write_i32_be(out, cursor, 0);
            write_string(out, cursor, def.name);
            write_contract_field_slice(out, cursor, def.fields);
        }
        crate::__contract_spec::ContractTypeDefView::Enum(def) => {
            write_i32_be(out, cursor, 1);
            write_string(out, cursor, def.name);
            write_contract_enum_variant_slice(out, cursor, def.variants);
        }
        crate::__contract_spec::ContractTypeDefView::Union(def) => {
            write_i32_be(out, cursor, 2);
            write_string(out, cursor, def.name);
            write_contract_union_case_slice(out, cursor, def.cases);
        }
    }
}

#[doc(hidden)]
#[inline]
pub const fn const_xdr_spec_interface_fragment_bytes<const OUT: usize>(
    fragment: &crate::__contract_spec::SpecInterfaceFragmentView<'_>,
) -> [u8; OUT] {
    assert!(OUT == const_xdr_spec_interface_fragment_len(fragment));
    let mut out = [0u8; OUT];
    let mut cursor = 0;

    write_i32_be(&mut out, &mut cursor, 0);
    write_string(&mut out, &mut cursor, fragment.package_name);
    write_string(&mut out, &mut cursor, fragment.interface_name);
    write_string(&mut out, &mut cursor, fragment.interface_version);
    write_contract_type_slice(&mut out, &mut cursor, fragment.throws);

    assert!(cursor == OUT);
    out
}

#[doc(hidden)]
#[inline]
pub const fn const_xdr_spec_method_fragment_bytes<const OUT: usize>(
    fragment: &crate::__contract_spec::SpecMethodFragmentView<'_>,
) -> [u8; OUT] {
    assert!(OUT == const_xdr_spec_method_fragment_len(fragment));
    let mut out = [0u8; OUT];
    let mut cursor = 0;

    write_i32_be(&mut out, &mut cursor, 1);
    write_string(&mut out, &mut cursor, fragment.package_name);
    write_string(&mut out, &mut cursor, fragment.interface_name);
    write_string(&mut out, &mut cursor, fragment.interface_version);
    write_contract_spec_method(&mut out, &mut cursor, &fragment.method);

    assert!(cursor == OUT);
    out
}

#[doc(hidden)]
#[inline]
pub const fn const_xdr_spec_typedef_fragment_bytes<const OUT: usize>(
    type_def: &crate::__contract_spec::ContractTypeDefView<'_>,
) -> [u8; OUT] {
    assert!(OUT == const_xdr_spec_typedef_fragment_len(type_def));
    let mut out = [0u8; OUT];
    let mut cursor = 0;

    write_i32_be(&mut out, &mut cursor, 2);
    write_contract_type_def(&mut out, &mut cursor, type_def);

    assert!(cursor == OUT);
    out
}

/// Build the full version-mismatch diagnostic bytes: `prefix` + `'major', from 'full'` + `suffix`.
///
/// `OUT` must equal `P + 11 + M + F + S` (the middle segment is always 11 + major + full bytes).
/// For use from generated `#[contract]` code only.
#[doc(hidden)]
#[inline]
pub const fn const_iface_version_mismatch_diagnostic_bytes<
    const P: usize,
    const M: usize,
    const F: usize,
    const S: usize,
    const OUT: usize,
>(
    prefix: &str,
    trait_major: &str,
    trait_full: &str,
    suffix: &[u8; S],
) -> [u8; OUT] {
    assert!(
        prefix.len() == P && trait_major.len() == M && trait_full.len() == F
    );
    assert!(OUT == P + 11 + M + F + S);
    let pb = prefix.as_bytes();
    let ab = trait_major.as_bytes();
    let fb = trait_full.as_bytes();
    let mut out = [0u8; OUT];
    let mut i = 0usize;

    let mut j = 0usize;
    while j < P {
        out[i] = pb[j];
        i += 1;
        j += 1;
    }

    out[i] = b'\'';
    i += 1;
    j = 0;
    while j < M {
        out[i] = ab[j];
        i += 1;
        j += 1;
    }
    out[i] = b'\'';
    i += 1;

    let mid = b", from ";
    j = 0;
    while j < 7 {
        out[i] = mid[j];
        i += 1;
        j += 1;
    }

    out[i] = b'\'';
    i += 1;
    j = 0;
    while j < F {
        out[i] = fb[j];
        i += 1;
        j += 1;
    }
    out[i] = b'\'';
    i += 1;

    j = 0;
    while j < S {
        out[i] = suffix[j];
        i += 1;
        j += 1;
    }

    assert!(i == OUT);
    out
}

/// Build the full interface-name-mismatch diagnostic bytes:
/// `prefix` + `'impl' but the trait defines 'trait'` + `suffix`.
///
/// `OUT` must equal `P + 27 + I + T + S`.
/// For use from generated `#[contract]` code only.
#[doc(hidden)]
#[inline]
pub const fn const_iface_name_mismatch_diagnostic_bytes<
    const P: usize,
    const I: usize,
    const T: usize,
    const S: usize,
    const OUT: usize,
>(
    prefix: &str,
    impl_name: &str,
    trait_name: &str,
    suffix: &[u8; S],
) -> [u8; OUT] {
    assert!(
        prefix.len() == P && impl_name.len() == I && trait_name.len() == T
    );
    assert!(OUT == P + 27 + I + T + S);
    let pb = prefix.as_bytes();
    let ib = impl_name.as_bytes();
    let tb = trait_name.as_bytes();
    let mut out = [0u8; OUT];
    let mut i = 0usize;

    let mut j = 0usize;
    while j < P {
        out[i] = pb[j];
        i += 1;
        j += 1;
    }

    out[i] = b'\'';
    i += 1;
    j = 0;
    while j < I {
        out[i] = ib[j];
        i += 1;
        j += 1;
    }
    out[i] = b'\'';
    i += 1;

    let mid = b" but the trait defines ";
    j = 0;
    while j < 23 {
        out[i] = mid[j];
        i += 1;
        j += 1;
    }

    out[i] = b'\'';
    i += 1;
    j = 0;
    while j < T {
        out[i] = tb[j];
        i += 1;
        j += 1;
    }
    out[i] = b'\'';
    i += 1;

    j = 0;
    while j < S {
        out[i] = suffix[j];
        i += 1;
        j += 1;
    }

    assert!(i == OUT);
    out
}

#[cfg(test)]
mod iface_version_mismatch_diagnostic_tests {
    use super::{
        const_iface_name_mismatch_diagnostic_bytes,
        const_iface_version_mismatch_diagnostic_bytes,
    };

    #[test]
    fn const_iface_version_mismatch_diagnostic_bytes_matches_expected_text() {
        const PREFIX: &str = "a: ";
        const MAJ: &str = "0";
        const FULL: &str = "0.0";
        const SUFFIX: &[u8; 2] = b" x";
        const OUT: [u8; 20] =
            const_iface_version_mismatch_diagnostic_bytes::<3, 1, 3, 2, 20>(
                PREFIX, MAJ, FULL, SUFFIX,
            );
        assert_eq!(
            core::str::from_utf8(&OUT).unwrap(),
            "a: '0', from '0.0' x"
        );
    }

    #[test]
    fn const_iface_name_mismatch_diagnostic_bytes_matches_expected_text() {
        const PREFIX: &str = "a: ";
        const IMPL: &str = "impl";
        const TRAIT: &str = "iface";
        const SUFFIX: &[u8; 2] = b" x";
        const OUT: [u8; 41] =
            const_iface_name_mismatch_diagnostic_bytes::<3, 4, 5, 2, 41>(
                PREFIX, IMPL, TRAIT, SUFFIX,
            );
        assert_eq!(
            core::str::from_utf8(&OUT).unwrap(),
            "a: 'impl' but the trait defines 'iface' x"
        );
    }
}

#[cfg(test)]
mod spec_fragment_xdr_tests {
    use super::{
        const_xdr_spec_interface_fragment_bytes,
        const_xdr_spec_interface_fragment_len,
        const_xdr_spec_method_fragment_bytes,
        const_xdr_spec_method_fragment_len,
        const_xdr_spec_typedef_fragment_bytes,
        const_xdr_spec_typedef_fragment_len,
    };
    use crate::__contract_spec::{
        ContractEnumDef, ContractEnumVariant, ContractField,
        ContractSpecMethod, ContractType, ContractTypeDef,
        ContractTypeDefView, ContractTypeView, SpecFragment,
        SpecInterfaceFragment, SpecInterfaceFragmentView, SpecMethodFragment,
        SpecMethodFragmentView,
    };
    use xdrs::txs::encode;

    #[test]
    fn const_interface_fragment_encoder_matches_xdrs() {
        const THROWS: &[ContractTypeView<'static>] =
            &[ContractTypeView::Named("DemoError")];
        const VIEW: SpecInterfaceFragmentView<'static> =
            SpecInterfaceFragmentView {
                package_name: "demo",
                interface_name: "Named",
                interface_version: "0.1.0",
                throws: THROWS,
            };
        const LEN: usize = const_xdr_spec_interface_fragment_len(&VIEW);
        const ENCODED: [u8; LEN] =
            const_xdr_spec_interface_fragment_bytes::<LEN>(&VIEW);

        let expected =
            encode(&SpecFragment::Interface(SpecInterfaceFragment {
                package_name: "demo".to_string(),
                interface_name: "Named".to_string(),
                interface_version: "0.1.0".to_string(),
                throws: vec![ContractType::Named("DemoError".to_string())],
            }))
            .unwrap();

        assert_eq!(ENCODED.as_slice(), expected.as_slice());
    }

    #[test]
    fn const_method_fragment_encoder_matches_xdrs() {
        const TUPLE_ITEMS: &[ContractTypeView<'static>] =
            &[ContractTypeView::U32, ContractTypeView::Named("Payload")];
        const ARGS: &[crate::__contract_spec::ContractFieldView<'static>] = &[
            crate::__contract_spec::ContractFieldView {
                name: "value",
                ty: ContractTypeView::U32,
            },
            crate::__contract_spec::ContractFieldView {
                name: "payload",
                ty: ContractTypeView::Named("Payload"),
            },
        ];
        const VIEW: SpecMethodFragmentView<'static> = SpecMethodFragmentView {
            package_name: "demo",
            interface_name: "Named",
            interface_version: "0.1.0",
            method: crate::__contract_spec::ContractSpecMethodView {
                name: "ping",
                args: ARGS,
                return_type: ContractTypeView::Tuple(TUPLE_ITEMS),
            },
        };
        const LEN: usize = const_xdr_spec_method_fragment_len(&VIEW);
        const ENCODED: [u8; LEN] =
            const_xdr_spec_method_fragment_bytes::<LEN>(&VIEW);

        let expected = encode(&SpecFragment::Method(SpecMethodFragment {
            package_name: "demo".to_string(),
            interface_name: "Named".to_string(),
            interface_version: "0.1.0".to_string(),
            method: ContractSpecMethod {
                name: "ping".to_string(),
                args: vec![
                    ContractField {
                        name: "value".to_string(),
                        ty: ContractType::U32,
                    },
                    ContractField {
                        name: "payload".to_string(),
                        ty: ContractType::Named("Payload".to_string()),
                    },
                ],
                return_type: ContractType::Tuple(vec![
                    ContractType::U32,
                    ContractType::Named("Payload".to_string()),
                ]),
            },
        }))
        .unwrap();

        assert_eq!(ENCODED.as_slice(), expected.as_slice());
    }

    #[test]
    fn const_typedef_fragment_encoder_matches_xdrs() {
        const VIEW: ContractTypeDefView<'static> = ContractTypeDefView::Enum(
            crate::__contract_spec::ContractEnumDefView {
                name: "DemoError",
                variants: &[crate::__contract_spec::ContractEnumVariantView {
                    name: "Nope",
                    value: 7,
                }],
            },
        );
        const LEN: usize = const_xdr_spec_typedef_fragment_len(&VIEW);
        const ENCODED: [u8; LEN] =
            const_xdr_spec_typedef_fragment_bytes::<LEN>(&VIEW);

        let expected = encode(&SpecFragment::TypeDef(ContractTypeDef::Enum(
            ContractEnumDef {
                name: "DemoError".to_string(),
                variants: vec![ContractEnumVariant {
                    name: "Nope".to_string(),
                    value: 7,
                }],
            },
        )))
        .unwrap();

        assert_eq!(ENCODED.as_slice(), expected.as_slice());
    }

    #[test]
    fn contract_type_ref_spec_supports_tuples() {
        assert!(matches!(
            crate::__contract_spec::type_ref::<(
                u32,
                u64,
                crate::ContractAddress,
            )>(),
            ContractTypeView::Tuple(items)
                if matches!(
                    items,
                    [
                        ContractTypeView::U32,
                        ContractTypeView::U64,
                        ContractTypeView::Address
                    ]
                )
        ));
    }
}
