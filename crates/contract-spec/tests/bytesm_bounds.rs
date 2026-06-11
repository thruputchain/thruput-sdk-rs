//! `BytesM<MAX>` must publish its length bound in the spec, like `VecM`.

use groundhog_contract_spec::{ContractTypeRefSpec, view::ContractTypeView};
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
