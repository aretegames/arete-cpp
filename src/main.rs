use ::std::{env, fs};
use std::ffi::OsStr;

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

    let file = fs::read_to_string(&input).unwrap();

    let mut parsed_info = ParsedInfo::default();

    let re = Regex::new("COMPONENT\\(\\s*(\\S+)\\s*\\)").unwrap();
    for component in re.captures_iter(&file) {
        parsed_info.parse_struct(component.get(1).unwrap().as_str(), false);
    }

    let re = Regex::new("RESOURCE\\(\\s*(\\S+)\\s*\\)").unwrap();
    for resource in re.captures_iter(&file) {
        parsed_info.parse_struct(resource.get(1).unwrap().as_str(), true);
    }

    let re = Regex::new("SYSTEM_ONCE\\([\\s\\S]*?(\\S+)\\s*,([\\s\\S]+?)\\)").unwrap();
    for system in re.captures_iter(&file) {
        parsed_info.parse_system(
            system.get(1).unwrap().as_str(),
            system.get(2).unwrap().as_str(),
            true,
        );
    }

    let re = Regex::new("SYSTEM\\([\\s\\S]*?(\\S+)\\s*,([\\s\\S]+?)\\)").unwrap();
    for system in re.captures_iter(&file) {
        parsed_info.parse_system(
            system.get(1).unwrap().as_str(),
            system.get(2).unwrap().as_str(),
            false,
        );
    }

    fs::write(output, parsed_info.gen_ffi(input.file_name().unwrap())).unwrap();
}

#[derive(Debug)]
enum StructType {
    Component,
    Resource,
}

#[derive(Debug)]
enum ArgType {
    DataAccessDirect,
    DataAccessCell,
    Query { inputs: Vec<SystemInputInfo> },
}

#[derive(Debug, Default)]
struct ParsedInfo {
    systems: Vec<SystemInfo>,
    structs: Vec<StructInfo>,
}

#[derive(Debug)]
struct SystemInfo {
    ident: String,
    is_once: bool,
    inputs: Vec<SystemInputInfo>,
}

#[derive(Debug)]
struct SystemInputInfo {
    ident: String,
    arg_type: ArgType,
    mutable: bool,
}

#[derive(Debug)]
struct StructInfo {
    ident: String,
    string_id: String,
    struct_type: StructType,
}

impl ParsedInfo {
    fn parse_system(&mut self, ident: &str, body: &str, is_once: bool) {
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

        self.systems.push(SystemInfo {
            ident: ident.to_owned(),
            is_once,
            inputs,
        });
    }

    fn parse_struct(&mut self, ident: &str, is_resource: bool) {
        let struct_type = if is_resource {
            StructType::Resource
        } else {
            StructType::Component
        };

        self.structs.push(StructInfo {
            ident: ident.to_string(),
            string_id: String::from("game_module::") + ident,
            struct_type,
        });
    }

    fn gen_ffi(self, include_file: &OsStr) -> String {
        let mut output = String::new();

        output += &format!("#include {include_file:?}\n");
        output += "#include <cstring>\n\n";

        output += &self.gen_components();
        output += &self.gen_resource_init();
        output += &self.gen_systems();

        output
    }

    fn gen_components(&self) -> String {
        let mut output = String::new();

        output += "enum ComponentType {\n";
        output += "    ComponentTypeComponent,\n";
        output += "    ComponentTypeResource,\n";
        output += "};\n\n";

        output += "enum ArgType {\n";
        output += "    ArgTypeDataAccessMut,\n";
        output += "    ArgTypeDataAccessRef,\n";
        output += "    ArgTypeQuery,\n";
        output += "};\n\n";

        output += "enum QueryType {\n";
        output += "    QueryTypeComponentMut,\n";
        output += "    QueryTypeComponentRef,\n";
        output += "};\n\n";

        output += &self.gen_component_size();
        output += &self.gen_component_align();
        output += &self.gen_component_type();
        output += &self.gen_set_component_ids();

        output
    }

    fn gen_component_size(&self) -> String {
        let mut output = String::new();

        output += "extern \"C\" size_t component_size(const char* string_id) {\n";

        if self.structs.is_empty() {
            output += "    std::abort();\n"
        } else {
            output += &format!(
                "    if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                self.structs[0].string_id
            );
            output += &format!("        return sizeof({});\n", self.structs[0].ident);
            for struct_info in &self.structs[1..] {
                output += &format!(
                    "    }} else if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                    struct_info.string_id
                );
                output += &format!("        return sizeof({});\n", struct_info.ident);
            }

            output += "    } else {\n";
            output += "        std::abort();\n";
            output += "    }\n";
        }

        output += "}\n\n";

        output
    }

