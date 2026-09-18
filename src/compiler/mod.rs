//! Minimal native compiler entrypoints for Pasta.
//!
//! The bootstrap compiler intentionally grows in slices. This stage supports a
//! typed static subset with top-level code plus exact-arity `DEF` functions,
//! direct calls, structured `IF` / `WHILE` control flow, and explicit
//! `RET.NOW(...)` returns.

mod codegen_llvm;
mod error;
mod ir;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::lexer::lexer::Lexer;
use crate::parser::ast::{BinaryOp, Expr, Identifier, Program, Span, Statement};
use crate::parser::parser::{ParseError, Parser};

pub use error::CompilerError;

/// Current bootstrap compiler target triple.
pub const DEFAULT_TARGET_TRIPLE: &str = "x86_64-pc-linux-gnu";

const MAIN_SCOPE: &str = "__main__";

#[derive(Clone)]
struct RuntimeBuiltinInfo {
    builtin: ir::RuntimeBuiltin,
    params: &'static [ir::ValueType],
    return_type: Option<ir::ValueType>,
}

fn runtime_builtin_info(name: &str) -> Option<RuntimeBuiltinInfo> {
    use ir::RuntimeBuiltin as Builtin;
    use ir::ValueType as Ty;

    Some(match name.to_ascii_lowercase().as_str() {
        "window" | "window_create" => RuntimeBuiltinInfo {
            builtin: Builtin::WindowNew,
            params: &[Ty::String, Ty::Number, Ty::Number],
            return_type: Some(Ty::String),
        },
        "window_poll" => RuntimeBuiltinInfo {
            builtin: Builtin::WindowPoll,
            params: &[Ty::String],
            return_type: Some(Ty::Bool),
        },
        "window_key" => RuntimeBuiltinInfo {
            builtin: Builtin::WindowKey,
            params: &[Ty::String],
            return_type: Some(Ty::String),
        },
        "window_close" => RuntimeBuiltinInfo {
            builtin: Builtin::WindowClose,
            params: &[Ty::String],
            return_type: None,
        },
        "set_draw_target" => RuntimeBuiltinInfo {
            builtin: Builtin::SetDrawTarget,
            params: &[Ty::String],
            return_type: None,
        },
        "set_color" => RuntimeBuiltinInfo {
            builtin: Builtin::SetColorPacked,
            params: &[Ty::Number],
            return_type: None,
        },
        "canvas_fill_rect" => RuntimeBuiltinInfo {
            builtin: Builtin::CanvasFillRect,
            params: &[Ty::String, Ty::Number, Ty::Number, Ty::Number, Ty::Number],
            return_type: None,
        },
        "swap_buffer" => RuntimeBuiltinInfo {
            builtin: Builtin::SwapBuffer,
            params: &[Ty::String],
            return_type: None,
        },
        "fps_init" => RuntimeBuiltinInfo {
            builtin: Builtin::FpsInit,
            params: &[Ty::Number],
            return_type: None,
        },
        "fps_begin" => RuntimeBuiltinInfo {
            builtin: Builtin::FpsBegin,
            params: &[Ty::Number],
            return_type: None,
        },
        "fps_end" => RuntimeBuiltinInfo {
            builtin: Builtin::FpsEnd,
            params: &[],
            return_type: None,
        },
        "fps_tick" => RuntimeBuiltinInfo {
            builtin: Builtin::FpsTick,
            params: &[],
            return_type: None,
        },
        "rand.int" => RuntimeBuiltinInfo {
            builtin: Builtin::RandInt2,
            params: &[Ty::Number, Ty::Number],
            return_type: Some(Ty::Number),
        },
        "list_len" => RuntimeBuiltinInfo {
            builtin: Builtin::ListLen,
            params: &[Ty::AbiValue],
            return_type: Some(Ty::Number),
        },
        "list_slice" => RuntimeBuiltinInfo {
            builtin: Builtin::ListSlice,
            params: &[Ty::AbiValue, Ty::Number, Ty::Number],
            return_type: Some(Ty::AbiValue),
        },
        "dict_get" => RuntimeBuiltinInfo {
            builtin: Builtin::DictGetNumber,
            params: &[Ty::AbiValue, Ty::String],
            return_type: Some(Ty::Number),
        },
        _ => return None,
    })
}

fn runtime_builtin_arity_error(name: &str, expected_arity: usize) -> String {
    format!(
        "compiled builtin '{}' expects exactly {} argument(s)",
        name, expected_arity
    )
}

/// Lower Pasta source into LLVM IR for the current bootstrap compiler subset.
pub fn compile_source_to_llvm(source: &str, source_name: &str) -> Result<String, CompilerError> {
    let program = parse_program(source)?;
    let ir_program = lower_program(&program)?;
    Ok(codegen_llvm::emit_module(
        &ir_program,
        source_name,
        DEFAULT_TARGET_TRIPLE,
    ))
}

/// Compile Pasta source into a native executable via `llc` and `clang`.
pub fn compile_source_to_executable(
    source: &str,
    source_name: &str,
    output_path: &Path,
) -> Result<(), CompilerError> {
    let llvm_ir = compile_source_to_llvm(source, source_name)?;
    let ll_path = sidecar_path(output_path, "ll");
    let obj_path = sidecar_path(output_path, "o");
    let runtime_lib_dir = runtime_library_dir()?;

    fs::write(&ll_path, llvm_ir).map_err(|e| {
        CompilerError::tool(format!(
            "failed to write LLVM IR to '{}': {e}",
            ll_path.display()
        ))
    })?;

    let llc_status = Command::new("llc")
        .arg("-filetype=obj")
        .arg("-o")
        .arg(&obj_path)
        .arg(&ll_path)
        .output()
        .map_err(|e| CompilerError::tool(format!("failed to launch llc: {e}")))?;
    if !llc_status.status.success() {
        cleanup_sidecars(&[&ll_path, &obj_path]);
        return Err(CompilerError::tool(format!(
            "llc failed:\n{}",
            String::from_utf8_lossy(&llc_status.stderr).trim()
        )));
    }

    let clang_status = Command::new("clang")
        .arg("-no-pie")
        .arg("-o")
        .arg(output_path)
        .arg(&obj_path)
        .arg(format!("-L{}", runtime_lib_dir.display()))
        .arg("-lpasta")
        .arg(format!("-Wl,-rpath,{}", runtime_lib_dir.display()))
        .output()
        .map_err(|e| CompilerError::tool(format!("failed to launch clang: {e}")))?;
    if !clang_status.status.success() {
        cleanup_sidecars(&[&ll_path, &obj_path]);
        return Err(CompilerError::tool(format!(
            "clang failed:\n{}",
            String::from_utf8_lossy(&clang_status.stderr).trim()
        )));
    }

    cleanup_sidecars(&[&ll_path, &obj_path]);
    Ok(())
}

