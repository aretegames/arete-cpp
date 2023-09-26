use ::std::{env, fs};
use std::path::Path;

use arete_codegen_core::*;
use clap::Parser;
use regex::Regex;

#[derive(Parser, Debug)]
#[command(author, version)]
struct Args {
    #[arg(short, long)]
    input: String,

    #[arg(short, long)]
    output: Option<String>,
}

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

fn parse_system(info: &mut FfiGenerator, ident: &str, mut body: &str, is_once: bool) {
    let mut inputs = Vec::new();

    body = body.trim();

    while !body.is_empty() {
        let mutable;
        if body.starts_with("const") {
            mutable = false;
            body = body[5..].trim_start();
        } else {
            mutable = true;
        }

        let ident_end = body
            .find('&')
            .expect("all parameters must be taken as references");

        if body.starts_with("Query") {
            body = body[5..].trim_start();
            assert!(body.starts_with('<'), "malformed query");
            body = body[1..].trim_start();

            let mut query_inputs = Vec::new();

            while !body.starts_with('&') {
                let mutable;
                if body.starts_with("const") {
                    mutable = false;
                    body = body[5..].trim_start();
                } else {
                    mutable = true;
                }

                let ident_end = body.find('&').expect("malformed query");

                let ident = body[..ident_end].trim_end().to_owned();

                query_inputs.push(SystemInputInfo {
                    ident,
                    arg_type: ArgType::DataAccessDirect,
                    mutable,
                });

                body = body[ident_end + 1..].trim_start()[1..].trim_start();
            }

            inputs.push(SystemInputInfo {
                ident: "query".to_owned() + &inputs.len().to_string(),
                arg_type: ArgType::Query {
                    inputs: query_inputs,
                },
                mutable,
            });
        } else {
            let ident = body[..ident_end].trim_end().to_owned();

            inputs.push(SystemInputInfo {
                ident,
                arg_type: ArgType::DataAccessDirect,
                mutable,
            });
        }

        if let Some(i) = body.find(',') {
            body = body[i + 1..].trim_start();
        } else {
            break;
        }
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
