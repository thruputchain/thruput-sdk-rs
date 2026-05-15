use groundhog_contract_spec::{
    ContractTypeRefSpec, view::ContractTypeView,
};
use groundhog_xdr::{SyscallNo, syscalls::SysErr};

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
