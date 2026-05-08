use syn::fold::Fold;
use syn::*;

struct StripOurAttributes;

impl Fold for StripOurAttributes {
    fn fold_attributes(&mut self, attrs: Vec<Attribute>) -> Vec<Attribute> {
        attrs.into_iter().filter(|a| !is_our_attribute(a)).collect()
    }
}

fn is_our_attribute(attr: &Attribute) -> bool {
    match &attr.meta {
        Meta::List(meta_list) => meta_list.path.is_ident("contract"),
        _ => false,
    }
}

pub trait Strip {
    fn strip_our_attributes(&self) -> Self;
}

impl Strip for ItemImpl {
    fn strip_our_attributes(&self) -> Self {
        StripOurAttributes.fold_item_impl(self.clone())
    }
}

impl Strip for ItemTrait {
    fn strip_our_attributes(&self) -> Self {
        StripOurAttributes.fold_item_trait(self.clone())
    }
}

impl Strip for ItemFn {
    fn strip_our_attributes(&self) -> Self {
        StripOurAttributes.fold_item_fn(self.clone())
    }
}
