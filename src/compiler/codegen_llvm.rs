use std::collections::HashMap;

use super::ir::{
    BootstrapBinaryOp, BootstrapExport, BootstrapExpr, BootstrapFunction, BootstrapGlobal,
    BootstrapModule, BootstrapProgram, BootstrapStmt, RuntimeBuiltin, ValueType,
};

pub fn emit_module(program: &BootstrapProgram, source_name: &str, target_triple: &str) -> String {
    let uses_abi_values = program_uses_abi_values(program);
    let mut emitter = ModuleEmitter::new(
        source_name,
        target_triple,
        &program.globals,
        program.module.is_some(),
    );
    let function_defs = program
        .functions
        .iter()
        .map(|function| emitter.emit_function(function))
        .collect::<Vec<_>>();
    let (module_defs, entry_def) = if let Some(module) = &program.module {
        (
            emitter.emit_shared_module(module, &program.main),
            None,
        )
    } else {
        (Vec::new(), Some(emitter.emit_main(&program.main)))
    };

    let mut out = String::new();
    out.push_str(&format!(
        "; Bootstrap Pasta compiler output\nsource_filename = \"{}\"\ntarget triple = \"{}\"\n\n",
        escape_metadata(source_name),
        target_triple
    ));

    if uses_abi_values || program.module.is_some() || emitter.needs_runtime_bridge {
        out.push_str("%pasta.pvalue = type { i32, i32, [16 x i8] }\n");
        out.push_str("%pasta.rt.dict_num_entry = type { ptr, double }\n\n");
    }

    for global in emitter.render_string_globals() {
        out.push_str(&global);
        out.push('\n');
    }
    for global in emitter.render_runtime_globals() {
        out.push_str(&global);
        out.push('\n');
    }
    if !emitter.string_globals.is_empty() || !emitter.global_vars.is_empty() || emitter.needs_puts || emitter.needs_printf {
        out.push('\n');
    }

    if emitter.needs_puts {
        out.push_str("declare i32 @puts(ptr noundef)\n");
    }
    if emitter.needs_printf {
        out.push_str("declare i32 @printf(ptr noundef, ...)\n");
    }
    if emitter.needs_strcmp {
        out.push_str("declare i32 @strcmp(ptr noundef, ptr noundef)\n");
    }
    if emitter.needs_strlen {
        out.push_str("declare i64 @strlen(ptr noundef)\n");
    }
    if emitter.needs_pasta_string_new {
        out.push_str("declare i32 @pasta_string_new(ptr noundef, i64 noundef, ptr noundef)\n");
    }
    if emitter.needs_pasta_string_view {
        out.push_str("declare i32 @pasta_string_view(%pasta.pvalue noundef, ptr noundef, ptr noundef)\n");
    }
    if emitter.needs_pasta_value_retain {
        out.push_str("declare i32 @pasta_value_retain(%pasta.pvalue noundef)\n");
    }
    if emitter.needs_pasta_value_release {
        out.push_str("declare i32 @pasta_value_release(%pasta.pvalue noundef)\n");
    }
    if emitter.needs_runtime_bridge {
        out.push_str("declare ptr @pasta_rt_window_new(ptr noundef, double noundef, double noundef)\n");
        out.push_str("declare i32 @pasta_rt_window_poll(ptr noundef)\n");
        out.push_str("declare ptr @pasta_rt_window_key(ptr noundef)\n");
        out.push_str("declare void @pasta_rt_window_close(ptr noundef)\n");
        out.push_str("declare void @pasta_rt_set_draw_target(ptr noundef)\n");
        out.push_str("declare void @pasta_rt_set_color_packed(double noundef)\n");
        out.push_str("declare void @pasta_rt_canvas_fill_rect(ptr noundef, double noundef, double noundef, double noundef, double noundef)\n");
        out.push_str("declare void @pasta_rt_swap_buffer(ptr noundef)\n");
        out.push_str("declare void @pasta_rt_fps_init(double noundef)\n");
        out.push_str("declare void @pasta_rt_fps_begin(double noundef)\n");
        out.push_str("declare void @pasta_rt_fps_end()\n");
        out.push_str("declare void @pasta_rt_fps_tick()\n");
        out.push_str("declare double @pasta_rt_rand_int2(double noundef, double noundef)\n");
        out.push_str("declare void @pasta_rt_list_new_numbers(ptr noundef, i64 noundef, ptr noundef)\n");
        out.push_str("declare void @pasta_rt_dict_new_string_number(ptr noundef, i64 noundef, ptr noundef)\n");
        out.push_str("declare double @pasta_rt_list_len(ptr noundef)\n");
        out.push_str("declare void @pasta_rt_list_slice(ptr noundef, double noundef, double noundef, ptr noundef)\n");
        out.push_str("declare void @pasta_rt_list_concat(ptr noundef, ptr noundef, ptr noundef)\n");
        out.push_str("declare double @pasta_rt_list_index_number(ptr noundef, double noundef)\n");
        out.push_str("declare double @pasta_rt_dict_get_number(ptr noundef, ptr noundef)\n");
        out.push_str("declare ptr @pasta_rt_number_to_string(double noundef)\n");
        out.push_str("declare ptr @pasta_rt_string_concat(ptr noundef, ptr noundef)\n");
    }
    if emitter.needs_puts
        || emitter.needs_printf
        || emitter.needs_strcmp
        || emitter.needs_strlen
        || emitter.needs_pasta_string_new
        || emitter.needs_pasta_string_view
        || emitter.needs_pasta_value_retain
        || emitter.needs_pasta_value_release
        || emitter.needs_runtime_bridge
    {
        out.push('\n');
    }

    for function in function_defs {
        out.push_str(&function);
        out.push('\n');
    }
    for definition in module_defs {
        out.push_str(&definition);
        out.push('\n');
    }
    if let Some(entry_def) = entry_def {
        out.push_str(&entry_def);
    }
    out
}

struct ModuleEmitter {
    shared_module_mode: bool,
    string_globals: Vec<StringGlobal>,
    string_lookup: HashMap<String, String>,
    next_string: usize,
    next_temp: usize,
    next_label: usize,
    global_vars: HashMap<String, ValueType>,
    needs_printf: bool,
    needs_puts: bool,
    needs_strcmp: bool,
    needs_strlen: bool,
    needs_pasta_string_new: bool,
    needs_pasta_string_view: bool,
    needs_pasta_value_retain: bool,
    needs_pasta_value_release: bool,
    needs_runtime_bridge: bool,
}

#[derive(Clone)]
struct StringGlobal {
    name: String,
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct VariableSlot {
    ptr: String,
    ty: ValueType,
}

#[derive(Clone)]
struct LoopContext {
    break_label: String,
    continue_label: String,
}

struct FunctionEmitter<'a> {
    module: &'a mut ModuleEmitter,
    locals: HashMap<String, VariableSlot>,
    allocas: Vec<String>,
    entry_inits: Vec<String>,
    body: Vec<String>,
    block_terminated: bool,
    is_main: bool,
    loop_stack: Vec<LoopContext>,
    dynamic_return_slot: Option<String>,
}

struct CompiledValue {
    ty: ValueType,
    operand: String,
    owned: bool,
}

impl ModuleEmitter {
    fn new(
        _source_name: &str,
        _target_triple: &str,
        globals: &[BootstrapGlobal],
        shared_module_mode: bool,
    ) -> Self {
        Self {
            shared_module_mode,
            string_globals: Vec::new(),
            string_lookup: HashMap::new(),
            next_string: 0,
            next_temp: 0,
            next_label: 0,
            global_vars: globals
                .iter()
                .map(|global| (global.name.clone(), global.ty.clone()))
                .collect(),
            needs_printf: false,
            needs_puts: false,
            needs_strcmp: false,
            needs_strlen: false,
            needs_pasta_string_new: false,
            needs_pasta_string_view: false,
            needs_pasta_value_retain: false,
            needs_pasta_value_release: false,
            needs_runtime_bridge: false,
        }
    }

