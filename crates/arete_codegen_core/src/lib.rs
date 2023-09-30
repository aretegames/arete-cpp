#[derive(Debug, Default)]
pub struct FfiGenerator {
    pub systems: Vec<SystemInfo>,
    pub structs: Vec<StructInfo>,
}

#[derive(Debug)]
pub enum StructType {
    Component,
    Resource,
}

#[derive(Debug)]
pub enum ArgType {
    DataAccessDirect,
    DataAccessCell,
    Query { inputs: Vec<SystemInputInfo> },
}

#[derive(Debug)]
pub struct SystemInfo {
    pub ident: String,
    pub is_once: bool,
    pub inputs: Vec<SystemInputInfo>,
}

#[derive(Debug)]
pub struct SystemInputInfo {
    pub ident: String,
    pub arg_type: ArgType,
    pub mutable: bool,
}

#[derive(Debug)]
pub struct StructInfo {
    pub ident: String,
    pub string_id: String,
    pub struct_type: StructType,
}

const ARETE_PUBLIC_COMPONENTS: &[&str] = &[
    "Camera",
    "Color",
    "DirectionalLight",
    "DynamicStaticMesh",
    "PointLight",
    "Transform",
];

impl FfiGenerator {
    pub fn gen_ffi(self, header: String) -> String {
        let mut output = header;

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

        output += &self.gen_component_string_id();
        output += &self.gen_component_size();
        output += &self.gen_component_align();
        output += &self.gen_component_type();
        output += &self.gen_set_component_ids();

        output
    }

    fn gen_component_string_id(&self) -> String {
        let mut output = String::new();

        output += "extern \"C\" const char* component_string_id(size_t index) {\n";
        output += "    switch(index) {\n";

        for (i, struct_info) in self.structs.iter().enumerate() {
            output += &format!("        case {i}: return \"{}\";\n", struct_info.string_id);
        }

        output += "        default: return nullptr;\n";
        output += "    }\n";
        output += "}\n\n";

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
            for (i, struct_info) in self.structs.iter().enumerate() {
                if i == 0 {
                    output += &format!(
                        "    if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                        self.structs[0].string_id
                    );
                } else {
                    output += &format!(
                        "    }} else if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                        struct_info.string_id
                    );
                }

                match &struct_info.struct_type {
                    StructType::Component => {
                        output += &format!(
                            "        static_assert(std::is_standard_layout_v<{}>);\n",
                            struct_info.ident
                        );
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
                if !matches!(i.arg_type, ArgType::Query { .. }) {
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
            .chain(self.structs.iter().map(|s| ComponentInfo {
                ident: &s.ident,
                string_id: s.string_id.clone(),
            }))
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
            for (i, resource) in resources.iter().enumerate() {
                if i == 0 {
                    output += &format!(
                        "    if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                        resource.string_id
                    );
                } else {
                    output += &format!(
                        "    }} else if (std::strcmp(string_id, \"{}\") == 0) {{\n",
                        resource.string_id
                    );
                }
                output += &format!(
                    "        std::construct_at(static_cast<{}*>(val));\n",
                    resource.ident
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
            output += "int32_t ";
            output += &system.ident;
            output += "_ffi(void** input) {\n";
            output += "    ";
            output += &system.ident;
            output += "(\n";

            for (i, input) in system.inputs.iter().enumerate() {
                output += "        *static_cast<";

                if !input.mutable {
                    output += "const ";
                }

                if let ArgType::Query { inputs } = &input.arg_type {
                    output += "Query<";
                    for (i, input) in inputs.iter().enumerate() {
                        if !input.mutable {
                            output += "const ";
                        }
                        output += &input.ident;
                        if i + 1 < inputs.len() {
                            output += "&, ";
                        }
                    }
                    output += "&>";
                } else {
                    output += &input.ident;
                }

                output += &format!("*>(input[{i}])");

                if i + 1 < system.inputs.len() {
                    output += ",\n";
                } else {
                    output += "\n";
                }
            }

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

        output += "typedef int32_t (*system_fn_ptr)(void**);\n\n";
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
        let string_id = |ident: &str| {
            self.structs
                .iter()
                .find(|s| s.ident == ident)
                .map(|s| s.string_id.clone())
                .unwrap_or_else(|| String::from("arete_public::") + ident)
        };

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
                            "                case {i}: return \"{}\";\n",
                            string_id(&input.ident)
                        );
                    }

                    output += "                default: std::abort();\n";
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
                    output += &format!("            case {i}: switch (query_index) {{\n");

                    for (i, input) in inputs.iter().enumerate() {
                        output += &format!("                case {i}: return QueryTypeComponent");
                        output += match input.mutable {
                            true => "Mut;\n",
                            false => "Ref;\n",
                        };
                    }

                    output += "                default: std::abort();\n";
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