/// Compile Pasta module source into a native shared library via `llc` and `clang`.
pub fn compile_source_to_shared_library(
    source: &str,
    source_name: &str,
    output_path: &Path,
) -> Result<(), CompilerError> {
    let program = parse_program(source)?;
    let ir_program = lower_shared_module_program(&program)?;
    let llvm_ir = codegen_llvm::emit_module(&ir_program, source_name, DEFAULT_TARGET_TRIPLE);
    let ll_path = sidecar_path(output_path, "ll");
    let obj_path = sidecar_path(output_path, "o");

    fs::write(&ll_path, llvm_ir).map_err(|e| {
        CompilerError::tool(format!(
            "failed to write LLVM IR to '{}': {e}",
            ll_path.display()
        ))
    })?;

    let llc_status = Command::new("llc")
        .arg("-relocation-model=pic")
        .arg("-filetype=obj")
        .arg("-o")
        .arg(&obj_path)
        .arg(&ll_path)
        .output()
        .map_err(|e| CompilerError::tool(format!("failed to launch llc: {e}")))?;
    if !llc_status.status.success() {
        cleanup_sidecars(&[&ll_path, &obj_path]);
        return Err(CompilerError::tool(format!(
            "llc failed:\n{}",
            String::from_utf8_lossy(&llc_status.stderr).trim()
        )));
    }

    let clang_status = Command::new("clang")
        .arg("-shared")
        .arg("-o")
        .arg(output_path)
        .arg(&obj_path)
        .output()
        .map_err(|e| CompilerError::tool(format!("failed to launch clang: {e}")))?;
    if !clang_status.status.success() {
        cleanup_sidecars(&[&ll_path, &obj_path]);
        return Err(CompilerError::tool(format!(
            "clang failed:\n{}",
            String::from_utf8_lossy(&clang_status.stderr).trim()
        )));
    }

    cleanup_sidecars(&[&ll_path, &obj_path]);
    Ok(())
}

fn parse_program(source: &str) -> Result<Program, CompilerError> {
    let tokens = Lexer::new(source).lex();
    let mut parser = Parser::new(tokens);
    let (program, diagnostics) = parser.parse_with_diagnostics();
    if let Some(diag) = diagnostics.first() {
        return Err(parse_error(diag));
    }
    Ok(program)
}

