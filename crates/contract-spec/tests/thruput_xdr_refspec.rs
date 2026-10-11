use thruput_contract_spec::{ContractTypeRefSpec, view::ContractTypeView};
use thruput_xdr::{SyscallNo, syserr::SysErr};

#[test]
fn syscallno_refspec_is_u64() {
    assert!(matches!(
        <SyscallNo as ContractTypeRefSpec>::INFO.ty,
        ContractTypeView::U64
    ));
}

#[test]
fn syserr_refspec_is_i32() {
    assert!(matches!(
        <SysErr as ContractTypeRefSpec>::INFO.ty,
        ContractTypeView::I32
    ));
}
