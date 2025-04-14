use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::Ident;
use syn::Result;

pub fn generate(input: &syn::DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    let generics = &input.generics;

    let mut output = TokenStream::new();

    output.extend(match &input.data {
        syn::Data::Struct(ref item) => {
            generate_for_struct(ident, generics, item)?
        }
        _ => err(input, "Can only derive Storage for structs")?,
    });

    output.extend(generate_load_keypath_impl(ident, generics)?);

    let ident_lower =
        Ident::new(&ident.to_string().to_lowercase(), ident.span());
    let impl_mod = format_ident!("_{}_storage_impls", ident_lower);

    Ok(quote! {

        mod #impl_mod {
            use super::*;

            #output
        }

    })
}

fn generate_for_struct(
    ident: &syn::Ident,
    generics: &syn::Generics,
    item: &syn::DataStruct,
) -> Result<TokenStream> {
    let ty = generic_type_name(ident, generics);
    let fields = &item.fields;

    let deserialize_impl = generate_deserialize_impl(ident, generics, fields)?;
    let default_impl = generate_default_impl(ident, generics, fields)?;

    Ok(quote! {

        use ::core::marker::PhantomData;
        use ::core::fmt;
        use ::serde::de::{Deserialize, DeserializeSeed, SeqAccess, Visitor};
        use ::serde::Deserializer;
        use sdk::storage::{Keypath, KeypathSeed, LoadKeypath, KeypathSeeded, DefaultFromKeypath};
        use sdk::bytestring::bytestring_get;
        use sdk::serialize;

        pub struct _KeypathSeed<T> {
            keypath: Keypath,
            marker: PhantomData<T>,
        }

        impl<T> _KeypathSeed<T> {
            pub fn new(keypath: &Keypath) -> Self {
                Self {
                    keypath: keypath.clone(),
                    marker: PhantomData,
                }
            }
        }

        impl KeypathSeeded for #ty {
            type Seed = _KeypathSeed<#ty>;

            fn seed(keypath: &Keypath) -> Self::Seed {
                _KeypathSeed::new(keypath)
            }
        }

        #deserialize_impl

        #default_impl
    })
}

fn generate_deserialize_impl(
    ident: &syn::Ident,
    generics: &syn::Generics,
    fields: &syn::Fields,
) -> Result<TokenStream> {
    let ty = generic_type_name(ident, generics);
    let generic_params = &generics.params;

    let fields = if let syn::Fields::Named(ref fields_named) = fields {
        &fields_named.named
    } else {
        return err(fields, "Can only handle named struct fields");
    };

    fn visit_element(field: &syn::Field) -> Result<TokenStream> {
        let ident = &field.ident;
        let ty = &field.ty;
        Ok(quote! {

            let seed = <#ty as KeypathSeeded>::seed(self.keypath);
            let #ident = seq.next_element_seed(seed)?.unwrap();
            self.keypath.seq_next();

        })
    }

    let n_fields = fields.len();
    let mut visit_field_names = TokenStream::new();
    let mut visit_elements = TokenStream::new();
    for field in fields.iter() {
        let ident = &field.ident;
        visit_field_names.extend(quote! { #ident, });

        visit_elements.extend(visit_element(field)?);
    }

    Ok(quote! {

        impl<'de, #generic_params> DeserializeSeed<'de> for _KeypathSeed<#ty> {

            type Value = #ty;

            fn deserialize<D>(self, deserializer: D) -> ::core::result::Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct KeypathVisitor<'a> {
                    keypath: &'a mut Keypath,
                }

                impl<'de> Visitor<'de> for KeypathVisitor<'_> {
                    type Value = #ident #generic_params;

                    fn expecting(
                        &self,
                        formatter: &mut fmt::Formatter,
                    ) -> fmt::Result {
                        write!(formatter, "a #ident")
                    }

                    fn visit_seq<A>(self, mut seq: A) -> ::core::result::Result<#ident #generic_params, A::Error>
                    where
                        A: SeqAccess<'de>,
                    {
                        self.keypath.enter_seq();
                        #visit_elements
                        self.keypath.leave_seq();

                        Ok(#ident { #visit_field_names })
                    }
                }

                let mut keypath = self.keypath.clone();
                let visitor = KeypathVisitor {
                    keypath: &mut keypath,
                };
                deserializer.deserialize_tuple(#n_fields, visitor)
            }
        }

    })
}

fn generate_default_impl(
    ident: &syn::Ident,
    generics: &syn::Generics,
    fields: &syn::Fields,
) -> Result<TokenStream> {
    let ty = generic_type_name(ident, generics);
    let generic_params = &generics.params;

    let fields = if let syn::Fields::Named(ref fields_named) = fields {
        &fields_named.named
    } else {
        return err(fields, "Can only handle named struct fields");
    };

    let mut field_names = TokenStream::new();
    let mut field_initializers = TokenStream::new();
    for field in fields.iter() {
        let field_name = &field.ident;
        field_names.extend(quote! { #field_name, });

        field_initializers.extend(quote!{
            let #field_name = DefaultFromKeypath::default_from_keypath(&keypath);
            keypath.seq_next();
        });
    }

    Ok(quote! {
        impl<#generic_params> DefaultFromKeypath for #ty {
            fn default_from_keypath(keypath: &Keypath) -> Self {
                let mut keypath = keypath.clone();

                keypath.enter_seq();

                #field_initializers

                keypath.leave_seq();

                Self {
                    #field_names
                }
            }
        }
    })
}

fn generate_load_keypath_impl(
    ident: &syn::Ident,
    generics: &syn::Generics,
) -> Result<TokenStream> {
    let ty = generic_type_name(ident, generics);
    let generic_params = &generics.params;

    Ok(quote! {

        impl #generic_params LoadKeypath for #ty {
            fn load_keypath(keypath: &Keypath) -> sdk::result::Result<Self> {
                let key = keypath.generate_key();
                let buf = bytestring_get(&key)?;

                let seed = _KeypathSeed::<#ty>::new(keypath);
                let value = serialize::from_bytes_seed(seed, &buf)?;

                Ok(value)
            }
        }

    })
}

fn generic_type_name(
    ident: &syn::Ident,
    generics: &syn::Generics,
) -> TokenStream {
    let generic_params = &generics.params;

    quote!(#ident #generic_params)
}

// Produce an error referencing a particular code span
fn err<T, S, M>(spanned: &S, msg: M) -> Result<T>
where
    S: Spanned,
    M: AsRef<str>,
{
    Err(syn::Error::new(spanned.span(), msg.as_ref()))
}