fn parse_error(diag: &ParseError) -> CompilerError {
    CompilerError::unsupported(
        format!(
            "parse error at {}:{}: {}",
            diag.span.start_line, diag.span.start_col, diag.message
        ),
        Some(diag.span.clone()),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TypeVar(usize);

#[derive(Default)]
struct TypeSystem {
    parent: Vec<usize>,
    concrete: Vec<Option<ir::ValueType>>,
}

impl TypeSystem {
    fn new_var(&mut self) -> TypeVar {
        let id = self.parent.len();
        self.parent.push(id);
        self.concrete.push(None);
        TypeVar(id)
    }

    fn fresh_concrete(&mut self, ty: ir::ValueType) -> TypeVar {
        let var = self.new_var();
        self.unify_with_concrete(var, ty, None).expect("fresh type should accept concrete");
        var
    }

    fn find(&mut self, var: TypeVar) -> usize {
        let idx = var.0;
        if self.parent[idx] != idx {
            let root = self.find(TypeVar(self.parent[idx]));
            self.parent[idx] = root;
        }
        self.parent[idx]
    }

    fn unify(&mut self, left: TypeVar, right: TypeVar, span: Option<Span>) -> Result<(), CompilerError> {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root == right_root {
            return Ok(());
        }

        match (
            self.concrete[left_root].clone(),
            self.concrete[right_root].clone(),
        ) {
            (Some(a), Some(b)) if a != b => Err(CompilerError::unsupported(
                format!("type mismatch: {:?} vs {:?}", a, b),
                span,
            )),
            (lhs, rhs) => {
                self.parent[right_root] = left_root;
                self.concrete[left_root] = lhs.or(rhs);
                Ok(())
            }
        }
    }

    fn unify_with_concrete(
        &mut self,
        var: TypeVar,
        ty: ir::ValueType,
        span: Option<Span>,
    ) -> Result<(), CompilerError> {
        let root = self.find(var);
        match self.concrete[root].clone() {
            Some(existing) if existing != ty => Err(CompilerError::unsupported(
                format!("type mismatch: {:?} vs {:?}", existing, ty),
                span,
            )),
            _ => {
                self.concrete[root] = Some(ty);
                Ok(())
            }
        }
    }

    fn resolve(&mut self, var: TypeVar) -> Option<ir::ValueType> {
        let root = self.find(var);
        self.concrete[root].clone()
    }
}

struct FunctionInfo {
    params: Vec<Identifier>,
    body: Vec<Statement>,
    param_vars: Vec<TypeVar>,
    return_var: TypeVar,
    has_return: bool,
}

enum PostCheck {
    EqComparable { var: TypeVar, span: Span },
}

struct AnalysisContext {
    types: TypeSystem,
    globals: HashMap<String, TypeVar>,
    functions: HashMap<String, FunctionInfo>,
    call_graph: HashMap<String, HashSet<String>>,
    post_checks: Vec<PostCheck>,
}

impl AnalysisContext {
    fn new(program: &Program) -> Result<Self, CompilerError> {
        let mut types = TypeSystem::default();
        let mut functions = HashMap::new();

        for statement in &program.statements {
            if let Statement::FunctionDef { name, params, body, span } = statement {
                if functions.contains_key(&name.name) {
                    return Err(CompilerError::unsupported(
                        format!("duplicate compiled function '{}'", name.name),
                        Some(span.clone()),
                    ));
                }
                let param_vars = params.iter().map(|_| types.new_var()).collect();
                let return_var = types.new_var();
                functions.insert(
                    name.name.clone(),
                    FunctionInfo {
                        params: params.clone(),
                        body: body.clone(),
                        param_vars,
                        return_var,
                        has_return: false,
                    },
                );
            }
        }

        Ok(Self {
            types,
            globals: HashMap::new(),
            functions,
            call_graph: HashMap::new(),
            post_checks: Vec::new(),
        })
    }

    fn record_call(&mut self, caller: &str, callee: &str) {
        self.call_graph
            .entry(caller.to_string())
            .or_default()
            .insert(callee.to_string());
    }

    fn verify_post_checks(&mut self) -> Result<(), CompilerError> {
        for check in &self.post_checks {
            match check {
                PostCheck::EqComparable { var, span } => {
                    let ty = self.types.resolve(*var).ok_or_else(|| {
                        CompilerError::unsupported(
                            "could not infer operand type for == / !=".to_string(),
                            Some(span.clone()),
                        )
                    })?;
                    if !matches!(
                        ty,
                        ir::ValueType::Number | ir::ValueType::Bool | ir::ValueType::String
                    ) {
                        return Err(CompilerError::unsupported(
                            "bootstrap compiler only supports == and != on numbers, booleans, or strings",
                            Some(span.clone()),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

fn lower_program(program: &Program) -> Result<ir::BootstrapProgram, CompilerError> {
    build_lowered_program(program, None)
}

#[derive(Clone)]
struct SharedModuleSpec {
    name: String,
    body: Program,
    exports: Vec<Identifier>,
}

fn lower_shared_module_program(program: &Program) -> Result<ir::BootstrapProgram, CompilerError> {
    let module = extract_shared_module_spec(program)?;
    let body = module.body.clone();
    build_lowered_program(&body, Some(module))
}

fn extract_shared_module_spec(program: &Program) -> Result<SharedModuleSpec, CompilerError> {
    let mut module_decl = None;
    for statement in &program.statements {
        match statement {
            Statement::ModuleDecl {
                name,
                exports,
                body,
                span: _,
            } => {
                if module_decl.is_some() {
                    return Err(CompilerError::unsupported(
                        "shared-library compilation currently supports exactly one top-level MOD declaration",
                        Some(statement_span(statement)),
                    ));
                }
                module_decl = Some(SharedModuleSpec {
                    name: name.name.clone(),
                    body: Program::new(body.clone()),
                    exports: exports.clone(),
                });
            }
            other => {
                return Err(CompilerError::unsupported(
                    "shared-library compilation currently requires a single top-level MOD declaration",
                    Some(statement_span(other)),
                ));
            }
        }
    }
    module_decl.ok_or_else(|| {
        CompilerError::unsupported(
            "shared-library compilation requires a top-level MOD declaration",
            None,
        )
    })
}

fn build_lowered_program(
    program: &Program,
    shared_module: Option<SharedModuleSpec>,
) -> Result<ir::BootstrapProgram, CompilerError> {
    let mut analysis = AnalysisContext::new(program)?;
    let mut main_env = HashMap::new();
    analyze_statements(program, &mut analysis, &mut main_env, MAIN_SCOPE, true)?;

    let function_names: Vec<String> = analysis.functions.keys().cloned().collect();
    for name in &function_names {
        analyze_function_body(&mut analysis, name)?;
    }
    analysis.verify_post_checks()?;

    let reachable = compute_reachable(
        &analysis.call_graph,
        shared_module
            .as_ref()
            .map(|module| {
                module
                    .exports
                    .iter()
                    .map(|export| export.name.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    );
    let signatures = resolve_function_signatures(&mut analysis, &reachable, shared_module.is_some())?;
    let global_types = resolve_global_types(&mut analysis)?;

    let mut globals = global_types
        .iter()
        .map(|(name, ty)| ir::BootstrapGlobal {
            name: name.clone(),
            ty: ty.clone(),
        })
        .collect::<Vec<_>>();
    globals.sort_by(|a, b| a.name.cmp(&b.name));

    let main = lower_main_statements(program, &global_types, &signatures)?;

    let mut functions = Vec::new();
    let mut reachable_names = reachable.into_iter().collect::<Vec<_>>();
    reachable_names.sort();
    for name in reachable_names {
        if name == MAIN_SCOPE {
            continue;
        }
        let info = analysis
            .functions
            .get(&name)
            .ok_or_else(|| CompilerError::unsupported(format!("unknown function '{}'", name), None))?;
        let signature = signatures
            .get(&name)
            .ok_or_else(|| CompilerError::unsupported(format!("missing signature for function '{}'", name), None))?;
        let body = lower_function_body(&name, info, &global_types, &signatures)?;
        functions.push(ir::BootstrapFunction {
            name: name.clone(),
            params: info
                .params
                .iter()
                .zip(signature.params.iter())
                .map(|(param, ty)| ir::BootstrapParam {
                    name: param.name.clone(),
                    ty: ty.clone(),
                })
                .collect(),
            return_type: signature.return_type.clone(),
            body,
        });
    }

    let module = if let Some(module) = shared_module {
        let mut exports = Vec::new();
        for export in module.exports {
            let signature = signatures.get(&export.name).ok_or_else(|| {
                CompilerError::unsupported(
                    format!(
                        "shared module export '{}' must be a reachable compiled function",
                        export.name
                    ),
                    Some(export.span.clone()),
                )
            })?;
            exports.push(ir::BootstrapExport {
                name: export.name.clone(),
                params: signature.params.clone(),
                return_type: signature.return_type.clone(),
            });
        }
        Some(ir::BootstrapModule {
            name: module.name,
            exports,
        })
    } else {
        None
    };

    Ok(ir::BootstrapProgram {
        globals,
        functions,
        module,
        main,
    })
}

#[derive(Clone)]
struct FunctionSignature {
    params: Vec<ir::ValueType>,
    return_type: ir::ValueType,
}

fn resolve_global_types(
    analysis: &mut AnalysisContext,
) -> Result<HashMap<String, ir::ValueType>, CompilerError> {
    let mut resolved = HashMap::new();
    for (name, var) in &analysis.globals {
        let ty = analysis.types.resolve(*var).ok_or_else(|| {
            CompilerError::unsupported(
                format!("could not infer type for global '{}'", name),
                None,
            )
        })?;
        resolved.insert(name.clone(), ty);
    }
    Ok(resolved)
}

fn resolve_function_signatures(
    analysis: &mut AnalysisContext,
    reachable: &HashSet<String>,
    allow_dynamic_fallback: bool,
) -> Result<HashMap<String, FunctionSignature>, CompilerError> {
    let mut signatures = HashMap::new();
    for name in reachable {
        if name == MAIN_SCOPE {
            continue;
        }
        let info = analysis.functions.get(name).ok_or_else(|| {
            CompilerError::unsupported(format!("unknown function '{}'", name), None)
        })?;
        if info.has_return && !statement_list_guarantees_return(&info.body) {
            return Err(CompilerError::unsupported(
                format!(
                    "compiled function '{}' must RET.NOW(...) on every control-flow path",
                    name
                ),
                None,
            ));
        }
        let mut params = Vec::new();
        for (param, var) in info.params.iter().zip(info.param_vars.iter()) {
            let ty = match analysis.types.resolve(*var) {
                Some(ty) => ty,
                None if allow_dynamic_fallback => ir::ValueType::AbiValue,
                None => {
                    return Err(CompilerError::unsupported(
                        format!(
                            "could not infer compiled parameter type for '{}({})'",
                            name, param.name
                        ),
                        Some(param.span.clone()),
                    ))
                }
            };
            params.push(ty);
        }
        let return_type = match analysis.types.resolve(info.return_var) {
            Some(ty) => ty,
            None if !info.has_return => ir::ValueType::AbiValue,
            None if allow_dynamic_fallback => ir::ValueType::AbiValue,
            None => {
                return Err(CompilerError::unsupported(
                    format!("could not infer compiled return type for '{}'", name),
                    None,
                ))
            }
        };
        signatures.insert(name.clone(), FunctionSignature { params, return_type });
    }
    Ok(signatures)
}

fn compute_reachable(
    call_graph: &HashMap<String, HashSet<String>>,
    additional_roots: Vec<String>,
) -> HashSet<String> {
    let mut reachable = HashSet::new();
    let mut stack = vec![MAIN_SCOPE.to_string()];
    stack.extend(additional_roots);
    while let Some(name) = stack.pop() {
        if !reachable.insert(name.clone()) {
            continue;
        }
        if let Some(callees) = call_graph.get(&name) {
            for callee in callees {
                stack.push(callee.clone());
            }
        }
    }
    reachable
}

fn analyze_function_body(analysis: &mut AnalysisContext, function_name: &str) -> Result<(), CompilerError> {
    let (params, param_vars, body) = {
        let info = analysis.functions.get(function_name).ok_or_else(|| {
            CompilerError::unsupported(format!("unknown function '{}'", function_name), None)
        })?;
        (info.params.clone(), info.param_vars.clone(), info.body.clone())
    };

    let mut env = HashMap::new();
    for (param, var) in params.iter().zip(param_vars.iter()) {
        env.insert(param.name.clone(), *var);
    }

    analyze_stmt_list(&body, analysis, &mut env, function_name, false, 0)?;
    Ok(())
}

fn analyze_statements(
    program: &Program,
    analysis: &mut AnalysisContext,
    env: &mut HashMap<String, TypeVar>,
    scope_name: &str,
    is_main: bool,
) -> Result<(), CompilerError> {
    analyze_stmt_list(&program.statements, analysis, env, scope_name, is_main, 0)
}

fn analyze_stmt_list(
    statements: &[Statement],
    analysis: &mut AnalysisContext,
    env: &mut HashMap<String, TypeVar>,
    scope_name: &str,
    is_main: bool,
    loop_depth: usize,
) -> Result<(), CompilerError> {
    for statement in statements {
        match statement {
            Statement::FunctionDef { .. } if is_main => {}
            Statement::Assignment { target, value, .. }
            | Statement::ConstAssignment { target, value, .. } => {
                let expr_ty = analyze_expr(value, analysis, env, scope_name)?;
                let target_var = if is_main {
                    *analysis
                        .globals
                        .entry(target.name.clone())
                        .or_insert_with(|| analysis.types.new_var())
                } else {
                    *env.entry(target.name.clone())
                        .or_insert_with(|| analysis.types.new_var())
                };
                analysis
                    .types
                    .unify(target_var, expr_ty, Some(target.span.clone()))?;
            }
            Statement::Print { expr, .. } => {
                analyze_expr(expr, analysis, env, scope_name)?;
            }
            Statement::ExprStmt { expr, .. } => {
                analyze_expr(expr, analysis, env, scope_name)?;
            }
            Statement::If {
                conditions,
                then_body,
                else_body,
                scope_modifier,
                span,
            } => {
                if scope_modifier.is_some() {
                    return Err(CompilerError::unsupported(
                        "compiled control flow does not yet support BIND_SCOPE / UNBIND_SCOPE modifiers",
                        Some(span.clone()),
                    ));
                }
                if conditions.is_empty() {
                    return Err(CompilerError::unsupported(
                        "compiled IF requires at least one condition",
                        Some(span.clone()),
                    ));
                }
                for condition in conditions {
                    let cond_ty = analyze_expr(condition, analysis, env, scope_name)?;
                    analysis.types.unify_with_concrete(
                        cond_ty,
                        ir::ValueType::Bool,
                        Some(condition.span()),
                    )?;
                }
                analyze_stmt_list(then_body, analysis, env, scope_name, is_main, loop_depth)?;
                if let Some(else_body) = else_body {
                    analyze_stmt_list(else_body, analysis, env, scope_name, is_main, loop_depth)?;
                }
            }
            Statement::WhileBlock {
                targets,
                alias,
                condition,
                body,
                scope_modifier,
                span,
            } => {
                if scope_modifier.is_some() {
                    return Err(CompilerError::unsupported(
                        "compiled control flow does not yet support BIND_SCOPE / UNBIND_SCOPE modifiers",
                        Some(span.clone()),
                    ));
                }
                if !targets.is_empty() || alias.is_some() {
                    return Err(CompilerError::unsupported(
                        "compiled mode currently only supports plain WHILE <cond>: loops",
                        Some(span.clone()),
                    ));
                }
                let cond_ty = analyze_expr(condition, analysis, env, scope_name)?;
                analysis.types.unify_with_concrete(
                    cond_ty,
                    ir::ValueType::Bool,
                    Some(condition.span()),
                )?;
                analyze_stmt_list(body, analysis, env, scope_name, is_main, loop_depth + 1)?;
            }
            Statement::Break { span } | Statement::Continue { span } => {
                if loop_depth == 0 {
                    return Err(CompilerError::unsupported(
                        "BREAK and CONTINUE are only supported inside compiled WHILE loops",
                        Some(span.clone()),
                    ));
                }
                break;
            }
            Statement::RetNow { value, span } => {
                if is_main {
                    return Err(CompilerError::unsupported(
                        "top-level RET.NOW is not supported in compiled mode",
                        Some(span.clone()),
                    ));
                }
                let expr_ty = analyze_expr(value, analysis, env, scope_name)?;
                let info = analysis.functions.get_mut(scope_name).ok_or_else(|| {
                    CompilerError::unsupported(
                        format!("unknown function '{}'", scope_name),
                        Some(span.clone()),
                    )
                })?;
                info.has_return = true;
                analysis
                    .types
                    .unify(info.return_var, expr_ty, Some(span.clone()))?;
                break;
            }
            other => {
                return Err(CompilerError::unsupported(
                    format!(
                        "bootstrap compiler does not yet support statement form {:?}",
                        other
                    ),
                    Some(statement_span(other)),
                ));
            }
        }
    }
    Ok(())
}

fn analyze_expr(
    expr: &Expr,
    analysis: &mut AnalysisContext,
    env: &HashMap<String, TypeVar>,
    scope_name: &str,
) -> Result<TypeVar, CompilerError> {
    match expr {
        Expr::Number(_, _) => Ok(analysis.types.fresh_concrete(ir::ValueType::Number)),
        Expr::Bool(_, _) => Ok(analysis.types.fresh_concrete(ir::ValueType::Bool)),
        Expr::String(_, _) => Ok(analysis.types.fresh_concrete(ir::ValueType::String)),
        Expr::None(_) => Ok(analysis.types.fresh_concrete(ir::ValueType::AbiValue)),
        Expr::Identifier(id) => env
            .get(&id.name)
            .copied()
            .or_else(|| analysis.globals.get(&id.name).copied())
            .ok_or_else(|| {
                if analysis.functions.contains_key(&id.name) {
                    CompilerError::unsupported(
                        format!(
                            "passing functions as first-class values is not yet supported in compiled mode ('{}')",
                            id.name
                        ),
                        Some(id.span.clone()),
                    )
                } else {
                    CompilerError::unsupported(
                        format!(
                            "bootstrap compiler requires variables to be assigned before use: '{}'",
                            id.name
                        ),
                        Some(id.span.clone()),
                    )
                }
            }),
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => {
            if matches!(op, BinaryOp::Not) {
                let rhs = analyze_expr(right, analysis, env, scope_name)?;
                analysis.types.unify_with_concrete(
                    rhs,
                    ir::ValueType::Bool,
                    Some(span.clone()),
                )?;
                return Ok(analysis.types.fresh_concrete(ir::ValueType::Bool));
            }

            let lhs = analyze_expr(left, analysis, env, scope_name)?;
            let rhs = analyze_expr(right, analysis, env, scope_name)?;
            let op = ir::BootstrapBinaryOp::try_from(op).map_err(|_| {
                CompilerError::unsupported(
                    format!("bootstrap compiler does not support binary operator '{}'", op),
                    Some(span.clone()),
                )
            })?;

            use ir::BootstrapBinaryOp as Op;
            match op {
                Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod => {
                    if op == Op::Add {
                        let lhs_resolved = analysis.types.resolve(lhs);
                        let rhs_resolved = analysis.types.resolve(rhs);
                        if lhs_resolved == Some(ir::ValueType::String)
                            && rhs_resolved == Some(ir::ValueType::String)
                        {
                            analysis.types.unify_with_concrete(
                                lhs,
                                ir::ValueType::String,
                                Some(span.clone()),
                            )?;
                            analysis.types.unify_with_concrete(
                                rhs,
                                ir::ValueType::String,
                                Some(span.clone()),
                            )?;
                            Ok(analysis.types.fresh_concrete(ir::ValueType::String))
                        } else if lhs_resolved == Some(ir::ValueType::AbiValue)
                            && rhs_resolved == Some(ir::ValueType::AbiValue)
                        {
                            analysis.types.unify_with_concrete(
                                lhs,
                                ir::ValueType::AbiValue,
                                Some(span.clone()),
                            )?;
                            analysis.types.unify_with_concrete(
                                rhs,
                                ir::ValueType::AbiValue,
                                Some(span.clone()),
                            )?;
                            Ok(analysis.types.fresh_concrete(ir::ValueType::AbiValue))
                        } else {
                            analysis.types.unify_with_concrete(
                                lhs,
                                ir::ValueType::Number,
                                Some(span.clone()),
                            )?;
                            analysis.types.unify_with_concrete(
                                rhs,
                                ir::ValueType::Number,
                                Some(span.clone()),
                            )?;
                            Ok(analysis.types.fresh_concrete(ir::ValueType::Number))
                        }
                    } else {
                        analysis.types.unify_with_concrete(
                            lhs,
                            ir::ValueType::Number,
                            Some(span.clone()),
                        )?;
                        analysis.types.unify_with_concrete(
                            rhs,
                            ir::ValueType::Number,
                            Some(span.clone()),
                        )?;
                        Ok(analysis.types.fresh_concrete(ir::ValueType::Number))
                    }
                }
                Op::Lt | Op::Gt | Op::Lte | Op::Gte => {
                    analysis.types.unify_with_concrete(
                        lhs,
                        ir::ValueType::Number,
                        Some(span.clone()),
                    )?;
                    analysis.types.unify_with_concrete(
                        rhs,
                        ir::ValueType::Number,
                        Some(span.clone()),
                    )?;
                    Ok(analysis.types.fresh_concrete(ir::ValueType::Bool))
                }
                Op::Eq | Op::Neq => {
                    analysis.types.unify(lhs, rhs, Some(span.clone()))?;
                    analysis
                        .post_checks
                        .push(PostCheck::EqComparable { var: lhs, span: span.clone() });
                    Ok(analysis.types.fresh_concrete(ir::ValueType::Bool))
                }
                Op::And | Op::Or => {
                    analysis.types.unify_with_concrete(
                        lhs,
                        ir::ValueType::Bool,
                        Some(span.clone()),
                    )?;
                    analysis.types.unify_with_concrete(
                        rhs,
                        ir::ValueType::Bool,
                        Some(span.clone()),
                    )?;
                    Ok(analysis.types.fresh_concrete(ir::ValueType::Bool))
                }
            }
        }
        Expr::Call { callee, args, span } => {
            let callee_name = if let Expr::Identifier(id) = &**callee {
                id.name.clone()
            } else {
                return Err(CompilerError::unsupported(
                    "compiled mode currently only supports direct named function calls",
                    Some(span.clone()),
                ));
            };

            if callee_name.eq_ignore_ascii_case("color") {
                if args.len() != 3 {
                    return Err(CompilerError::unsupported(
                        "compiled builtin 'color' expects exactly 3 argument(s)",
                        Some(span.clone()),
                    ));
                }
                for arg in args {
                    let arg_ty = analyze_expr(arg, analysis, env, scope_name)?;
                    analysis.types.unify_with_concrete(
                        arg_ty,
                        ir::ValueType::Number,
                        Some(arg.span()),
                    )?;
                }
                return Ok(analysis.types.fresh_concrete(ir::ValueType::Number));
            }

            if callee_name.eq_ignore_ascii_case("str") {
                if args.len() != 1 {
                    return Err(CompilerError::unsupported(
                        "compiled builtin 'str' expects exactly 1 argument(s)",
                        Some(span.clone()),
                    ));
                }
                let arg_ty = analyze_expr(&args[0], analysis, env, scope_name)?;
                let resolved = analysis.types.resolve(arg_ty);
                match resolved {
                    Some(ir::ValueType::Number) | Some(ir::ValueType::String) | None => {
                        if resolved.is_none() {
                            analysis.types.unify_with_concrete(
                                arg_ty,
                                ir::ValueType::Number,
                                Some(args[0].span()),
                            )?;
                        }
                        return Ok(analysis.types.fresh_concrete(ir::ValueType::String));
                    }
                    Some(other) => {
                        return Err(CompilerError::unsupported(
                            format!("compiled builtin 'str' does not yet support {:?}", other),
                            Some(span.clone()),
                        ))
                    }
                }
            }

            if let Some(info) = runtime_builtin_info(&callee_name) {
                if args.len() != info.params.len() {
                    return Err(CompilerError::unsupported(
                        runtime_builtin_arity_error(&callee_name, info.params.len()),
                        Some(span.clone()),
                    ));
                }
                for (arg, param_ty) in args.iter().zip(info.params.iter()) {
                    let arg_ty = analyze_expr(arg, analysis, env, scope_name)?;
                    analysis
                        .types
                        .unify_with_concrete(arg_ty, param_ty.clone(), Some(arg.span()))?;
                }
                return Ok(match info.return_type {
                    Some(return_type) => analysis.types.fresh_concrete(return_type),
                    None => analysis.types.fresh_concrete(ir::ValueType::AbiValue),
                });
            }

            let (param_vars, return_var, expected_arity) = {
                let info = analysis.functions.get(&callee_name).ok_or_else(|| {
                    CompilerError::unsupported(
                        format!("unknown compiled function '{}'", callee_name),
                        Some(span.clone()),
                    )
                })?;
                (info.param_vars.clone(), info.return_var, info.params.len())
            };

            if args.len() != expected_arity {
                return Err(CompilerError::unsupported(
                    format!(
                        "compiled function '{}' expects exactly {} argument(s), got {}",
                        callee_name,
                        expected_arity,
                        args.len()
                    ),
                    Some(span.clone()),
                ));
            }

            for (arg, param_var) in args.iter().zip(param_vars.iter()) {
                let arg_ty = analyze_expr(arg, analysis, env, scope_name)?;
                analysis
                    .types
                    .unify(*param_var, arg_ty, Some(arg.span()))?;
            }

            analysis.record_call(scope_name, &callee_name);
            Ok(return_var)
        }
        Expr::List { items, .. } => {
            for item in items {
                let item_ty = analyze_expr(item, analysis, env, scope_name)?;
                analysis.types.unify_with_concrete(
                    item_ty,
                    ir::ValueType::Number,
                    Some(item.span()),
                )?;
            }
            Ok(analysis.types.fresh_concrete(ir::ValueType::AbiValue))
        }
        Expr::Dict { pairs, .. } => {
            for (key, value) in pairs {
                let key_ty = analyze_expr(key, analysis, env, scope_name)?;
                analysis.types.unify_with_concrete(
                    key_ty,
                    ir::ValueType::String,
                    Some(key.span()),
                )?;
                let value_ty = analyze_expr(value, analysis, env, scope_name)?;
                analysis.types.unify_with_concrete(
                    value_ty,
                    ir::ValueType::Number,
                    Some(value.span()),
                )?;
            }
            Ok(analysis.types.fresh_concrete(ir::ValueType::AbiValue))
        }
        Expr::Index { base, indices, span } => {
            if indices.len() != 1 {
                return Err(CompilerError::unsupported(
                    "compiled indexing currently supports exactly one index",
                    Some(span.clone()),
                ));
            }
            let base_ty = analyze_expr(base, analysis, env, scope_name)?;
            analysis.types.unify_with_concrete(
                base_ty,
                ir::ValueType::AbiValue,
                Some(base.span()),
            )?;
            let index_ty = analyze_expr(&indices[0], analysis, env, scope_name)?;
            analysis.types.unify_with_concrete(
                index_ty,
                ir::ValueType::Number,
                Some(indices[0].span()),
            )?;
            Ok(analysis.types.fresh_concrete(ir::ValueType::Number))
        }
        other => Err(CompilerError::unsupported(
            format!(
                "bootstrap compiler does not yet support expression form {:?}",
                other
            ),
            Some(other.span()),
        )),
    }
}

fn lower_main_statements(
    program: &Program,
    globals: &HashMap<String, ir::ValueType>,
    signatures: &HashMap<String, FunctionSignature>,
) -> Result<Vec<ir::BootstrapStmt>, CompilerError> {
    let mut env = globals.clone();
    lower_stmt_list(&program.statements, &mut env, globals, signatures, true, 0)
}

fn lower_function_body(
    function_name: &str,
    info: &FunctionInfo,
    globals: &HashMap<String, ir::ValueType>,
    signatures: &HashMap<String, FunctionSignature>,
) -> Result<Vec<ir::BootstrapStmt>, CompilerError> {
    let mut env = HashMap::new();
    let signature = signatures.get(function_name).ok_or_else(|| {
        CompilerError::unsupported(
            format!("missing signature for function '{}'", function_name),
            None,
        )
    })?;
    for (param, sig_ty) in info.params.iter().zip(signature.params.iter()) {
        env.insert(param.name.clone(), sig_ty.clone());
    }
    lower_stmt_list(&info.body, &mut env, globals, signatures, false, 0)
}

fn lower_stmt_list(
    statements: &[Statement],
    env: &mut HashMap<String, ir::ValueType>,
    globals: &HashMap<String, ir::ValueType>,
    signatures: &HashMap<String, FunctionSignature>,
    is_main: bool,
    loop_depth: usize,
) -> Result<Vec<ir::BootstrapStmt>, CompilerError> {
    let mut lowered = Vec::new();
    for statement in statements {
        match statement {
            Statement::FunctionDef { .. } if is_main => {}
            Statement::Assignment { target, value, .. }
            | Statement::ConstAssignment { target, value, .. } => {
                let lowered_value = lower_expr(value, env, globals, signatures)?;
                let ty = lowered_value.ty();
                if is_main {
                    env.insert(target.name.clone(), ty.clone());
                } else {
                    env.entry(target.name.clone()).or_insert_with(|| ty.clone());
                }
                lowered.push(ir::BootstrapStmt::Assign {
                    name: target.name.clone(),
                    value: lowered_value,
                    ty,
                });
            }
            Statement::If {
                conditions,
                then_body,
                else_body,
                scope_modifier,
                span,
            } => {
                if scope_modifier.is_some() {
                    return Err(CompilerError::unsupported(
                        "compiled control flow does not yet support BIND_SCOPE / UNBIND_SCOPE modifiers",
                        Some(span.clone()),
                    ));
                }
                lowered.push(ir::BootstrapStmt::If {
                    condition: lower_condition_expr(conditions, env, globals, signatures, span)?,
                    then_body: lower_stmt_list(
                        then_body,
                        env,
                        globals,
                        signatures,
                        is_main,
                        loop_depth,
                    )?,
                    else_body: else_body
                        .as_ref()
                        .map(|body| {
                            lower_stmt_list(body, env, globals, signatures, is_main, loop_depth)
                        })
                        .transpose()?,
                });
            }
            Statement::WhileBlock {
                targets,
                alias,
                condition,
                body,
                scope_modifier,
                span,
            } => {
                if scope_modifier.is_some() {
                    return Err(CompilerError::unsupported(
                        "compiled control flow does not yet support BIND_SCOPE / UNBIND_SCOPE modifiers",
                        Some(span.clone()),
                    ));
                }
                if !targets.is_empty() || alias.is_some() {
                    return Err(CompilerError::unsupported(
                        "compiled mode currently only supports plain WHILE <cond>: loops",
                        Some(span.clone()),
                    ));
                }
                lowered.push(ir::BootstrapStmt::While {
                    condition: lower_expr(condition, env, globals, signatures)?,
                    body: lower_stmt_list(
                        body,
                        env,
                        globals,
                        signatures,
                        is_main,
                        loop_depth + 1,
                    )?,
                });
            }
            Statement::Break { span } => {
                if loop_depth == 0 {
                    return Err(CompilerError::unsupported(
                        "BREAK and CONTINUE are only supported inside compiled WHILE loops",
                        Some(span.clone()),
                    ));
                }
                lowered.push(ir::BootstrapStmt::Break);
                break;
            }
            Statement::Continue { span } => {
                if loop_depth == 0 {
                    return Err(CompilerError::unsupported(
                        "BREAK and CONTINUE are only supported inside compiled WHILE loops",
                        Some(span.clone()),
                    ));
                }
                lowered.push(ir::BootstrapStmt::Continue);
                break;
            }
            Statement::Print { expr, .. } => {
                let value = lower_expr(expr, env, globals, signatures)?;
                if value.ty() == ir::ValueType::AbiValue {
                    return Err(CompilerError::unsupported(
                        "compiled PRINT does not yet support opaque ABI values".to_string(),
                        Some(expr.span()),
                    ));
                }
                lowered.push(ir::BootstrapStmt::Print { value });
            }
            Statement::ExprStmt { expr, .. } => {
                if let Expr::Call { callee, args, span } = expr {
                    if let Expr::Identifier(id) = &**callee {
                        if let Some(info) = runtime_builtin_info(&id.name) {
                            if info.return_type.is_none() {
                                if args.len() != info.params.len() {
                                    return Err(CompilerError::unsupported(
                                        runtime_builtin_arity_error(&id.name, info.params.len()),
                                        Some(span.clone()),
                                    ));
                                }
                                let lowered_args = args
                                    .iter()
                                    .map(|arg| lower_expr(arg, env, globals, signatures))
                                    .collect::<Result<Vec<_>, _>>()?;
                                for (arg, param_ty) in lowered_args.iter().zip(info.params.iter()) {
                                    if &arg.ty() != param_ty {
                                        return Err(CompilerError::unsupported(
                                            format!(
                                                "compiled builtin '{}' argument type mismatch: expected {:?}, got {:?}",
                                                id.name,
                                                param_ty,
                                                arg.ty()
                                            ),
                                            Some(span.clone()),
                                        ));
                                    }
                                }
                                lowered.push(ir::BootstrapStmt::RuntimeCall {
                                    builtin: info.builtin,
                                    args: lowered_args,
                                });
                                continue;
                            }
                        }
                    }
                }
                lowered.push(ir::BootstrapStmt::Expr {
                    value: lower_expr(expr, env, globals, signatures)?,
                });
            }
            Statement::RetNow { value, .. } => {
                lowered.push(ir::BootstrapStmt::Return {
                    value: lower_expr(value, env, globals, signatures)?,
                });
                break;
            }
            other => {
                return Err(CompilerError::unsupported(
                    format!(
                        "bootstrap compiler does not yet support statement form {:?}",
                        other
                    ),
                    Some(statement_span(other)),
                ));
            }
        }
    }
    Ok(lowered)
}

fn lower_condition_expr(
    conditions: &[Expr],
    env: &HashMap<String, ir::ValueType>,
    globals: &HashMap<String, ir::ValueType>,
    signatures: &HashMap<String, FunctionSignature>,
    span: &Span,
) -> Result<ir::BootstrapExpr, CompilerError> {
    let mut iter = conditions.iter();
    let first = iter.next().ok_or_else(|| {
        CompilerError::unsupported(
            "compiled IF requires at least one condition".to_string(),
            Some(span.clone()),
        )
    })?;
    let mut combined = lower_expr(first, env, globals, signatures)?;
    if combined.ty() != ir::ValueType::Bool {
        return Err(CompilerError::unsupported(
            "compiled IF conditions must be booleans",
            Some(first.span()),
        ));
    }
    for condition in iter {
        let lowered = lower_expr(condition, env, globals, signatures)?;
        if lowered.ty() != ir::ValueType::Bool {
            return Err(CompilerError::unsupported(
                "compiled IF conditions must be booleans",
                Some(condition.span()),
            ));
        }
        combined = ir::BootstrapExpr::Binary {
            op: ir::BootstrapBinaryOp::Or,
            left: Box::new(combined),
            right: Box::new(lowered),
            ty: ir::ValueType::Bool,
        };
    }
    Ok(combined)
}

fn lower_expr(
    expr: &Expr,
    env: &HashMap<String, ir::ValueType>,
    globals: &HashMap<String, ir::ValueType>,
    signatures: &HashMap<String, FunctionSignature>,
) -> Result<ir::BootstrapExpr, CompilerError> {
    match expr {
        Expr::Number(value, _) => Ok(ir::BootstrapExpr::Number(*value)),
        Expr::Bool(value, _) => Ok(ir::BootstrapExpr::Bool(*value)),
        Expr::String(value, _) => Ok(ir::BootstrapExpr::String(value.clone())),
        Expr::None(_) => Ok(ir::BootstrapExpr::None),
        Expr::Identifier(id) => {
            let ty = env
                .get(&id.name)
                .or_else(|| globals.get(&id.name))
                .cloned()
                .ok_or_else(|| {
                    CompilerError::unsupported(
                        format!(
                            "bootstrap compiler requires variables to be assigned before use: '{}'",
                            id.name
                        ),
                        Some(id.span.clone()),
                    )
                })?;
            Ok(ir::BootstrapExpr::Variable {
                name: id.name.clone(),
                ty,
            })
        }
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => {
            if matches!(op, BinaryOp::Not) {
                let lowered = lower_expr(right, env, globals, signatures)?;
                if lowered.ty() != ir::ValueType::Bool {
                    return Err(CompilerError::unsupported(
                        "bootstrap compiler only supports NOT on booleans",
                        Some(span.clone()),
                    ));
                }
                return Ok(ir::BootstrapExpr::Not(Box::new(lowered)));
            }

            let lowered_left = lower_expr(left, env, globals, signatures)?;
            let lowered_right = lower_expr(right, env, globals, signatures)?;
            let lowered_op = ir::BootstrapBinaryOp::try_from(op).map_err(|_| {
                CompilerError::unsupported(
                    format!("bootstrap compiler does not support binary operator '{}'", op),
                    Some(span.clone()),
                )
            })?;
            let ty = infer_binary_type(&lowered_op, &lowered_left, &lowered_right, span)?;

            if lowered_op == ir::BootstrapBinaryOp::Add && ty == ir::ValueType::AbiValue {
                return Ok(ir::BootstrapExpr::RuntimeCall {
                    builtin: ir::RuntimeBuiltin::ListConcat,
                    args: vec![lowered_left, lowered_right],
                    ty,
                });
            }
            if lowered_op == ir::BootstrapBinaryOp::Add && ty == ir::ValueType::String {
                return Ok(ir::BootstrapExpr::RuntimeCall {
                    builtin: ir::RuntimeBuiltin::StringConcat,
                    args: vec![lowered_left, lowered_right],
                    ty,
                });
            }

            Ok(ir::BootstrapExpr::Binary {
                op: lowered_op,
                left: Box::new(lowered_left),
                right: Box::new(lowered_right),
                ty,
            })
        }
        Expr::Call { callee, args, span } => {
            let name = if let Expr::Identifier(id) = &**callee {
                id.name.clone()
            } else {
                return Err(CompilerError::unsupported(
                    "compiled mode currently only supports direct named function calls",
                    Some(span.clone()),
                ));
            };
            if name.eq_ignore_ascii_case("color") {
                if args.len() != 3 {
                    return Err(CompilerError::unsupported(
                        "compiled builtin 'color' expects exactly 3 argument(s)",
                        Some(span.clone()),
                    ));
                }
                let lowered_args = args
                    .iter()
                    .map(|arg| lower_expr(arg, env, globals, signatures))
                    .collect::<Result<Vec<_>, _>>()?;
                if lowered_args.iter().any(|arg| arg.ty() != ir::ValueType::Number) {
                    return Err(CompilerError::unsupported(
                        "compiled builtin 'color' currently only supports numeric arguments",
                        Some(span.clone()),
                    ));
                }
                return Ok(ir::BootstrapExpr::Color {
                    r: Box::new(lowered_args[0].clone()),
                    g: Box::new(lowered_args[1].clone()),
                    b: Box::new(lowered_args[2].clone()),
                });
            }
            if name.eq_ignore_ascii_case("str") {
                if args.len() != 1 {
                    return Err(CompilerError::unsupported(
                        "compiled builtin 'str' expects exactly 1 argument(s)",
                        Some(span.clone()),
                    ));
                }
                let lowered = lower_expr(&args[0], env, globals, signatures)?;
                return match lowered.ty() {
                    ir::ValueType::String => Ok(lowered),
                    ir::ValueType::Number => Ok(ir::BootstrapExpr::RuntimeCall {
                        builtin: ir::RuntimeBuiltin::NumberToString,
                        args: vec![lowered],
                        ty: ir::ValueType::String,
                    }),
                    other => Err(CompilerError::unsupported(
                        format!("compiled builtin 'str' does not yet support {:?}", other),
                        Some(span.clone()),
                    )),
                };
            }
            if let Some(info) = runtime_builtin_info(&name) {
                let return_type = info.return_type.ok_or_else(|| {
                    CompilerError::unsupported(
                        format!(
                            "compiled builtin '{}' is only supported as a statement right now",
                            name
                        ),
                        Some(span.clone()),
                    )
                })?;
                if args.len() != info.params.len() {
                    return Err(CompilerError::unsupported(
                        runtime_builtin_arity_error(&name, info.params.len()),
                        Some(span.clone()),
                    ));
                }
                let lowered_args = args
                    .iter()
                    .map(|arg| lower_expr(arg, env, globals, signatures))
                    .collect::<Result<Vec<_>, _>>()?;
                for (arg, param_ty) in lowered_args.iter().zip(info.params.iter()) {
                    if &arg.ty() != param_ty {
                        return Err(CompilerError::unsupported(
                            format!(
                                "compiled builtin '{}' argument type mismatch: expected {:?}, got {:?}",
                                name,
                                param_ty,
                                arg.ty()
                            ),
                            Some(span.clone()),
                        ));
                    }
                }
                return Ok(ir::BootstrapExpr::RuntimeCall {
                    builtin: info.builtin,
                    args: lowered_args,
                    ty: return_type,
                });
            }
            let signature = signatures.get(&name).ok_or_else(|| {
                CompilerError::unsupported(
                    format!("unknown compiled function '{}'", name),
                    Some(span.clone()),
                )
            })?;
            if args.len() != signature.params.len() {
                return Err(CompilerError::unsupported(
                    format!(
                        "compiled function '{}' expects exactly {} argument(s), got {}",
                        name,
                        signature.params.len(),
                        args.len()
                    ),
                    Some(span.clone()),
                ));
            }
            let lowered_args = args
                .iter()
                .map(|arg| lower_expr(arg, env, globals, signatures))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ir::BootstrapExpr::Call {
                name,
                args: lowered_args,
                ty: signature.return_type.clone(),
            })
        }
        Expr::List { items, span } => {
            let lowered_items = items
                .iter()
                .map(|item| lower_expr(item, env, globals, signatures))
                .collect::<Result<Vec<_>, _>>()?;
            if lowered_items
                .iter()
                .any(|item| item.ty() != ir::ValueType::Number)
            {
                return Err(CompilerError::unsupported(
                    "compiled list literals currently only support numeric items",
                    Some(span.clone()),
                ));
            }
            Ok(ir::BootstrapExpr::ListNumberLiteral {
                items: lowered_items,
            })
        }
        Expr::Dict { pairs, span: _ } => {
            let mut lowered_pairs = Vec::with_capacity(pairs.len());
            for (key, value) in pairs {
                let key = match key {
                    Expr::String(text, _) => text.clone(),
                    _ => {
                        return Err(CompilerError::unsupported(
                            "compiled dict literals currently require string-literal keys",
                            Some(key.span()),
                        ))
                    }
                };
                let lowered_value = lower_expr(value, env, globals, signatures)?;
                if lowered_value.ty() != ir::ValueType::Number {
                    return Err(CompilerError::unsupported(
                        "compiled dict literals currently only support numeric values",
                        Some(value.span()),
                    ));
                }
                lowered_pairs.push((key, lowered_value));
            }
            Ok(ir::BootstrapExpr::DictStringNumberLiteral {
                pairs: lowered_pairs,
            })
        }
        Expr::Index { base, indices, span } => {
            if indices.len() != 1 {
                return Err(CompilerError::unsupported(
                    "compiled indexing currently supports exactly one index",
                    Some(span.clone()),
                ));
            }
            let lowered_base = lower_expr(base, env, globals, signatures)?;
            let lowered_index = lower_expr(&indices[0], env, globals, signatures)?;
            if lowered_base.ty() != ir::ValueType::AbiValue
                || lowered_index.ty() != ir::ValueType::Number
            {
                return Err(CompilerError::unsupported(
                    "compiled indexing currently supports list[index] with numeric index",
                    Some(span.clone()),
                ));
            }
            Ok(ir::BootstrapExpr::RuntimeCall {
                builtin: ir::RuntimeBuiltin::ListIndexNumber,
                args: vec![lowered_base, lowered_index],
                ty: ir::ValueType::Number,
            })
        }
        other => Err(CompilerError::unsupported(
            format!(
                "bootstrap compiler does not yet support expression form {:?}",
                other
            ),
            Some(other.span()),
        )),
    }
}

fn infer_binary_type(
    op: &ir::BootstrapBinaryOp,
    left: &ir::BootstrapExpr,
    right: &ir::BootstrapExpr,
    span: &Span,
) -> Result<ir::ValueType, CompilerError> {
    use ir::BootstrapBinaryOp as Op;
    use ir::ValueType as Ty;

    let left_ty = left.ty();
    let right_ty = right.ty();

    match op {
        Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod => {
            if left_ty == Ty::Number && right_ty == Ty::Number {
                Ok(Ty::Number)
            } else if *op == Op::Add && left_ty == Ty::String && right_ty == Ty::String {
                Ok(Ty::String)
            } else if *op == Op::Add && left_ty == Ty::AbiValue && right_ty == Ty::AbiValue {
                Ok(Ty::AbiValue)
            } else {
                Err(CompilerError::unsupported(
                    format!(
                        "bootstrap compiler only supports numeric '{}' expressions, string concatenation, or list concatenation",
                        render_bootstrap_op(op)
                    ),
                    Some(span.clone()),
                ))
            }
        }
        Op::Lt | Op::Gt | Op::Lte | Op::Gte => {
            if left_ty == Ty::Number && right_ty == Ty::Number {
                Ok(Ty::Bool)
            } else {
                Err(CompilerError::unsupported(
                    format!(
                        "bootstrap compiler only supports numeric '{}' comparisons",
                        render_bootstrap_op(op)
                    ),
                    Some(span.clone()),
                ))
            }
        }
        Op::Eq | Op::Neq => {
            if left_ty == right_ty && matches!(left_ty, Ty::Number | Ty::Bool | Ty::String) {
                Ok(Ty::Bool)
            } else {
                Err(CompilerError::unsupported(
                    "bootstrap compiler only supports == and != on numbers, booleans, or strings",
                    Some(span.clone()),
                ))
            }
        }
        Op::And | Op::Or => {
            if left_ty == Ty::Bool && right_ty == Ty::Bool {
                Ok(Ty::Bool)
            } else {
                Err(CompilerError::unsupported(
                    format!(
                        "bootstrap compiler only supports boolean '{}' expressions",
                        render_bootstrap_op(op)
                    ),
                    Some(span.clone()),
                ))
            }
        }
    }
}

fn render_bootstrap_op(op: &ir::BootstrapBinaryOp) -> &'static str {
    use ir::BootstrapBinaryOp as Op;

    match op {
        Op::Add => "+",
        Op::Sub => "-",
        Op::Mul => "*",
        Op::Div => "/",
        Op::Mod => "%",
        Op::Eq => "==",
        Op::Neq => "!=",
        Op::Lt => "<",
        Op::Gt => ">",
        Op::Lte => "<=",
        Op::Gte => ">=",
        Op::And => "AND",
        Op::Or => "OR",
    }
}

fn statement_list_guarantees_return(statements: &[Statement]) -> bool {
    for statement in statements {
        match statement {
            Statement::RetNow { .. } => return true,
            Statement::If {
                then_body,
                else_body: Some(else_body),
                ..
            } if statement_list_guarantees_return(then_body)
                && statement_list_guarantees_return(else_body) =>
            {
                return true;
            }
            Statement::Break { .. } | Statement::Continue { .. } => return false,
            _ => {}
        }
    }
    false
}

fn statement_span(statement: &Statement) -> Span {
    use crate::parser::ast::Statement::*;

    match statement {
        Assignment { span, .. }
        | ConstAssignment { span, .. }
        | MultiAssignment { span, .. }
        | IndexAssignment { span, .. }
        | ObjDecl { span, .. }
        | SpawnBlock { span, .. }
        | DoBlock { span, .. }
        | FunctionDef { span, .. }
        | WhileBlock { span, .. }
        | ForIn { span, .. }
        | ModuleDecl { span, .. }
        | Break { span, .. }
        | Continue { span, .. }
        | PriorityOverride { span, .. }
        | Constraint { span, .. }
        | ExprStmt { span, .. }
        | FromBlock { span, .. }
        | Print { span, .. }
        | If { span, .. }
        | End { span, .. }
        | RetNow { span, .. }
        | RetLate { span, .. }
        | AttemptBlock { span, .. }
        | TryBlock { span, .. }
        | LoopBlock { span, .. }
        | GotoLabel { span, .. }
        | GotoBlock { span, .. }
        | Pull { span, .. }
        | Push { span, .. }
        | Alloc { span, .. }
        | Free { span, .. }
        | Info { span, .. }
        | Seek { span, .. }
        | Swap { span, .. }
        | Other { span, .. }
        | UseUnsafe { span, .. }
        | WaitForThread { span, .. } => span.clone(),
        DefDoUntil(def) => def.span.clone(),
        ThreadHeader(h) => h.span.clone(),
        ParallelBlock(p) => p.span.clone(),
    }
}

fn sidecar_path(output_path: &Path, extension: &str) -> PathBuf {
    let mut path = output_path.to_path_buf();
    path.set_extension(extension);
    path
}

fn runtime_library_dir() -> Result<PathBuf, CompilerError> {
    let runtime_lib_name = runtime_library_filename();
    let project_root = detect_runtime_project_root()?;
    let build_status = Command::new("cargo")
        .arg("build")
        .arg("--quiet")
        .arg("--lib")
        .current_dir(&project_root)
        .output()
        .map_err(|e| CompilerError::tool(format!("failed to launch cargo to build {runtime_lib_name}: {e}")))?;
    if !build_status.status.success() {
        return Err(CompilerError::tool(format!(
            "failed to build compiled runtime library:\n{}",
            String::from_utf8_lossy(&build_status.stderr).trim()
        )));
    }

    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| project_root.join("target"));
    let candidates = [
        target_dir.join("debug"),
        target_dir.join("release"),
        target_dir.join("debug").join("deps"),
        target_dir.join("release").join("deps"),
    ];
    for candidate in candidates {
        if candidate.join(runtime_lib_name).exists() {
            return Ok(candidate);
        }
    }

    Err(CompilerError::tool(format!(
        "compiled runtime library was not found under '{}'",
        target_dir.display()
    )))
}

fn detect_runtime_project_root() -> Result<PathBuf, CompilerError> {
    let mut candidates = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir);
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));

    for start in candidates {
        for dir in start.ancestors() {
            let dir = dir.to_path_buf();
            if dir.join("Cargo.toml").exists() && dir.join("src").is_dir() {
                return Ok(dir);
            }
        }
    }

    Err(CompilerError::tool(
        format!(
            "failed to locate a Cargo project root for building {}",
            runtime_library_filename()
        ),
    ))
}

fn runtime_library_filename() -> &'static str {
    #[cfg(windows)]
    {
        "pasta.dll"
    }
    #[cfg(target_os = "macos")]
    {
        "libpasta.dylib"
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "libpasta.so"
    }
}

fn cleanup_sidecars(paths: &[&Path]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}
