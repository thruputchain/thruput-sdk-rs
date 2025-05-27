use std::{fs, path::PathBuf};
use syn::{Item, ItemTrait};
use quote::quote;
use syn::{Type, TypeReference};

fn strip_reference(ty: &Type) -> &Type {
    if let Type::Reference(TypeReference { elem, .. }) = ty {
        elem.as_ref()
    } else {
        ty
    }
}

use syn::{TypePath};

pub fn to_cpp_type(ty: &Type) -> String {
    match ty {
        Type::Path(TypePath { path, .. }) => {
            let ident = path.segments.last().unwrap().ident.to_string();

            match ident.as_str() {
                "i8"    => "int8_t".to_string(),
                "u8"    => "uint8_t".to_string(),
                "i16"   => "int16_t".to_string(),
                "u16"   => "uint16_t".to_string(),
                "i32"   => "int32_t".to_string(),
                "u32"   => "uint32_t".to_string(),
                "i64"   => "int64_t".to_string(),
                "u64"   => "uint64_t".to_string(),
                "isize" => "intptr_t".to_string(),
                "usize" => "uintptr_t".to_string(),

                "f32"   => "float".to_string(),
                "f64"   => "double".to_string(),

                "bool"  => "bool".to_string(),
                "char"  => "char".to_string(),

                _ => ident, // fallback: use the same name (assumes struct or opaque type)
            }
        }

        Type::Tuple(tup) => {
            if tup.elems.is_empty() {
                "void".to_string()
            } else {
                let mut ret = String::new();
                for elem in &tup.elems {
                    if !ret.is_empty() {
                        ret.push_str(", ");
                    }
                    ret.push_str(&to_cpp_type(&elem));
                }
                format!("std::tuple<{}>", ret)
            }
        }

        _ => {
            quote::quote!(#ty).to_string() // fallback to raw Rust type
        }
    }
}


use syn::{ReturnType, PathArguments};

/// Extract the inner `T` from either `T` or `Result<T, E>`
fn strip_result(ret: &ReturnType) -> String {
    match ret {
        ReturnType::Default => "void".to_string(),
        ReturnType::Type(_, ty) => match ty.as_ref() {
            Type::Path(type_path) => {
                let segments = &type_path.path.segments;
                let last = segments.last().unwrap();

                if last.ident == "Result" {
                    // Result<T, E> → extract T
                    if let PathArguments::AngleBracketed(args) = &last.arguments {
                        args.args.first().and_then(|arg| {
                            if let syn::GenericArgument::Type(inner_ty) = arg {
                                let inner_ty_stripped = strip_reference(inner_ty);
                                Some(to_cpp_type(inner_ty_stripped))
                            } else {
                                Some("void".to_string())
                            }
                        }).unwrap()
                    } else {
                        "void".to_string()
                    }
                } else {
                    to_cpp_type(ty)
                }
            }
            _ => to_cpp_type(ty),
        },
    }
}

fn main() {
    use clap::{Arg, Command};

    let matches = Command::new("cpp_binding_generator")
        .arg(Arg::new("input")
            .long("input")
            .short('i')
            .value_name("INPUT.rs")
            .help("Input Rust source file")
            .required(true))
        .arg(Arg::new("trait")
            .long("trait") 
            .short('t')
            .value_name("TRAIT_NAME")
            .help("Trait name")
            .required(true))
        .arg(Arg::new("output")
            .long("output")
            .short('o')
            .value_name("OUTPUT.h")
            .help("Output C++ header file")
            .required(true))
        .get_matches();

    let src_path = PathBuf::from(matches.get_one::<String>("input").unwrap());
    let trait_name = matches.get_one::<String>("trait").unwrap();
    let header_path = PathBuf::from(matches.get_one::<String>("output").unwrap());

    let code = fs::read_to_string(&src_path).expect("Failed to read source file");
    let syntax = syn::parse_file(&code).expect("Failed to parse source file");

    let my_trait: ItemTrait = syntax.items.into_iter().find_map(|item| {
        if let Item::Trait(tr) = item {
            if tr.ident == trait_name {
                Some(tr)
            } else {
                None
            }
        } else {
            None
        }
    }).expect("Trait not found");

    let mut out = String::from("// Auto-generated C++ header\n#pragma once\n\n");

    out.push_str(&format!("struct {} {{\n", trait_name));

    out.push_str(&format!("    constexpr static std::string_view kTraitName = \"{}\";\n", trait_name));
    out.push_str(&format!("    constexpr static std::string_view kVersion = \"0.1.0\";\n"));

    let methods : Vec<_> = my_trait
        .items
        .iter()
        .filter_map(|item| {
            if let syn::TraitItem::Fn(trait_fn) = item {
                Some(trait_fn.sig.clone())
            } else {
                None
            }
        })
    .collect();

    let method_names = methods.iter().map(|method| method.ident.to_string()).collect::<Vec<_>>();

    out.push_str(&format!("    constexpr static auto kExportNames = std::array<std::string_view, {}>{{\n", method_names.len()));
    for method_name in method_names {
        out.push_str(&format!("            \"{}\",\n", method_name));
    }
    out.push_str("        };\n");

    let mut method_sigs = Vec::new();

    for method in methods {

        let mut sig_str = String::new();
        let ret2 = strip_result(&method.output);
        sig_str.push_str(&format!("{}(*)(", ret2));

        let mut first = true;
        for arg in method.inputs.iter().skip(1) {
            if let syn::FnArg::Typed(arg) = arg {
                if !first {
                    sig_str.push_str(", ");
                }
                let ty = strip_reference(&arg.ty);
                let ty_str = quote!(#ty).to_string();
                sig_str.push_str(&ty_str);
                first = false;
            } else {
                panic!("Expected typed argument");
            }
        }

        let full_sig = format!("{})", sig_str);
        method_sigs.push(full_sig);
    }

    let method_sigs_str = method_sigs.join(",\n            ");
    
    out.push_str(&format!("    using export_types = std::tuple<\n            {}>;\n", method_sigs_str));

    out.push_str("};\n");



    fs::create_dir_all(header_path.parent().unwrap()).unwrap();
    fs::write(&header_path, out).unwrap();

    println!("Header written to {}", header_path.display());
}