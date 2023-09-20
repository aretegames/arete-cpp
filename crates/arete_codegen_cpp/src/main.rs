use ::std::{env, fs};
use std::{ffi::OsStr, path::Path};

use arete_codegen_core::*;
use clap::Parser;
use convert_case::{Case, Casing};
use regex::Regex;

#[derive(Parser, Debug)]
#[command(author, version)]
struct Args {
    #[arg(short, long)]
    input: String,

    #[arg(short, long)]
    output: Option<String>,
}

const ARETE_PUBLIC_COMPONENTS: &[&str] = &[
    "Camera",
    "Color",
    "DirectionalLight",
    "DynamicStaticMesh",
    "PointLight",
    "Transform",
];

fn main() {
    let args = Args::parse();

    let current_exe = env::current_exe().unwrap();
    let exe_dir = current_exe.parent().unwrap();

    let input = exe_dir.join(&args.input);

    let output = if let Some(output) = args.output {
        exe_dir.join(output)
    } else {
        exe_dir.join(&args.input).parent().unwrap().join("ffi.cpp")
    };

    let file = filter_read_file(&input);

    let mut ffi_generator = FfiGenerator::default();

    let re = Regex::new("COMPONENT\\(\\s*(\\S+)\\s*\\)").unwrap();
    for component in re.captures_iter(&file) {
        parse_struct(
            &mut ffi_generator,
            component.get(1).unwrap().as_str(),
            false,
        );
    }

    let re = Regex::new("RESOURCE\\(\\s*(\\S+)\\s*\\)").unwrap();
    for resource in re.captures_iter(&file) {
        parse_struct(&mut ffi_generator, resource.get(1).unwrap().as_str(), true);
    }

    let re = Regex::new("SYSTEM_ONCE\\([\\s\\S]*?(\\S+)\\s*,([\\s\\S]+?)\\)").unwrap();
    for system in re.captures_iter(&file) {
        parse_system(
            &mut ffi_generator,
            system.get(1).unwrap().as_str(),
            system.get(2).unwrap().as_str(),
            true,
        );
    }

    let re = Regex::new("SYSTEM\\([\\s\\S]*?(\\S+)\\s*,([\\s\\S]+?)\\)").unwrap();
    for system in re.captures_iter(&file) {
        parse_system(
            &mut ffi_generator,
            system.get(1).unwrap().as_str(),
            system.get(2).unwrap().as_str(),
            false,
        );
    }

    let mut output_header = String::new();

    output_header += &format!("#include {:?}\n", input.file_name().unwrap());
    output_header += "#include <cstring>\n\n";

    fs::write(output, ffi_generator.gen_ffi(output_header)).unwrap();
}

fn filter_read_file(path: &Path) -> String {
    let mut file = String::new();

    for line in fs::read_to_string(path).unwrap().lines() {
        file += line.find("//").map(|i| &line[..i]).unwrap_or(line);
    }

    while let Some(i_start) = file.find("/*") {
        let Some(i_end) = file[i_start..].find("*/") else {
            // ??
            break;
        };

        file.replace_range(i_start..=i_start + i_end + 1, "");
    }

    file
}

fn parse_system(info: &mut FfiGenerator, ident: &str, body: &str, is_once: bool) {
    let re_params = Regex::new("\\s*(const)?\\s*([a-zA-Z0-9]+)\\s*&[\\s\\S]*?,?").unwrap();

    let mut inputs = Vec::new();

    for param in re_params.captures_iter(body) {
        let ident = param.get(2).unwrap().as_str();
        let mutable = param.get(1).is_none();

        // let param_type = component.path.segments.last().unwrap().ident.to_string();

        // let (ident, arg_type) = if param_type == "Query" {
        //     let PathArguments::AngleBracketed(query_inputs) =
        //         &component.path.segments.last().unwrap().arguments
        //     else {
        //         panic!("invalid query generics")
        //     };

        //     let inputs = query_inputs
        //         .args
        //         .iter()
        //         .flat_map(|input| {
        //             let GenericArgument::Type(input) = input else {
        //                 panic!("invalid query generics")
        //             };

        //             if let Type::Reference(ty) = input {
        //                 let Type::Path(component) = ty.elem.as_ref() else {
        //                     panic!("unsupported query input type")
        //                 };

        //                 Vec::from([SystemInputInfo {
        //                     ident: component.path.segments.last().unwrap().ident.to_string(),
        //                     arg_type: ArgType::DataAccessDirect,
        //                     mutable: ty.mutability.is_some(),
        //                 }])
        //             } else {
        //                 let Type::Tuple(tuple) = input else {
        //                     panic!("unsupported query input type")
        //                 };

        //                 tuple
        //                     .elems
        //                     .iter()
        //                     .map(|elem| {
        //                         let Type::Reference(ty) = elem else {
        //                             panic!("system inputs must be references")
        //                         };

        //                         let Type::Path(component) = ty.elem.as_ref() else {
        //                             panic!("unsupported system input type")
        //                         };

        //                         SystemInputInfo {
        //                             ident: component
        //                                 .path
        //                                 .segments
        //                                 .last()
        //                                 .unwrap()
        //                                 .ident
        //                                 .to_string(),
        //                             arg_type: ArgType::DataAccessDirect,
        //                             mutable: ty.mutability.is_some(),
        //                         }
        //                     })
        //                     .collect()
        //             }
        //         })
        //         .collect();

        //     (param_type, ArgType::Query { inputs })
        // } else if param_type == "ComponentCell" {
        //     let PathArguments::AngleBracketed(query_inputs) =
        //         &component.path.segments.last().unwrap().arguments
        //     else {
        //         panic!("invalid query generics")
        //     };

        //     let GenericArgument::Type(input) = query_inputs.args.first().unwrap() else {
        //         panic!("invalid ComponentCell generic")
        //     };

        //     let Type::Path(component) = input else {
        //         panic!("unsupported system input type")
        //     };

        //     let ident = component.path.segments.last().unwrap().ident.to_string();

        //     (ident, ArgType::DataAccessCell)
        // } else {
        //     (param_type, ArgType::DataAccessDirect)
        // };

        inputs.push(SystemInputInfo {
            ident: ident.to_owned(),
            arg_type: ArgType::DataAccessDirect,
            mutable,
        });
    }

    info.systems.push(SystemInfo {
        ident: ident.to_owned(),
        is_once,
        inputs,
    });
}

fn parse_struct(info: &mut FfiGenerator, ident: &str, is_resource: bool) {
    let struct_type = if is_resource {
        StructType::Resource
    } else {
        StructType::Component
    };

    info.structs.push(StructInfo {
        ident: ident.to_string(),
        string_id: String::from("game_module::") + ident,
        struct_type,
    });
}