    fn gen_component_align(&self) -> String {
        let mut output = String::new();

        output += "extern \"C\" size_t component_align(const char* string_id) {\n";

        if self.structs.is_empty() {
            output += "    std::abort();\n";
        } else {
            output += &format!(
                "    if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                self.structs[0].string_id
            );
            output += &format!("        return alignof({});\n", self.structs[0].ident);
            for struct_info in &self.structs[1..] {
                output += &format!(
                    "    }} else if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                    struct_info.string_id
                );
                output += &format!("        return alignof({});\n", struct_info.ident);
            }

            output += "    } else {\n";
            output += "        std::abort();\n";
            output += "    }\n";
        }

        output += "}\n\n";

        output
    }

    fn gen_component_type(&self) -> String {
        let mut output = String::new();

        output += "extern \"C\" ComponentType component_type(const char* string_id) {\n";

        if self.structs.is_empty() {
            output += "    std::abort();\n";
        } else {
            output += &format!(
                "    if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                self.structs[0].string_id
            );

            output += &format!(
                "        static_assert(std::is_standard_layout_v<{}>);\n",
                self.structs[0].ident
            );

            match &self.structs[0].struct_type {
                StructType::Component => {
                    output += &format!(
                        "        static_assert(std::is_trivially_copyable_v<{}>);\n",
                        self.structs[0].ident
                    );
                    output += "        return ComponentTypeComponent;\n";
                }
                StructType::Resource => {
                    output += "        return ComponentTypeResource;\n";
                }
            };

            for struct_info in &self.structs[1..] {
                output += &format!(
                    "    }} else if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                    struct_info.string_id
                );

                output += &format!(
                    "        static_assert(std::is_standard_layout_v<{}>);\n",
                    struct_info.ident
                );

                match &struct_info.struct_type {
                    StructType::Component => {
                        output += &format!(
                            "        static_assert(std::is_trivially_copyable_v<{}>);\n",
                            struct_info.ident
                        );
                        output += "        return ComponentTypeComponent;\n";
                    }
                    StructType::Resource => {
                        output += "        return ComponentTypeResource;\n";
                    }
                }
            }

            output += "    } else {\n";
            output += "        std::abort();\n";
            output += "    }\n";
        }

        output += "}\n\n";

        output
    }

    fn gen_set_component_ids(&self) -> String {
        struct ComponentInfo<'a> {
            ident: &'a str,
            string_id: String,
        }

        let mut components: Vec<_> = self
            .systems
            .iter()
            .flat_map(|s| &s.inputs)
            .filter_map(|i| {
                if i.ident != "Query" {
                    let string_id = self
                        .structs
                        .iter()
                        .find(|s| s.ident == i.ident)
                        .map(|s| s.string_id.clone())
                        .unwrap_or_else(|| String::from("arete_public::") + &i.ident);
                    Some(ComponentInfo {
                        ident: &i.ident,
                        string_id,
                    })
                } else {
                    None
                }
            })
            .chain(ARETE_PUBLIC_COMPONENTS.iter().map(|ident| ComponentInfo {
                ident,
                string_id: String::from("arete_public::") + ident,
            }))
            .collect();

        components.sort_unstable_by(|a, b| a.ident.cmp(b.ident));
        components.dedup_by(|a, b| a.ident == b.ident);

        let mut output = String::new();

        output += "extern \"C\" void set_component_id(const char* string_id, ComponentId id) {\n";

        if !components.is_empty() {
            output += &format!(
                "    if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                components[0].string_id
            );
            output += &format!("        Component<{}>::ID = id;\n", components[0].ident);

            for component in &components[1..] {
                output += &format!(
                    "    }} else if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                    component.string_id
                );
                output += &format!("        Component<{}>::ID = id;\n", component.ident);
            }

            output += "    }\n";
        }

        output += "}\n\n";

