pub mod model;

pub use model::{InterfaceId, RawInterfaceFragment, SpecToolError};

pub fn canonicalize_interfaces(
    raw: &[RawInterfaceFragment],
) -> Result<Vec<InterfaceId>, SpecToolError> {
    if raw.is_empty() {
        return Err(SpecToolError::NoInterfaceFragments);
    }

    let mut impl_self_type: Option<&str> = None;
    let mut interfaces = Vec::with_capacity(raw.len());

    for fragment in raw {
        if fragment.interface_name.is_empty() || fragment.interface_version.is_empty() {
            return Err(SpecToolError::InvalidRawFragment(
                "interface fragments must include name and version".to_string(),
            ));
        }

        if let Some(self_type_key) = fragment.self_type_key.as_deref() {
            match impl_self_type {
                Some(existing) if existing != self_type_key => {
                    return Err(SpecToolError::MultipleImplTypes(
                        existing.to_string(),
                        self_type_key.to_string(),
                    ));
                }
                None => impl_self_type = Some(self_type_key),
                Some(_) => {}
            }
        }

        interfaces.push(InterfaceId::new(
            fragment.interface_name.clone(),
            fragment.interface_version.clone(),
        ));
    }

    interfaces.sort();
    interfaces.dedup();
    Ok(interfaces)
}
