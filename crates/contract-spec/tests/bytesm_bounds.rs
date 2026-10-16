//! `BytesM<MAX>` and `heapless::Vec<T, N>` must publish their length bound
//! in the spec, like `VecM`.

use thruput_contract_spec::{ContractTypeRefSpec, view::ContractTypeView};
use xdr_common::BytesM;

#[test]
fn bytesm_refspec_preserves_max_len() {
    match <BytesM<256> as ContractTypeRefSpec>::INFO.ty {
        ContractTypeView::Vector(vector) => {
            assert!(matches!(vector.element, ContractTypeView::U8));
            assert_eq!(vector.max_len, 256);
        }
        other => panic!("expected Vector(U8, 256), got {other:?}"),
    }
}

#[test]
fn unbounded_bytesm_refspec_keeps_default_bound() {
    match <BytesM as ContractTypeRefSpec>::INFO.ty {
        ContractTypeView::Vector(vector) => {
            assert!(matches!(vector.element, ContractTypeView::U8));
            assert_eq!(vector.max_len, u32::MAX);
        }
        other => panic!("expected Vector(U8, u32::MAX), got {other:?}"),
    }
}

#[test]
fn heapless_bytes_refspec_preserves_bound() {
    match <heapless::Vec<u8, 64> as ContractTypeRefSpec>::INFO.ty {
        ContractTypeView::Vector(vector) => {
            assert!(matches!(vector.element, ContractTypeView::U8));
            assert_eq!(vector.max_len, 64);
        }
        other => panic!("expected Vector(U8, 64), got {other:?}"),
    }
}

#[test]
fn heapless_vec_refspec_carries_element_and_bound() {
    match <heapless::Vec<[u8; 8], 32> as ContractTypeRefSpec>::INFO.ty {
        ContractTypeView::Vector(vector) => {
            match vector.element {
                ContractTypeView::Array(array) => {
                    assert!(matches!(array.element, ContractTypeView::U8));
                    assert_eq!(array.len, 8);
                }
                other => panic!("expected Array(U8, 8), got {other:?}"),
            }
            assert_eq!(vector.max_len, 32);
        }
        other => panic!("expected Vector(Array(U8, 8), 32), got {other:?}"),
    }
}