    fn emit_function(&mut self, function: &BootstrapFunction) -> String {
        let shared_module_mode = self.shared_module_mode;
        let mut emitter = FunctionEmitter::new(self, false);
        let params_sig = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| format!("{} %arg{}.{}", llvm_type(&param.ty), index, sanitize_name(&param.name)))
            .collect::<Vec<_>>()
            .join(", ");

        for (index, param) in function.params.iter().enumerate() {
            let param_operand = format!("%arg{}.{}", index, sanitize_name(&param.name));
            let slot = emitter.ensure_local_variable(&param.name, &param.ty);
            if param.ty == ValueType::AbiValue {
                emitter.body.push(format!(
                    "store {} {}, ptr {}",
                    llvm_type(&param.ty),
                    param_operand,
                    slot.ptr
                ));
            } else {
                emitter.body.push(format!(
                    "store {} {}, ptr {}",
                    llvm_type(&param.ty),
                    param_operand,
                    slot.ptr
                ));
            }
        }

        emitter.emit_statements(&function.body);
        if !emitter.block_terminated {
            if function.return_type == ValueType::AbiValue {
                emitter.emit_cleanup_dynamic_locals();
                emitter.body.push("ret %pasta.pvalue zeroinitializer".to_string());
            } else {
                emitter.body.push(format!(
                    "ret {} {}",
                    llvm_type(&function.return_type),
                    llvm_zero_value(&function.return_type)
                ));
            }
            emitter.block_terminated = true;
        }
        let linkage = if shared_module_mode { "internal " } else { "" };
        let mut out = format!(
            "define {linkage}{} {}({}) {{\nentry:\n",
            llvm_type(&function.return_type),
            function_symbol(&function.name),
            params_sig
        );
        for alloca in &emitter.allocas {
            out.push_str("  ");
            out.push_str(alloca);
            out.push('\n');
        }
        for line in &emitter.entry_inits {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
        for line in &emitter.body {
            if !line.ends_with(':') {
                out.push_str("  ");
            }
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("}\n");
        out
    }

    fn emit_main(&mut self, statements: &[BootstrapStmt]) -> String {
        self.emit_entry_function("@main", statements)
    }

    fn emit_entry_function(&mut self, symbol: &str, statements: &[BootstrapStmt]) -> String {
        let mut emitter = FunctionEmitter::new(self, true);
        emitter.emit_statements(statements);
        let mut out = format!("define i32 {symbol}() {{\nentry:\n");
        for alloca in &emitter.allocas {
            out.push_str("  ");
            out.push_str(alloca);
            out.push('\n');
        }
        for line in &emitter.entry_inits {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
        for line in &emitter.body {
            if !line.ends_with(':') {
                out.push_str("  ");
            }
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("  ret i32 0\n}\n");
        out
    }

    fn emit_shared_module(
        &mut self,
        module: &BootstrapModule,
        init_statements: &[BootstrapStmt],
    ) -> Vec<String> {
        let mut out = Vec::new();
        out.push(self.emit_module_manifest_fn(module));
        out.push(self.emit_entry_function(&module_init_symbol(&module.name), init_statements));
        for export in &module.exports {
            out.push(self.emit_module_wrapper(module, export));
        }
        out
    }

    fn render_string_globals(&self) -> Vec<String> {
        self.string_globals
            .iter()
            .map(|global| {
                format!(
                    "{} = private unnamed_addr constant [{} x i8] c\"{}\"",
                    global.name,
                    global.bytes.len(),
                    bytes_to_llvm_string(&global.bytes)
                )
            })
            .collect()
    }

    fn render_runtime_globals(&self) -> Vec<String> {
        let linkage = if self.shared_module_mode { "internal " } else { "" };
        let mut globals = self
            .global_vars
            .iter()
            .map(|(name, ty)| {
                format!(
                    "{} = {linkage}global {} {}",
                    global_symbol(name),
                    llvm_type(ty),
                    llvm_zero_value(ty)
                )
            })
            .collect::<Vec<_>>();
        globals.sort();
        globals
    }

    fn next_temp(&mut self) -> String {
        let name = format!("%t{}", self.next_string + self.next_temp);
        self.next_temp += 1;
        name
    }

    fn next_label(&mut self, prefix: &str) -> String {
        let name = format!("{}.{}", prefix, self.next_label);
        self.next_label += 1;
        name
    }

    fn emit_c_string_ptr(&mut self, body: &mut Vec<String>, value: &str, fixed_name: Option<&str>) -> String {
        let global_name = if let Some(name) = fixed_name {
            if !self.string_lookup.contains_key(name) {
                self.string_lookup.insert(name.to_string(), name.to_string());
                self.string_globals.push(StringGlobal {
                    name: name.to_string(),
                    bytes: string_bytes(value),
                });
            }
            name.to_string()
        } else if let Some(existing) = self.string_lookup.get(value) {
            existing.clone()
        } else {
            let name = format!("@.str.{}", self.next_string);
            self.next_string += 1;
            self.string_lookup.insert(value.to_string(), name.clone());
            self.string_globals.push(StringGlobal {
                name: name.clone(),
                bytes: string_bytes(value),
            });
            name
        };

        let len = self
            .string_globals
            .iter()
            .find(|global| global.name == global_name)
            .map(|global| global.bytes.len())
            .expect("string global should exist");
        let temp = self.next_temp();
        body.push(format!(
            "{temp} = getelementptr inbounds [{len} x i8], ptr {global_name}, i64 0, i64 0"
        ));
        temp
    }

    fn emit_module_manifest_fn(&mut self, module: &BootstrapModule) -> String {
        let manifest = render_module_manifest_json(module);
        let mut body = Vec::new();
        let manifest_ptr =
            self.emit_c_string_ptr(&mut body, &manifest, Some(&module_manifest_global(&module.name)));
        let mut out = format!(
            "define ptr {}() {{\nentry:\n",
            module_manifest_symbol(&module.name)
        );
        for line in &body {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
        out.push_str(&format!("  ret ptr {manifest_ptr}\n"));
        out.push_str("}\n");
        out
    }

    fn emit_module_wrapper(&mut self, module: &BootstrapModule, export: &BootstrapExport) -> String {
        let mut emitter = FunctionEmitter::new(self, true);
        let argc_bad = emitter.module.next_label("wrapper.bad_arity");
        let null_out = emitter.module.next_label("wrapper.null_out");
        let null_argv = emitter.module.next_label("wrapper.null_argv");
        let check_argv = emitter.module.next_label("wrapper.check_argv");
        let call_label = emitter.module.next_label("wrapper.call");

        let out_null = emitter.module.next_temp();
        emitter.body.push(format!("{out_null} = icmp eq ptr %out, null"));
        emitter.body.push(format!(
            "br i1 {out_null}, label %{null_out}, label %{}",
            if export.params.is_empty() {
                call_label.clone()
            } else {
                check_argv.clone()
            }
        ));

        if !export.params.is_empty() {
            emitter.start_label(&check_argv);
            let argv_null = emitter.module.next_temp();
            emitter.body.push(format!("{argv_null} = icmp eq ptr %argv, null"));
            emitter.body.push(format!(
                "br i1 {argv_null}, label %{null_argv}, label %{call_label}"
            ));
        }

        emitter.start_label(&call_label);
        let argc_ok = emitter.module.next_temp();
        emitter.body.push(format!(
            "{argc_ok} = icmp eq i64 %argc, {}",
            export.params.len()
        ));
        let decode_label = emitter.module.next_label("wrapper.decode");
        emitter.body.push(format!(
            "br i1 {argc_ok}, label %{decode_label}, label %{argc_bad}"
        ));

        emitter.start_label(&decode_label);
        let mut native_args = Vec::new();
        for (index, ty) in export.params.iter().enumerate() {
            native_args.push(emitter.emit_wrapper_arg_decode(index, ty));
        }
        let temp = emitter.module.next_temp();
        let arg_sig = native_args
            .iter()
            .zip(export.params.iter())
            .map(|(value, ty)| format!("{} {}", llvm_type(ty), value))
            .collect::<Vec<_>>()
            .join(", ");
        emitter.body.push(format!(
            "{temp} = call {} {}({})",
            llvm_type(&export.return_type),
            function_symbol(&export.name),
            arg_sig
        ));
        emitter.emit_wrapper_return_store(&temp, &export.return_type);

        emitter.start_label(&argc_bad);
        emitter
            .body
            .push(format!("ret i32 {PABI_STATUS_ARITY_MISMATCH}"));

        emitter.start_label(&null_out);
        emitter.body.push(format!("ret i32 {PABI_STATUS_NULL_OUT}"));

        if !export.params.is_empty() {
            emitter.start_label(&null_argv);
            emitter
                .body
                .push(format!("ret i32 {PABI_STATUS_NULL_INPUT}"));
        }

        let mut out = format!(
            "define i32 {}(i64 %argc, ptr %argv, ptr %out) {{\nentry:\n",
            module_call_symbol(&module.name, &export.name)
        );
        for alloca in &emitter.allocas {
            out.push_str("  ");
            out.push_str(alloca);
            out.push('\n');
        }
        for line in &emitter.body {
            if !line.ends_with(':') {
                out.push_str("  ");
            }
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("}\n");
        out
    }
}

impl<'a> FunctionEmitter<'a> {
    fn new(module: &'a mut ModuleEmitter, is_main: bool) -> Self {
        Self {
            module,
            locals: HashMap::new(),
            allocas: Vec::new(),
            entry_inits: Vec::new(),
            body: Vec::new(),
            block_terminated: false,
            is_main,
            loop_stack: Vec::new(),
            dynamic_return_slot: None,
        }
    }

    fn emit_statements(&mut self, statements: &[BootstrapStmt]) {
        for statement in statements {
            if self.block_terminated {
                break;
            }
            self.emit_statement(statement);
        }
    }

    fn emit_statement(&mut self, statement: &BootstrapStmt) {
        match statement {
            BootstrapStmt::Assign { name, value, ty } => {
                let compiled = self.emit_expr(value);
                if *ty == ValueType::AbiValue {
                    let slot = self.ensure_local_variable(name, ty);
                    self.emit_store_dynamic_slot(&slot, compiled);
                    return;
                }
                if self.is_main {
                    self.body.push(format!(
                        "store {} {}, ptr {}",
                        llvm_type(ty),
                        compiled.operand,
                        global_symbol(name)
                    ));
                } else {
                    let slot = self.ensure_local_variable(name, ty);
                    self.body.push(format!(
                        "store {} {}, ptr {}",
                        llvm_type(ty),
                        compiled.operand,
                        slot.ptr
                    ));
                }
            }
            BootstrapStmt::If {
                condition,
                then_body,
                else_body,
            } => self.emit_if(condition, then_body, else_body.as_deref()),
            BootstrapStmt::While { condition, body } => self.emit_while(condition, body),
            BootstrapStmt::Break => {
                let loop_ctx = self
                    .loop_stack
                    .last()
                    .expect("BREAK should be validated before codegen")
                    .clone();
                self.emit_jump(&loop_ctx.break_label);
            }
            BootstrapStmt::Continue => {
                let loop_ctx = self
                    .loop_stack
                    .last()
                    .expect("CONTINUE should be validated before codegen")
                    .clone();
                self.emit_jump(&loop_ctx.continue_label);
            }
            BootstrapStmt::Print { value } => {
                let compiled = self.emit_expr(value);
                match compiled.ty {
                    ValueType::String => {
                        self.module.needs_puts = true;
                        let call = self.module.next_temp();
                        self.body.push(format!(
                            "{call} = call i32 @puts(ptr noundef {})",
                            compiled.operand
                        ));
                    }
                    ValueType::Number => {
                        self.module.needs_printf = true;
                        let int_value = self.module.next_temp();
                        let int_as_double = self.module.next_temp();
                        let is_integer = self.module.next_temp();
                        let int_label = self.module.next_label("print_num_int");
                        let float_label = self.module.next_label("print_num_float");
                        let end_label = self.module.next_label("print_num_end");

                        self.body.push(format!(
                            "{int_value} = fptosi double {} to i64",
                            compiled.operand
                        ));
                        self.body
                            .push(format!("{int_as_double} = sitofp i64 {int_value} to double"));
                        self.body.push(format!(
                            "{is_integer} = fcmp oeq double {}, {int_as_double}",
                            compiled.operand
                        ));
                        self.body.push(format!(
                            "br i1 {is_integer}, label %{int_label}, label %{float_label}"
                        ));

                        self.body.push(format!("{int_label}:"));
                        let int_fmt =
                            self.module
                                .emit_c_string_ptr(&mut self.body, "%lld\n", Some("@.fmt.num.int"));
                        let int_call = self.module.next_temp();
                        self.body.push(format!(
                            "{int_call} = call i32 (ptr, ...) @printf(ptr noundef {int_fmt}, i64 {int_value})"
                        ));
                        self.body.push(format!("br label %{end_label}"));

                        self.body.push(format!("{float_label}:"));
                        let float_fmt =
                            self.module
                                .emit_c_string_ptr(&mut self.body, "%g\n", Some("@.fmt.num.float"));
                        let float_call = self.module.next_temp();
                        self.body.push(format!(
                            "{float_call} = call i32 (ptr, ...) @printf(ptr noundef {float_fmt}, double {})",
                            compiled.operand
                        ));
                        self.body.push(format!("br label %{end_label}"));

                        self.body.push(format!("{end_label}:"));
                    }
                    ValueType::Bool => {
                        self.module.needs_puts = true;
                        let true_ptr = self.module.emit_c_string_ptr(&mut self.body, "true", Some("@.bool.true"));
                        let false_ptr = self.module.emit_c_string_ptr(&mut self.body, "false", Some("@.bool.false"));
                        let selected = self.module.next_temp();
                        self.body.push(format!(
                            "{selected} = select i1 {}, ptr {}, ptr {}",
                            compiled.operand, true_ptr, false_ptr
                        ));
                        let call = self.module.next_temp();
                        self.body.push(format!(
                            "{call} = call i32 @puts(ptr noundef {selected})"
                        ));
                    }
                    ValueType::AbiValue => {
                        panic!("opaque ABI values should be rejected before PRINT codegen")
                    }
                }
            }
            BootstrapStmt::Expr { value } => {
                let compiled = self.emit_expr(value);
                if compiled.ty == ValueType::AbiValue && compiled.owned {
                    self.emit_release_dynamic_value(&compiled.operand);
                }
            }
            BootstrapStmt::RuntimeCall { builtin, args } => {
                self.emit_runtime_statement_call(*builtin, args);
            }
            BootstrapStmt::Return { value } => {
                let compiled = self.emit_expr(value);
                if compiled.ty == ValueType::AbiValue {
                    self.emit_dynamic_return(compiled);
                    return;
                }
                self.body.push(format!(
                    "ret {} {}",
                    llvm_type(&compiled.ty),
                    compiled.operand
                ));
                self.block_terminated = true;
            }
        }
    }

    fn emit_if(
        &mut self,
        condition: &BootstrapExpr,
        then_body: &[BootstrapStmt],
        else_body: Option<&[BootstrapStmt]>,
    ) {
        let cond = self.emit_expr(condition);
        let then_label = self.module.next_label("if.then");
        let merge_label = self.module.next_label("if.end");
        let else_label = else_body
            .map(|_| self.module.next_label("if.else"))
            .unwrap_or_else(|| merge_label.clone());

        self.body.push(format!(
            "br i1 {}, label %{}, label %{}",
            cond.operand, then_label, else_label
        ));
        self.block_terminated = true;

        self.start_label(&then_label);
        self.emit_statements(then_body);
        let then_falls_through = !self.block_terminated;
        if then_falls_through {
            self.emit_jump(&merge_label);
        }

        let else_falls_through = if let Some(else_body) = else_body {
            self.start_label(&else_label);
            self.emit_statements(else_body);
            let falls_through = !self.block_terminated;
            if falls_through {
                self.emit_jump(&merge_label);
            }
            falls_through
        } else {
            true
        };

        if then_falls_through || else_falls_through {
            self.start_label(&merge_label);
        }
    }

    fn emit_while(&mut self, condition: &BootstrapExpr, body: &[BootstrapStmt]) {
        let cond_label = self.module.next_label("while.cond");
        let body_label = self.module.next_label("while.body");
        let end_label = self.module.next_label("while.end");

        self.emit_jump(&cond_label);
        self.start_label(&cond_label);
        let cond = self.emit_expr(condition);
        self.body.push(format!(
            "br i1 {}, label %{}, label %{}",
            cond.operand, body_label, end_label
        ));
        self.block_terminated = true;

        self.start_label(&body_label);
        self.loop_stack.push(LoopContext {
            break_label: end_label.clone(),
            continue_label: cond_label.clone(),
        });
        self.emit_statements(body);
        self.loop_stack.pop();
        if !self.block_terminated {
            self.emit_jump(&cond_label);
        }

        self.start_label(&end_label);
    }

    fn start_label(&mut self, label: &str) {
        self.body.push(format!("{label}:"));
        self.block_terminated = false;
    }

    fn emit_jump(&mut self, label: &str) {
        self.body.push(format!("br label %{label}"));
        self.block_terminated = true;
    }

    fn emit_wrapper_arg_decode(&mut self, index: usize, ty: &ValueType) -> String {
        let arg_ptr = self.module.next_temp();
        self.body.push(format!(
            "{arg_ptr} = getelementptr inbounds %pasta.pvalue, ptr %argv, i64 {index}"
        ));
        match ty {
            ValueType::Number => {
                self.emit_expect_pvalue_fields(&arg_ptr, 2, None);
                let data_ptr = self.module.next_temp();
                self.body.push(format!(
                    "{data_ptr} = getelementptr inbounds %pasta.pvalue, ptr {arg_ptr}, i32 0, i32 2"
                ));
                let value = self.module.next_temp();
                self.body.push(format!("{value} = load double, ptr {data_ptr}"));
                value
            }
            ValueType::Bool => {
                self.emit_expect_pvalue_fields(&arg_ptr, 1, None);
                let data_ptr = self.module.next_temp();
                self.body.push(format!(
                    "{data_ptr} = getelementptr inbounds %pasta.pvalue, ptr {arg_ptr}, i32 0, i32 2"
                ));
                let raw = self.module.next_temp();
                self.body.push(format!("{raw} = load i8, ptr {data_ptr}"));
                let value = self.module.next_temp();
                self.body.push(format!("{value} = icmp ne i8 {raw}, 0"));
                value
            }
            ValueType::String => {
                self.emit_expect_pvalue_fields(&arg_ptr, 3, Some(1));
                self.module.needs_pasta_string_view = true;
                let loaded = self.module.next_temp();
                self.body
                    .push(format!("{loaded} = load %pasta.pvalue, ptr {arg_ptr}"));
                let string_slot = format!("%arg{index}.string.slot");
                let len_slot = format!("%arg{index}.len.slot");
                self.allocas.push(format!("{string_slot} = alloca ptr"));
                self.allocas.push(format!("{len_slot} = alloca i64"));
                let status = self.module.next_temp();
                self.body.push(format!(
                    "{status} = call i32 @pasta_string_view(%pasta.pvalue {loaded}, ptr {string_slot}, ptr {len_slot})"
                ));
                let ok = self.module.next_temp();
                let good_label = self.module.next_label("wrapper.str.ok");
                let bad_label = self.module.next_label("wrapper.str.bad");
                self.body.push(format!("{ok} = icmp eq i32 {status}, 0"));
                self.body.push(format!(
                    "br i1 {ok}, label %{good_label}, label %{bad_label}"
                ));
                self.start_label(&bad_label);
                self.body.push(format!("ret i32 {status}"));
                self.start_label(&good_label);
                let value = self.module.next_temp();
                self.body.push(format!("{value} = load ptr, ptr {string_slot}"));
                value
            }
            ValueType::AbiValue => {
                let loaded = self.module.next_temp();
                self.body
                    .push(format!("{loaded} = load %pasta.pvalue, ptr {arg_ptr}"));
                self.emit_retain_dynamic_value(&loaded);
                loaded
            }
        }
    }

    fn emit_expect_pvalue_fields(&mut self, arg_ptr: &str, expected_tag: i32, expected_kind: Option<i32>) {
        let tag_ptr = self.module.next_temp();
        self.body.push(format!(
            "{tag_ptr} = getelementptr inbounds %pasta.pvalue, ptr {arg_ptr}, i32 0, i32 0"
        ));
        let tag = self.module.next_temp();
        self.body.push(format!("{tag} = load i32, ptr {tag_ptr}"));
        let tag_ok = self.module.next_temp();
        self.body
            .push(format!("{tag_ok} = icmp eq i32 {tag}, {expected_tag}"));
        let tag_good = self.module.next_label("wrapper.tag.ok");
        let tag_bad = self.module.next_label("wrapper.tag.bad");
        self.body
            .push(format!("br i1 {tag_ok}, label %{tag_good}, label %{tag_bad}"));
        self.start_label(&tag_bad);
        self.body
            .push(format!("ret i32 {PABI_STATUS_TYPE_MISMATCH}"));
        self.start_label(&tag_good);
        if let Some(expected_kind) = expected_kind {
            let kind_ptr = self.module.next_temp();
            self.body.push(format!(
                "{kind_ptr} = getelementptr inbounds %pasta.pvalue, ptr {arg_ptr}, i32 0, i32 1"
            ));
            let kind = self.module.next_temp();
            self.body.push(format!("{kind} = load i32, ptr {kind_ptr}"));
            let kind_ok = self.module.next_temp();
            self.body
                .push(format!("{kind_ok} = icmp eq i32 {kind}, {expected_kind}"));
            let good = self.module.next_label("wrapper.kind.ok");
            let bad = self.module.next_label("wrapper.kind.bad");
            self.body
                .push(format!("br i1 {kind_ok}, label %{good}, label %{bad}"));
            self.start_label(&bad);
            self.body
                .push(format!("ret i32 {PABI_STATUS_TYPE_MISMATCH}"));
            self.start_label(&good);
        }
    }

    fn emit_wrapper_return_store(&mut self, value: &str, ty: &ValueType) {
        match ty {
            ValueType::Number => {
                self.emit_store_pvalue_header(2, 0);
                let data_ptr = self.module.next_temp();
                self.body.push(format!(
                    "{data_ptr} = getelementptr inbounds %pasta.pvalue, ptr %out, i32 0, i32 2"
                ));
                self.body.push(format!("store double {value}, ptr {data_ptr}"));
                self.body.push(format!("ret i32 {PABI_STATUS_OK}"));
            }
            ValueType::Bool => {
                self.emit_store_pvalue_header(1, 0);
                let as_i8 = self.module.next_temp();
                self.body.push(format!("{as_i8} = zext i1 {value} to i8"));
                let data_ptr = self.module.next_temp();
                self.body.push(format!(
                    "{data_ptr} = getelementptr inbounds %pasta.pvalue, ptr %out, i32 0, i32 2"
                ));
                self.body.push(format!("store i8 {as_i8}, ptr {data_ptr}"));
                self.body.push(format!("ret i32 {PABI_STATUS_OK}"));
            }
            ValueType::String => {
                self.module.needs_pasta_string_new = true;
                self.module.needs_strlen = true;
                let null = self.module.next_temp();
                let empty_label = self.module.next_label("wrapper.ret.empty");
                let value_label = self.module.next_label("wrapper.ret.value");
                self.body.push(format!("{null} = icmp eq ptr {value}, null"));
                self.body.push(format!(
                    "br i1 {null}, label %{empty_label}, label %{value_label}"
                ));
                self.start_label(&empty_label);
                self.body.push(
                    "%ret.empty.status = call i32 @pasta_string_new(ptr null, i64 0, ptr %out)"
                        .to_string(),
                );
                self.body.push("ret i32 %ret.empty.status".to_string());
                self.start_label(&value_label);
                let len = self.module.next_temp();
                self.body
                    .push(format!("{len} = call i64 @strlen(ptr {value})"));
                let status = self.module.next_temp();
                self.body.push(format!(
                    "{status} = call i32 @pasta_string_new(ptr {value}, i64 {len}, ptr %out)"
                ));
                self.body.push(format!("ret i32 {status}"));
            }
            ValueType::AbiValue => {
                self.emit_retain_dynamic_value(value);
                self.body
                    .push(format!("store %pasta.pvalue {value}, ptr %out"));
                self.body.push(format!("ret i32 {PABI_STATUS_OK}"));
            }
        }
        self.block_terminated = true;
    }

    fn emit_store_pvalue_header(&mut self, tag_value: i32, handle_kind: i32) {
        let tag_ptr = self.module.next_temp();
        self.body.push(format!(
            "{tag_ptr} = getelementptr inbounds %pasta.pvalue, ptr %out, i32 0, i32 0"
        ));
        self.body.push(format!("store i32 {tag_value}, ptr {tag_ptr}"));
        let kind_ptr = self.module.next_temp();
        self.body.push(format!(
            "{kind_ptr} = getelementptr inbounds %pasta.pvalue, ptr %out, i32 0, i32 1"
        ));
        self.body.push(format!("store i32 {handle_kind}, ptr {kind_ptr}"));
    }

    fn emit_expr(&mut self, expr: &BootstrapExpr) -> CompiledValue {
        match expr {
            BootstrapExpr::Number(value) => CompiledValue {
                ty: ValueType::Number,
                operand: format_double(*value),
                owned: false,
            },
            BootstrapExpr::Bool(value) => CompiledValue {
                ty: ValueType::Bool,
                operand: if *value { "1".to_string() } else { "0".to_string() },
                owned: false,
            },
            BootstrapExpr::String(value) => CompiledValue {
                ty: ValueType::String,
                operand: self.module.emit_c_string_ptr(&mut self.body, value, None),
                owned: false,
            },
            BootstrapExpr::None => CompiledValue {
                ty: ValueType::AbiValue,
                operand: "zeroinitializer".to_string(),
                owned: false,
            },
            BootstrapExpr::Variable { name, ty } => {
                if let Some(slot) = self.locals.get(name).cloned() {
                    let temp = self.module.next_temp();
                    self.body.push(format!(
                        "{temp} = load {}, ptr {}",
                        llvm_type(ty),
                        slot.ptr
                    ));
                    CompiledValue {
                        ty: ty.clone(),
                        operand: temp,
                        owned: false,
                    }
                } else {
                    let temp = self.module.next_temp();
                    self.body.push(format!(
                        "{temp} = load {}, ptr {}",
                        llvm_type(ty),
                        global_symbol(name)
                    ));
                    CompiledValue {
                        ty: ty.clone(),
                        operand: temp,
                        owned: false,
                    }
                }
            }
            BootstrapExpr::Call { name, args, ty } => {
                let compiled_args = args.iter().map(|arg| self.emit_expr(arg)).collect::<Vec<_>>();
                let arg_sig = compiled_args
                    .iter()
                    .map(|arg| {
                        if arg.ty == ValueType::AbiValue {
                            let operand = if arg.owned {
                                arg.operand.clone()
                            } else {
                                self.emit_retain_dynamic_value(&arg.operand);
                                arg.operand.clone()
                            };
                            format!("{} {}", llvm_type(&arg.ty), operand)
                        } else {
                            format!("{} {}", llvm_type(&arg.ty), arg.operand)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let temp = self.module.next_temp();
                self.body.push(format!(
                    "{temp} = call {} {}({})",
                    llvm_type(ty),
                    function_symbol(name),
                    arg_sig
                ));
                CompiledValue {
                    ty: ty.clone(),
                    operand: temp,
                    owned: *ty == ValueType::AbiValue,
                }
            }
            BootstrapExpr::RuntimeCall { builtin, args, ty } => {
                self.module.needs_runtime_bridge = true;
                match ty {
                    ValueType::String => {
                        let compiled_args =
                            args.iter().map(|arg| self.emit_expr(arg)).collect::<Vec<_>>();
                        let arg_sig = compiled_args
                            .iter()
                            .map(|arg| format!("{} {}", llvm_type(&arg.ty), arg.operand))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let temp = self.module.next_temp();
                        self.body.push(format!(
                            "{temp} = call ptr {}({arg_sig})",
                            runtime_builtin_symbol(*builtin)
                        ));
                        CompiledValue {
                            ty: ValueType::String,
                            operand: temp,
                            owned: false,
                        }
                    }
                    ValueType::Bool => {
                        let compiled_args =
                            args.iter().map(|arg| self.emit_expr(arg)).collect::<Vec<_>>();
                        let arg_sig = compiled_args
                            .iter()
                            .map(|arg| format!("{} {}", llvm_type(&arg.ty), arg.operand))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let call = self.module.next_temp();
                        let temp = self.module.next_temp();
                        self.body.push(format!(
                            "{call} = call i32 {}({arg_sig})",
                            runtime_builtin_symbol(*builtin)
                        ));
                        self.body.push(format!("{temp} = icmp ne i32 {call}, 0"));
                        CompiledValue {
                            ty: ValueType::Bool,
                            operand: temp,
                            owned: false,
                        }
                    }
                    ValueType::Number => {
                        let compiled_args =
                            args.iter().map(|arg| self.emit_expr(arg)).collect::<Vec<_>>();
                        let temp = self.module.next_temp();
                        match builtin {
                            RuntimeBuiltin::ListLen => {
                                let value_ptr = self.emit_runtime_abi_arg_ptr(&compiled_args[0]);
                                self.body.push(format!(
                                    "{temp} = call double {}(ptr noundef {value_ptr})",
                                    runtime_builtin_symbol(*builtin)
                                ));
                            }
                            RuntimeBuiltin::ListIndexNumber => {
                                let value_ptr = self.emit_runtime_abi_arg_ptr(&compiled_args[0]);
                                self.body.push(format!(
                                    "{temp} = call double {}(ptr noundef {value_ptr}, double noundef {})",
                                    runtime_builtin_symbol(*builtin),
                                    compiled_args[1].operand
                                ));
                            }
                            RuntimeBuiltin::DictGetNumber => {
                                let value_ptr = self.emit_runtime_abi_arg_ptr(&compiled_args[0]);
                                self.body.push(format!(
                                    "{temp} = call double {}(ptr noundef {value_ptr}, ptr noundef {})",
                                    runtime_builtin_symbol(*builtin),
                                    compiled_args[1].operand
                                ));
                            }
                            _ => {
                                let arg_sig = compiled_args
                                    .iter()
                                    .map(|arg| format!("{} {}", llvm_type(&arg.ty), arg.operand))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                self.body.push(format!(
                                    "{temp} = call double {}({arg_sig})",
                                    runtime_builtin_symbol(*builtin)
                                ));
                            }
                        }
                        CompiledValue {
                            ty: ValueType::Number,
                            operand: temp,
                            owned: false,
                        }
                    }
                    ValueType::AbiValue => {
                        let compiled_args =
                            args.iter().map(|arg| self.emit_expr(arg)).collect::<Vec<_>>();
                        let out_ptr = self.module.next_temp();
                        let temp = self.module.next_temp();
                        self.body.push(format!("{out_ptr} = alloca %pasta.pvalue"));
                        self.body
                            .push(format!("store %pasta.pvalue zeroinitializer, ptr {out_ptr}"));
                        match builtin {
                            RuntimeBuiltin::ListSlice => {
                                let value_ptr = self.emit_runtime_abi_arg_ptr(&compiled_args[0]);
                                self.body.push(format!(
                                    "call void {}(ptr noundef {value_ptr}, double noundef {}, double noundef {}, ptr noundef {out_ptr})",
                                    runtime_builtin_symbol(*builtin),
                                    compiled_args[1].operand,
                                    compiled_args[2].operand
                                ));
                            }
                            RuntimeBuiltin::ListConcat => {
                                let left_ptr = self.emit_runtime_abi_arg_ptr(&compiled_args[0]);
                                let right_ptr = self.emit_runtime_abi_arg_ptr(&compiled_args[1]);
                                self.body.push(format!(
                                    "call void {}(ptr noundef {left_ptr}, ptr noundef {right_ptr}, ptr noundef {out_ptr})",
                                    runtime_builtin_symbol(*builtin)
                                ));
                            }
                            _ => {
                                panic!("unsupported ABI runtime expression builtin: {builtin:?}");
                            }
                        }
                        self.body
                            .push(format!("{temp} = load %pasta.pvalue, ptr {out_ptr}"));
                        CompiledValue {
                            ty: ValueType::AbiValue,
                            operand: temp,
                            owned: true,
                        }
                    }
                }
            }
            BootstrapExpr::ListNumberLiteral { items } => {
                self.module.needs_runtime_bridge = true;
                let len = items.len();
                let array_ptr = self.module.next_temp();
                self.body
                    .push(format!("{array_ptr} = alloca [{} x double]", len));
                for (index, item) in items.iter().enumerate() {
                    let value = self.emit_expr(item);
                    let item_ptr = self.module.next_temp();
                    self.body.push(format!(
                        "{item_ptr} = getelementptr inbounds [{} x double], ptr {array_ptr}, i32 0, i32 {}",
                        len, index
                    ));
                    self.body
                        .push(format!("store double {}, ptr {item_ptr}", value.operand));
                }
                let temp = self.module.next_temp();
                let out_ptr = self.module.next_temp();
                self.body.push(format!("{out_ptr} = alloca %pasta.pvalue"));
                self.body
                    .push(format!("store %pasta.pvalue zeroinitializer, ptr {out_ptr}"));
                self.body.push(format!(
                    "call void @pasta_rt_list_new_numbers(ptr noundef {array_ptr}, i64 noundef {}, ptr noundef {out_ptr})",
                    len
                ));
                self.body
                    .push(format!("{temp} = load %pasta.pvalue, ptr {out_ptr}"));
                CompiledValue {
                    ty: ValueType::AbiValue,
                    operand: temp,
                    owned: true,
                }
            }
            BootstrapExpr::DictStringNumberLiteral { pairs } => {
                self.module.needs_runtime_bridge = true;
                let len = pairs.len();
                let array_ptr = self.module.next_temp();
                self.body.push(format!(
                    "{array_ptr} = alloca [{} x %pasta.rt.dict_num_entry]",
                    len
                ));
                for (index, (key, value_expr)) in pairs.iter().enumerate() {
                    let key_ptr = self.module.emit_c_string_ptr(&mut self.body, key, None);
                    let value = self.emit_expr(value_expr);
                    let entry_ptr = self.module.next_temp();
                    self.body.push(format!(
                        "{entry_ptr} = getelementptr inbounds [{} x %pasta.rt.dict_num_entry], ptr {array_ptr}, i32 0, i32 {}",
                        len, index
                    ));
                    let key_field_ptr = self.module.next_temp();
                    self.body.push(format!(
                        "{key_field_ptr} = getelementptr inbounds %pasta.rt.dict_num_entry, ptr {entry_ptr}, i32 0, i32 0"
                    ));
                    self.body
                        .push(format!("store ptr {}, ptr {key_field_ptr}", key_ptr));
                    let value_field_ptr = self.module.next_temp();
                    self.body.push(format!(
                        "{value_field_ptr} = getelementptr inbounds %pasta.rt.dict_num_entry, ptr {entry_ptr}, i32 0, i32 1"
                    ));
                    self.body.push(format!(
                        "store double {}, ptr {value_field_ptr}",
                        value.operand
                    ));
                }
                let temp = self.module.next_temp();
                let out_ptr = self.module.next_temp();
                self.body.push(format!("{out_ptr} = alloca %pasta.pvalue"));
                self.body
                    .push(format!("store %pasta.pvalue zeroinitializer, ptr {out_ptr}"));
                self.body.push(format!(
                    "call void @pasta_rt_dict_new_string_number(ptr noundef {array_ptr}, i64 noundef {}, ptr noundef {out_ptr})",
                    len
                ));
                self.body
                    .push(format!("{temp} = load %pasta.pvalue, ptr {out_ptr}"));
                CompiledValue {
                    ty: ValueType::AbiValue,
                    operand: temp,
                    owned: true,
                }
            }
            BootstrapExpr::Color { r, g, b } => {
                let r_value = self.emit_expr(r);
                let g_value = self.emit_expr(g);
                let b_value = self.emit_expr(b);

                let r_i64 = self.module.next_temp();
                let r_i32 = self.module.next_temp();
                let r_mask = self.module.next_temp();
                let r_shift = self.module.next_temp();
                let g_i64 = self.module.next_temp();
                let g_i32 = self.module.next_temp();
                let g_mask = self.module.next_temp();
                let g_shift = self.module.next_temp();
                let b_i64 = self.module.next_temp();
                let b_i32 = self.module.next_temp();
                let b_mask = self.module.next_temp();
                let rg_or = self.module.next_temp();
                let rgb_or = self.module.next_temp();
                let argb = self.module.next_temp();
                let argb_i64 = self.module.next_temp();
                let packed = self.module.next_temp();

                self.body
                    .push(format!("{r_i64} = fptosi double {} to i64", r_value.operand));
                self.body.push(format!("{r_i32} = trunc i64 {r_i64} to i32"));
                self.body.push(format!("{r_mask} = and i32 {r_i32}, 255"));
                self.body.push(format!("{r_shift} = shl i32 {r_mask}, 16"));

                self.body
                    .push(format!("{g_i64} = fptosi double {} to i64", g_value.operand));
                self.body.push(format!("{g_i32} = trunc i64 {g_i64} to i32"));
                self.body.push(format!("{g_mask} = and i32 {g_i32}, 255"));
                self.body.push(format!("{g_shift} = shl i32 {g_mask}, 8"));

                self.body
                    .push(format!("{b_i64} = fptosi double {} to i64", b_value.operand));
                self.body.push(format!("{b_i32} = trunc i64 {b_i64} to i32"));
                self.body.push(format!("{b_mask} = and i32 {b_i32}, 255"));

                self.body.push(format!("{rg_or} = or i32 {r_shift}, {g_shift}"));
                self.body.push(format!("{rgb_or} = or i32 {rg_or}, {b_mask}"));
                self.body.push(format!("{argb} = or i32 4278190080, {rgb_or}"));
                self.body.push(format!("{argb_i64} = zext i32 {argb} to i64"));
                self.body
                    .push(format!("{packed} = uitofp i64 {argb_i64} to double"));

                CompiledValue {
                    ty: ValueType::Number,
                    operand: packed,
                    owned: false,
                }
            }
            BootstrapExpr::Binary {
                op,
                left,
                right,
                ty,
            } => {
                let lhs = self.emit_expr(left);
                let rhs = self.emit_expr(right);
                let temp = self.module.next_temp();
                match op {
                    BootstrapBinaryOp::Add => self.body.push(format!(
                        "{temp} = fadd double {}, {}",
                        lhs.operand, rhs.operand
                    )),
                    BootstrapBinaryOp::Sub => self.body.push(format!(
                        "{temp} = fsub double {}, {}",
                        lhs.operand, rhs.operand
                    )),
                    BootstrapBinaryOp::Mul => self.body.push(format!(
                        "{temp} = fmul double {}, {}",
                        lhs.operand, rhs.operand
                    )),
                    BootstrapBinaryOp::Div => self.body.push(format!(
                        "{temp} = fdiv double {}, {}",
                        lhs.operand, rhs.operand
                    )),
                    BootstrapBinaryOp::Mod => self.body.push(format!(
                        "{temp} = frem double {}, {}",
                        lhs.operand, rhs.operand
                    )),
                    BootstrapBinaryOp::Eq => match lhs.ty {
                        ValueType::Bool => self.body.push(format!(
                            "{temp} = icmp eq i1 {}, {}",
                            lhs.operand, rhs.operand
                        )),
                        ValueType::String => {
                            self.module.needs_strcmp = true;
                            let cmp = self.module.next_temp();
                            self.body.push(format!(
                                "{cmp} = call i32 @strcmp(ptr noundef {}, ptr noundef {})",
                                lhs.operand, rhs.operand
                            ));
                            self.body.push(format!("{temp} = icmp eq i32 {cmp}, 0"));
                        }
                        _ => self.body.push(format!(
                            "{temp} = fcmp oeq double {}, {}",
                            lhs.operand, rhs.operand
                        )),
                    },
                    BootstrapBinaryOp::Neq => match lhs.ty {
                        ValueType::Bool => self.body.push(format!(
                            "{temp} = icmp ne i1 {}, {}",
                            lhs.operand, rhs.operand
                        )),
                        ValueType::String => {
                            self.module.needs_strcmp = true;
                            let cmp = self.module.next_temp();
                            self.body.push(format!(
                                "{cmp} = call i32 @strcmp(ptr noundef {}, ptr noundef {})",
                                lhs.operand, rhs.operand
                            ));
                            self.body.push(format!("{temp} = icmp ne i32 {cmp}, 0"));
                        }
                        _ => self.body.push(format!(
                            "{temp} = fcmp one double {}, {}",
                            lhs.operand, rhs.operand
                        )),
                    },
                    BootstrapBinaryOp::Lt => self.body.push(format!(
                        "{temp} = fcmp olt double {}, {}",
                        lhs.operand, rhs.operand
                    )),
                    BootstrapBinaryOp::Gt => self.body.push(format!(
                        "{temp} = fcmp ogt double {}, {}",
                        lhs.operand, rhs.operand
                    )),
                    BootstrapBinaryOp::Lte => self.body.push(format!(
                        "{temp} = fcmp ole double {}, {}",
                        lhs.operand, rhs.operand
                    )),
                    BootstrapBinaryOp::Gte => self.body.push(format!(
                        "{temp} = fcmp oge double {}, {}",
                        lhs.operand, rhs.operand
                    )),
                    BootstrapBinaryOp::And => self.body.push(format!(
                        "{temp} = and i1 {}, {}",
                        lhs.operand, rhs.operand
                    )),
                    BootstrapBinaryOp::Or => self.body.push(format!(
                        "{temp} = or i1 {}, {}",
                        lhs.operand, rhs.operand
                    )),
                }
                CompiledValue {
                    ty: ty.clone(),
                    operand: temp,
                    owned: false,
                }
            }
            BootstrapExpr::Not(expr) => {
                let value = self.emit_expr(expr);
                let temp = self.module.next_temp();
                self.body.push(format!("{temp} = xor i1 {}, true", value.operand));
                CompiledValue {
                    ty: ValueType::Bool,
                    operand: temp,
                    owned: false,
                }
            }
        }
    }

    fn ensure_local_variable(&mut self, name: &str, ty: &ValueType) -> VariableSlot {
        if let Some(existing) = self.locals.get(name) {
            return existing.clone();
        }
        let ptr = format!("%{}.slot", sanitize_name(name));
        self.allocas.push(format!("{ptr} = alloca {}", llvm_type(ty)));
        if *ty == ValueType::AbiValue {
            self.entry_inits
                .push(format!("store %pasta.pvalue zeroinitializer, ptr {ptr}"));
        }
        let slot = VariableSlot {
            ptr,
            ty: ty.clone(),
        };
        self.locals.insert(name.to_string(), slot.clone());
        slot
    }

    fn ensure_dynamic_return_slot(&mut self) -> String {
        if let Some(slot) = &self.dynamic_return_slot {
            return slot.clone();
        }
        let slot = "%ret.value.slot".to_string();
        self.allocas.push(format!("{slot} = alloca %pasta.pvalue"));
        self.entry_inits
            .push(format!("store %pasta.pvalue zeroinitializer, ptr {slot}"));
        self.dynamic_return_slot = Some(slot.clone());
        slot
    }

    fn emit_retain_dynamic_value(&mut self, operand: &str) {
        self.module.needs_pasta_value_retain = true;
        let status = self.module.next_temp();
        self.body.push(format!(
            "{status} = call i32 @pasta_value_retain(%pasta.pvalue {operand})"
        ));
    }

    fn emit_release_dynamic_value(&mut self, operand: &str) {
        self.module.needs_pasta_value_release = true;
        let status = self.module.next_temp();
        self.body.push(format!(
            "{status} = call i32 @pasta_value_release(%pasta.pvalue {operand})"
        ));
    }

    fn emit_store_dynamic_slot(&mut self, slot: &VariableSlot, value: CompiledValue) {
        let stored = if value.owned {
            value.operand
        } else {
            self.emit_retain_dynamic_value(&value.operand);
            value.operand
        };
        let old = self.module.next_temp();
        self.body
            .push(format!("{old} = load %pasta.pvalue, ptr {}", slot.ptr));
        self.emit_release_dynamic_value(&old);
        self.body
            .push(format!("store %pasta.pvalue {stored}, ptr {}", slot.ptr));
    }

    fn emit_cleanup_dynamic_locals(&mut self) {
        let mut names = self.locals.keys().cloned().collect::<Vec<_>>();
        names.sort();
        for name in names {
            let slot = self.locals.get(&name).cloned().expect("local should exist");
            if slot.ty != ValueType::AbiValue {
                continue;
            }
            let loaded = self.module.next_temp();
            self.body
                .push(format!("{loaded} = load %pasta.pvalue, ptr {}", slot.ptr));
            self.emit_release_dynamic_value(&loaded);
        }
    }

    fn emit_dynamic_return(&mut self, value: CompiledValue) {
        let ret_slot = self.ensure_dynamic_return_slot();
        let returned = if value.owned {
            value.operand
        } else {
            self.emit_retain_dynamic_value(&value.operand);
            value.operand
        };
        self.body
            .push(format!("store %pasta.pvalue {returned}, ptr {ret_slot}"));
        self.emit_cleanup_dynamic_locals();
        let loaded = self.module.next_temp();
        self.body
            .push(format!("{loaded} = load %pasta.pvalue, ptr {ret_slot}"));
        self.body.push(format!("ret %pasta.pvalue {loaded}"));
        self.block_terminated = true;
    }

    fn emit_runtime_abi_arg_ptr(&mut self, value: &CompiledValue) -> String {
        debug_assert_eq!(value.ty, ValueType::AbiValue);
        let ptr = self.module.next_temp();
        self.body.push(format!("{ptr} = alloca %pasta.pvalue"));
        self.body
            .push(format!("store %pasta.pvalue {}, ptr {ptr}", value.operand));
        ptr
    }

    fn emit_runtime_statement_call(&mut self, builtin: RuntimeBuiltin, args: &[BootstrapExpr]) {
        self.module.needs_runtime_bridge = true;
        let compiled_args = args.iter().map(|arg| self.emit_expr(arg)).collect::<Vec<_>>();
        let arg_sig = compiled_args
            .iter()
            .map(|arg| format!("{} {}", llvm_type(&arg.ty), arg.operand))
            .collect::<Vec<_>>()
            .join(", ");
        self.body.push(format!(
            "call void {}({arg_sig})",
            runtime_builtin_symbol(builtin)
        ));
    }
}

fn global_symbol(name: &str) -> String {
    format!("@g.{}", sanitize_name(name))
}

fn module_init_symbol(name: &str) -> String {
    format!("@pasta_mod_{}_init", sanitize_name(name))
}

fn module_manifest_symbol(name: &str) -> String {
    format!("@pasta_mod_{}_manifest_json", sanitize_name(name))
}

fn module_manifest_global(name: &str) -> String {
    format!("@.module.manifest.{}", sanitize_name(name))
}

fn module_call_symbol(module_name: &str, export_name: &str) -> String {
    format!(
        "@pasta_mod_{}_call_{}",
        sanitize_name(module_name),
        sanitize_name(export_name)
    )
}

fn function_symbol(name: &str) -> String {
    format!("@pasta_fn_{}", sanitize_name(name))
}

fn runtime_builtin_symbol(builtin: RuntimeBuiltin) -> &'static str {
    match builtin {
        RuntimeBuiltin::WindowNew => "@pasta_rt_window_new",
        RuntimeBuiltin::WindowPoll => "@pasta_rt_window_poll",
        RuntimeBuiltin::WindowKey => "@pasta_rt_window_key",
        RuntimeBuiltin::WindowClose => "@pasta_rt_window_close",
        RuntimeBuiltin::SetDrawTarget => "@pasta_rt_set_draw_target",
        RuntimeBuiltin::SetColorPacked => "@pasta_rt_set_color_packed",
        RuntimeBuiltin::CanvasFillRect => "@pasta_rt_canvas_fill_rect",
        RuntimeBuiltin::SwapBuffer => "@pasta_rt_swap_buffer",
        RuntimeBuiltin::FpsInit => "@pasta_rt_fps_init",
        RuntimeBuiltin::FpsBegin => "@pasta_rt_fps_begin",
        RuntimeBuiltin::FpsEnd => "@pasta_rt_fps_end",
        RuntimeBuiltin::FpsTick => "@pasta_rt_fps_tick",
        RuntimeBuiltin::RandInt2 => "@pasta_rt_rand_int2",
        RuntimeBuiltin::ListLen => "@pasta_rt_list_len",
        RuntimeBuiltin::ListSlice => "@pasta_rt_list_slice",
        RuntimeBuiltin::ListConcat => "@pasta_rt_list_concat",
        RuntimeBuiltin::ListIndexNumber => "@pasta_rt_list_index_number",
        RuntimeBuiltin::DictGetNumber => "@pasta_rt_dict_get_number",
        RuntimeBuiltin::NumberToString => "@pasta_rt_number_to_string",
        RuntimeBuiltin::StringConcat => "@pasta_rt_string_concat",
    }
}

fn string_bytes(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    bytes
}

fn bytes_to_llvm_string(bytes: &[u8]) -> String {
    let mut out = String::new();
    for byte in bytes {
        match *byte {
            b'\\' => out.push_str("\\5C"),
            b'"' => out.push_str("\\22"),
            0x20..=0x7E => out.push(char::from(*byte)),
            _ => out.push_str(&format!("\\{:02X}", byte)),
        }
    }
    out
}

fn escape_metadata(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn llvm_type(ty: &ValueType) -> &'static str {
    match ty {
        ValueType::Number => "double",
        ValueType::Bool => "i1",
        ValueType::String => "ptr",
        ValueType::AbiValue => "%pasta.pvalue",
    }
}

fn llvm_zero_value(ty: &ValueType) -> &'static str {
    match ty {
        ValueType::Number => "0.0",
        ValueType::Bool => "false",
        ValueType::String => "null",
        ValueType::AbiValue => "zeroinitializer",
    }
}

fn sanitize_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "tmp".to_string()
    } else {
        out
    }
}

const PABI_STATUS_OK: i32 = 0;
const PABI_STATUS_NULL_OUT: i32 = 1;
const PABI_STATUS_NULL_INPUT: i32 = 2;
const PABI_STATUS_ARITY_MISMATCH: i32 = 8;
const PABI_STATUS_TYPE_MISMATCH: i32 = 9;

fn format_double(value: f64) -> String {
    if value == 0.0 {
        "0.0".to_string()
    } else {
        format!("{value:.17e}")
    }
}

fn render_module_manifest_json(module: &BootstrapModule) -> String {
    let exports = module
        .exports
        .iter()
        .map(|export| {
            let params = export
                .params
                .iter()
                .map(|ty| format!(r#""{}""#, value_type_name(ty)))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                r#"{{"name":"{}","arity":{},"params":[{}],"return_type":"{}"}}"#,
                escape_json(&export.name),
                export.params.len(),
                params,
                value_type_name(&export.return_type)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"module":"{}","abi_version":1,"call_abi":"pasta.module.v1","handle_ownership":"retain_release_refcounted","visibility":"exports_only","exports":[{}]}}"#,
        escape_json(&module.name),
        exports
    )
}

fn value_type_name(ty: &ValueType) -> &'static str {
    match ty {
        ValueType::Number => "number",
        ValueType::Bool => "bool",
        ValueType::String => "string",
        ValueType::AbiValue => "value",
    }
}

fn escape_json(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn program_uses_abi_values(program: &BootstrapProgram) -> bool {
    program
        .globals
        .iter()
        .any(|global| global.ty == ValueType::AbiValue)
        || program
            .functions
            .iter()
            .any(function_uses_abi_values)
        || program.main.iter().any(stmt_uses_abi_values)
}

fn function_uses_abi_values(function: &BootstrapFunction) -> bool {
    function.return_type == ValueType::AbiValue
        || function.params.iter().any(|param| param.ty == ValueType::AbiValue)
        || function.body.iter().any(stmt_uses_abi_values)
}

fn stmt_uses_abi_values(stmt: &BootstrapStmt) -> bool {
    match stmt {
        BootstrapStmt::Assign { value, ty, .. } => *ty == ValueType::AbiValue || expr_uses_abi_values(value),
        BootstrapStmt::If {
            condition,
            then_body,
            else_body,
        } => {
            expr_uses_abi_values(condition)
                || then_body.iter().any(stmt_uses_abi_values)
                || else_body
                    .as_ref()
                    .map(|body| body.iter().any(stmt_uses_abi_values))
                    .unwrap_or(false)
        }
        BootstrapStmt::While { condition, body } => {
            expr_uses_abi_values(condition) || body.iter().any(stmt_uses_abi_values)
        }
        BootstrapStmt::Print { value }
        | BootstrapStmt::Expr { value }
        | BootstrapStmt::Return { value } => expr_uses_abi_values(value),
        BootstrapStmt::RuntimeCall { args, .. } => args.iter().any(expr_uses_abi_values),
        BootstrapStmt::Break | BootstrapStmt::Continue => false,
    }
}

fn expr_uses_abi_values(expr: &BootstrapExpr) -> bool {
    match expr {
        BootstrapExpr::None => true,
        BootstrapExpr::Variable { ty, .. }
        | BootstrapExpr::Call { ty, .. }
        | BootstrapExpr::RuntimeCall { ty, .. } => *ty == ValueType::AbiValue,
        BootstrapExpr::ListNumberLiteral { .. } | BootstrapExpr::DictStringNumberLiteral { .. } => {
            true
        }
        BootstrapExpr::Binary { left, right, ty, .. } => {
            *ty == ValueType::AbiValue || expr_uses_abi_values(left) || expr_uses_abi_values(right)
        }
        BootstrapExpr::Color { r, g, b } => {
            expr_uses_abi_values(r) || expr_uses_abi_values(g) || expr_uses_abi_values(b)
        }
        BootstrapExpr::Not(inner) => expr_uses_abi_values(inner),
        BootstrapExpr::Number(_) | BootstrapExpr::Bool(_) | BootstrapExpr::String(_) => false,
    }
}