        output
    }

    fn gen_resource_init(&self) -> String {
        let resources: Vec<_> = self
            .structs
            .iter()
            .filter(|s| matches!(s.struct_type, StructType::Resource))
            .collect();

        let mut output = String::new();

        output += "extern \"C\" int32_t resource_init(const char* string_id, void* val) {\n";

        if resources.is_empty() {
            output += "    return 1;\n";
        } else {
            output += &format!(
                "    if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                resources[0].string_id
            );
            output += &format!(
                "        *static_cast<{}*>(val) = {}{{}};\n",
                resources[0].ident, resources[0].ident,
            );

            for resource in &resources[1..] {
                output += &format!(
                    "    }} else if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                    resource.string_id
                );
                output += &format!(
                    "        *static_cast<{}*>(val) = {}{{}};\n",
                    resource.ident, resource.ident,
                );
            }

            output += "    } else {\n";
            output += "        return 1;\n";
            output += "    }\n\n";
            output += "    return 0;\n";
        }

        output += "}\n\n";

        output
    }

    fn gen_systems(&self) -> String {
        let mut output = String::new();

        output += &self.gen_system_fn_ffi();
        output += &self.gen_systems_len();
        output += &self.gen_system_is_once();
        output += &self.gen_system_fn();
        output += &self.gen_system_args_len();
        output += &self.gen_system_arg_type();
        output += &self.gen_system_arg_component();
        output += &self.gen_system_arg_query_len();
        output += &self.gen_system_arg_query_component();
        output += &self.gen_system_arg_query_type();

        output
    }

    fn gen_system_fn_ffi(&self) -> String {
        let mut output = String::new();

        let gen_system_fn = &mut |system: &SystemInfo| {
            let struct_ident = system.ident.to_case(Case::Pascal) + "Data";

            output += "struct ";
            output += &struct_ident;
            output += " {\n";

            for input in &system.inputs {
                output += "    ";
                if !input.mutable {
                    output += "const ";
                }
                output += &input.ident;
                output += "* ";
                output += &input.ident.to_case(Case::Snake);
                if matches!(input.arg_type, ArgType::DataAccessCell) {
                    output += "ComponentCell<";
                }
                if matches!(input.arg_type, ArgType::DataAccessCell) {
                    output += ">";
                }

                if let ArgType::Query { inputs } = &input.arg_type {
                    output += "<";
                    if inputs.len() > 1 {
                        output += "(";
                    }
                    for input in inputs {
                        output += "&'a ";
                        if input.mutable {
                            output += "mut ";
                        }
                        output += &input.ident;
                        output += ", ";
                    }
                    if inputs.len() > 1 {
                        output += ")";
                    }
                    output += ">";
                }

                output += ";\n";
            }

            output += "};\n\n";

            output += "int32_t ";
            output += &system.ident;
            output += "_ffi(void* input) {\n";
            output += "    auto data = static_cast<";
            output += &struct_ident;
            output += "*>(input);\n";
            output += "    ";
            output += &system.ident;
            output += "(\n";

            for input in &system.inputs[..system.inputs.len() - 1] {
                output += "        *data->";
                output += &input.ident.to_case(Case::Snake);
                output += ",\n";
            }

            output += "        *data->";
            output += &system.inputs.last().unwrap().ident.to_case(Case::Snake);
            output += "\n";

            output += "    );\n\n";
            output += "    return 0;\n";
            output += "}\n\n";
        };

        for system in &self.systems {
            gen_system_fn(system);
        }

        output
    }

    fn gen_systems_len(&self) -> String {
        let mut output = String::new();

        output += "extern \"C\" size_t systems_len() {\n";
        output += &format!("    return {};\n", self.systems.len());
        output += "}\n\n";

        output
    }

    fn gen_system_is_once(&self) -> String {
        let mut output = String::new();

        output += "extern \"C\" bool system_is_once(size_t system_index) {\n";
        output += "    switch (system_index) {\n";

        for (i, system) in self.systems.iter().enumerate() {
            output += &format!("        case {i}: return {};\n", system.is_once);
        }

        output += "        default: std::abort();\n";
        output += "    }\n";
        output += "}\n\n";

        output
    }

    fn gen_system_fn(&self) -> String {
        let mut output = String::new();

        output += "typedef int32_t (*system_fn_ptr)(void*);\n\n";
        output += "extern \"C\" system_fn_ptr system_fn(size_t system_index) {\n";
        output += "    switch (system_index) {\n";

        for (i, system) in self.systems.iter().enumerate() {
            output += &format!("        case {i}: return {}_ffi;\n", system.ident);
        }

        output += "        default: std::abort();\n";
        output += "    }\n";
        output += "}\n\n";

        output
    }

    fn gen_system_args_len(&self) -> String {
        let mut output = String::new();

        output += "extern \"C\" size_t system_args_len(size_t system_index) {\n";
        output += "    switch (system_index) {\n";

        for (i, system) in self.systems.iter().enumerate() {
            output += &format!("        case {i}: return {};\n", system.inputs.len());
        }

        output += "        default: std::abort();\n";
        output += "    }\n";
        output += "}\n\n";

        output
    }

    fn gen_system_arg_type(&self) -> String {
        let mut output = String::new();

        output += "extern \"C\" ArgType system_arg_type(size_t system_index, size_t arg_index) {\n";
        output += "    switch (system_index) {\n";

        for (i, system) in self.systems.iter().enumerate() {
            output += &format!("        case {i}: switch (arg_index) {{\n");

            for (i, input) in system.inputs.iter().enumerate() {
                output += &format!("            case {i}: return ArgType");
                output += match &input.arg_type {
                    ArgType::DataAccessDirect if input.mutable => "DataAccessMut;\n",
                    ArgType::DataAccessDirect => "DataAccessRef;\n",
                    ArgType::DataAccessCell => "DataAccessRef;\n",
                    ArgType::Query { .. } => "Query;\n",
                };
            }

            output += "            default: std::abort();\n";
            output += "        }\n";
        }

        output += "        default: std::abort();\n";
        output += "    }\n";
        output += "}\n\n";

        output
    }

    fn gen_system_arg_component(&self) -> String {
        let string_id = |ident: &str| {
            self.structs
                .iter()
                .find(|s| s.ident == ident)
                .map(|s| s.string_id.clone())
                .unwrap_or_else(|| String::from("arete_public::") + ident)
        };

        let mut output = String::new();

        output += "extern \"C\" const char* system_arg_component(size_t system_index, size_t arg_index) {\n";
        output += "    switch (system_index) {\n";

        for (i, system) in self.systems.iter().enumerate() {
            output += &format!("        case {i}: switch (arg_index) {{\n");

            for (i, input) in system.inputs.iter().enumerate().filter(|(_, input)| {
                matches!(
                    input.arg_type,
                    ArgType::DataAccessDirect | ArgType::DataAccessCell
                )
            }) {
                output += &format!(
                    "            case {i}: return \"{}\";\n",
                    string_id(&input.ident)
                );
            }

            output += "            default: std::abort();\n";
            output += "        }\n";
        }

        output += "        default: std::abort();\n";
        output += "    }\n";
        output += "}\n\n";

        output
    }

    fn gen_system_arg_query_len(&self) -> String {
        let mut output = String::new();

        output +=
            "extern \"C\" size_t system_arg_query_len(size_t system_index, size_t arg_index) {\n";
        output += "    switch (system_index) {\n";

        for (i, system) in self.systems.iter().enumerate().filter(|(_, system)| {
            system
                .inputs
                .iter()
                .any(|input| matches!(input.arg_type, ArgType::Query { .. }))
        }) {
            output += &format!("        case {i}: switch (arg_index) {{\n");

            for (i, input) in system.inputs.iter().enumerate() {
                if let ArgType::Query { inputs } = &input.arg_type {
                    output += &format!("            case {i}: return {};\n", inputs.len());
                }
            }

            output += "            default: std::abort();\n";
            output += "        }\n";
        }

        output += "        default: std::abort();\n";
        output += "    }\n";
        output += "}\n\n";

        output
    }

    fn gen_system_arg_query_component(&self) -> String {
        let mut output = String::new();

        output += "extern \"C\" const char* system_arg_query_component(\n";
        output += "    size_t system_index,\n";
        output += "    size_t arg_index,\n";
        output += "    size_t query_index\n";
        output += ") {\n";
        output += "    switch (system_index) {\n";

        for (i, system) in self.systems.iter().enumerate().filter(|(_, system)| {
            system
                .inputs
                .iter()
                .any(|input| matches!(input.arg_type, ArgType::Query { .. }))
        }) {

            output += &format!("        case {i}: switch (arg_index) {{\n");

            for (i, input) in system.inputs.iter().enumerate() {
                if let ArgType::Query { inputs } = &input.arg_type {
                    output += &format!("            case {i}: switch (query_index) {{\n");

                    for (i, input) in inputs.iter().enumerate() {
                        output += &format!(
                            "                case {i}: {}::string_id().as_ptr(),\n",
                            input.ident
                        );
                    }

                    output += "                default: std::abort();\n";
                    output += "            }\n";
                }
            }

            output += "            default: std::abort();\n";
            output += "        }\n";

            todo!();
        }

        output += "        default: std::abort();\n";
        output += "    }\n";
        output += "}\n\n";

        output
    }

    fn gen_system_arg_query_type(&self) -> String {
        let mut output = String::new();

        output += "extern \"C\" QueryType system_arg_query_type(\n";
        output += "    size_t system_index,\n";
        output += "    size_t arg_index,\n";
        output += "    size_t query_index\n";
        output += ") {\n";
        output += "    switch (system_index) {\n";

        for (i, system) in self.systems.iter().enumerate().filter(|(_, system)| {
            system
                .inputs
                .iter()
                .any(|input| matches!(input.arg_type, ArgType::Query { .. }))
        }) {
            output += &format!("        case {i}: switch (arg_index) {{\n");

            for (i, input) in system.inputs.iter().enumerate() {
                if let ArgType::Query { inputs } = &input.arg_type {
                    output += &format!("            {i} => switch (query_index) {{\n");

                    for (i, input) in inputs.iter().enumerate() {
                        output += &format!("                case {i}: QueryType::Component");
                        output += match input.mutable {
                            true => "Mut,\n",
                            false => "Ref,\n",
                        };
                    }

                    output += "                default: std::abort(),\n";
                    output += "            }\n";
                }
            }

            output += "            default: std::abort();\n";
            output += "        }\n";
        }

        output += "        default: std::abort();\n";
        output += "    }\n";
        output += "}\n\n";

        output
    }
}
