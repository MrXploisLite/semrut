use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::types::BasicType;
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
use inkwell::basic_block::BasicBlock;
use inkwell::AddressSpace;
use crate::mir::{MirProgram, MirFunction, MirStmt, MirTerminator, MirValue, MirType, MirBinOp, MirUnaryOp, MirPattern, MirMatchArm};
use thiserror::Error;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("LLVM error: {msg}")]
    LlvmError { msg: String },

    #[error("failed to emit binary: {msg}")]
    EmitError { msg: String },
}

type Result<T> = std::result::Result<T, CodegenError>;

pub struct LlvmModule {
    pub ir: String,
}

impl std::fmt::Display for LlvmModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.ir)
    }
}

// ─── Codegen State ────────────────────────────────────────

struct CodegenState<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    /// Map from MIR variable name to (LLVM alloca, LLVM type)
    allocas: HashMap<String, (PointerValue<'ctx>, inkwell::types::BasicTypeEnum<'ctx>)>,
    /// LLVM functions
    functions: HashMap<String, FunctionValue<'ctx>>,
    /// Map from MIR block ID to LLVM BasicBlock
    llvm_blocks: HashMap<usize, BasicBlock<'ctx>>,
}

impl<'ctx> CodegenState<'ctx> {
    fn new(context: &'ctx Context, module: Module<'ctx>, builder: Builder<'ctx>) -> Self {
        CodegenState {
            context,
            module,
            builder,
            allocas: HashMap::new(),
            functions: HashMap::new(),
            llvm_blocks: HashMap::new(),
        }
    }

    fn load_var(&self, name: &str) -> Result<BasicValueEnum<'ctx>> {
        let (alloca, ty) = self.allocas.get(name).ok_or_else(|| CodegenError::LlvmError {
            msg: format!("undefined variable '{}'", name),
        })?;
        let loaded = self.builder.build_load(*ty, *alloca, name)
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
        Ok(loaded)
    }

    fn store_var(&self, name: &str, value: BasicValueEnum<'ctx>) -> Result<()> {
        let (alloca, _) = self.allocas.get(name).ok_or_else(|| CodegenError::LlvmError {
            msg: format!("undefined variable '{}'", name),
        })?;
        self.builder.build_store(*alloca, value)
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
        Ok(())
    }

    fn create_var(&mut self, name: &str, value: BasicValueEnum<'ctx>) -> Result<()> {
        // If variable already exists, store to existing alloca
        if let Some((alloca, _)) = self.allocas.get(name) {
            self.builder.build_store(*alloca, value)
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
            return Ok(());
        }
        // Otherwise create new alloca
        let ty = value.get_type();
        let alloca = self.builder.build_alloca(ty, name)
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
        self.builder.build_store(alloca, value)
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
        self.allocas.insert(name.to_string(), (alloca, ty));
        Ok(())
    }
}

// ─── Type Mapping ─────────────────────────────────────────

fn mir_type_to_llvm<'ctx>(context: &'ctx Context, ty: &MirType) -> inkwell::types::BasicTypeEnum<'ctx> {
    match ty {
        MirType::Void => context.i8_type().into(), // placeholder — void handled at fn_type level
        MirType::I8 | MirType::U8 => context.i8_type().into(),
        MirType::I16 | MirType::U16 => context.i16_type().into(),
        MirType::I32 | MirType::U32 => context.i32_type().into(),
        MirType::I64 | MirType::U64 => context.i64_type().into(),
        MirType::I128 | MirType::U128 => context.i128_type().into(),
        MirType::F16 => context.f16_type().into(),
        MirType::F32 => context.f32_type().into(),
        MirType::F64 => context.f64_type().into(),
        MirType::Bool => context.bool_type().into(),
        MirType::Char => context.i32_type().into(),
        MirType::Str => context.ptr_type(AddressSpace::default()).into(),
        MirType::Ptr(_) => context.ptr_type(AddressSpace::default()).into(),
        MirType::Ref { .. } => context.ptr_type(AddressSpace::default()).into(),
        MirType::Array(inner, len) => {
            let elem = mir_type_to_llvm(context, inner);
            elem.array_type(*len as u32).into()
        }
        MirType::Slice(_) => context.ptr_type(AddressSpace::default()).into(),
        MirType::Struct(_) => context.i64_type().into(),
        MirType::Enum(_) => context.i64_type().into(),
        MirType::Generic(_, _) => context.i64_type().into(),
        MirType::GenericParam(_) => context.i64_type().into(), // placeholder — resolved during monomorphization
    }
}

fn basic_type_to_fn_type<'ctx>(_context: &'ctx Context, ret: inkwell::types::BasicTypeEnum<'ctx>, params: &[inkwell::types::BasicMetadataTypeEnum<'ctx>]) -> inkwell::types::FunctionType<'ctx> {
    match ret {
        inkwell::types::BasicTypeEnum::IntType(t) => t.fn_type(params, false),
        inkwell::types::BasicTypeEnum::FloatType(t) => t.fn_type(params, false),
        inkwell::types::BasicTypeEnum::PointerType(t) => t.fn_type(params, false),
        inkwell::types::BasicTypeEnum::ArrayType(t) => t.fn_type(params, false),
        inkwell::types::BasicTypeEnum::VectorType(t) => t.fn_type(params, false),
        inkwell::types::BasicTypeEnum::StructType(t) => t.fn_type(params, false),
        inkwell::types::BasicTypeEnum::ScalableVectorType(t) => t.fn_type(params, false),
    }
}

// ─── Main Codegen ─────────────────────────────────────────

pub fn codegen(mir: &MirProgram, _opt_level: u8) -> Result<LlvmModule> {
    let context = Context::create();
    let module = context.create_module("semrut");
    let builder = context.create_builder();

    let mut state = CodegenState::new(&context, module, builder);

    // Declare external C library functions
    declare_external_functions(&mut state);

    // Pre-declare all user functions (so forward references work)
    for func in &mir.functions {
        let param_types: Vec<inkwell::types::BasicMetadataTypeEnum> = func.params.iter()
            .map(|(_, ty)| mir_type_to_llvm(&context, ty).into())
            .collect();

        let fn_type = if func.ret_type == MirType::Void {
            context.void_type().fn_type(&param_types, false)
        } else {
            let ret_type = mir_type_to_llvm(&context, &func.ret_type);
            basic_type_to_fn_type(&context, ret_type, &param_types)
        };

        let llvm_func = state.module.add_function(&func.name, fn_type, None);
        state.functions.insert(func.name.clone(), llvm_func);
    }

    // Compile each function
    for func in &mir.functions {
        compile_function(&mut state, func)?;
    }

    let ir = state.module.print_to_string().to_string();
    Ok(LlvmModule { ir })
}

fn declare_external_functions(state: &mut CodegenState) {
    let context = state.context;
    let module = &state.module;
    let ptr_type = context.ptr_type(AddressSpace::default());

    // int printf(const char *format, ...)
    let printf_type = context.i32_type().fn_type(&[ptr_type.into()], true);
    module.add_function("printf", printf_type, None);

    // void *malloc(size_t)
    let malloc_type = ptr_type.fn_type(&[context.i64_type().into()], false);
    module.add_function("malloc", malloc_type, None);

    // void free(void *)
    let free_type = context.void_type().fn_type(&[ptr_type.into()], false);
    module.add_function("free", free_type, None);

    // void *memcpy(void *, const void *, size_t)
    let memcpy_type = ptr_type.fn_type(&[ptr_type.into(), ptr_type.into(), context.i64_type().into()], false);
    module.add_function("memcpy", memcpy_type, None);

    // void *memset(void *, int, size_t)
    let memset_type = ptr_type.fn_type(&[ptr_type.into(), context.i32_type().into(), context.i64_type().into()], false);
    module.add_function("memset", memset_type, None);
}

fn compile_function<'ctx>(state: &mut CodegenState<'ctx>, func: &MirFunction) -> Result<()> {
    let context = state.context;

    // Sort blocks by ID to ensure entry (bb0) is processed first
    let mut sorted_blocks = func.blocks.clone();
    sorted_blocks.sort_by_key(|b| b.id);

    let param_types: Vec<inkwell::types::BasicMetadataTypeEnum> = func.params.iter()
        .map(|(_, ty)| mir_type_to_llvm(context, ty).into())
        .collect();

    let ret_type = mir_type_to_llvm(context, &func.ret_type);

    let fn_type = if func.ret_type == MirType::Void {
        context.void_type().fn_type(&param_types, false)
    } else {
        basic_type_to_fn_type(context, ret_type, &param_types)
    };

    // Function already declared in pre-pass, just get it
    let llvm_func = state.functions.get(&func.name).copied()
        .or_else(|| state.module.get_function(&func.name))
        .expect("function should be declared");

    let entry = context.append_basic_block(llvm_func, "entry");
    state.builder.position_at_end(entry);
    state.llvm_blocks.insert(0, entry);

    // Pre-create all other LLVM basic blocks
    for block in &sorted_blocks {
        if block.id == 0 { continue; } // entry already created
        let bb = context.append_basic_block(llvm_func, &format!("bb{}", block.id));
        state.llvm_blocks.insert(block.id, bb);
    }

    // Alloca for params
    state.allocas.clear();
    for (i, (name, ty)) in func.params.iter().enumerate() {
        let param = llvm_func.get_nth_param(i as u32).unwrap();
        let llvm_ty = mir_type_to_llvm(context, ty);
        let alloca = state.builder.build_alloca(llvm_ty, name)
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
        state.builder.build_store(alloca, param)
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
        state.allocas.insert(name.clone(), (alloca, llvm_ty));
    }

    // Track compiled blocks to avoid infinite recursion on back-edges
    let mut compiled: HashSet<usize> = HashSet::new();

    // Compile blocks
    if let Some(block) = sorted_blocks.first() {
        compile_block(state, block, &sorted_blocks, &mut compiled, &func)?;
    }

    // Ensure function has a terminator
    if let Some(bb) = state.builder.get_insert_block() {
        if bb.get_terminator().is_none() {
            if func.ret_type == MirType::Void {
                state.builder.build_return(None)
                    .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
            }
        }
    }

    Ok(())
}

fn compile_block<'ctx>(
    state: &mut CodegenState<'ctx>,
    block: &crate::mir::MirBlock,
    all_blocks: &[crate::mir::MirBlock],
    compiled: &mut HashSet<usize>,
    func: &MirFunction,
) -> Result<()> {
    if compiled.contains(&block.id) {
        return Ok(()); // already compiled (back-edge)
    }
    compiled.insert(block.id);

    // Compile statements
    for stmt in &block.stmts {
        compile_stmt(state, stmt)?;
    }

    // Compile terminator
    match &block.terminator {
        MirTerminator::Return(value) => {
            match value {
                Some(v) => {
                    let val = resolve_value(state, v)?;
                    state.builder.build_return(Some(&val))
                        .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
                }
                None => {
                    // For non-void functions, return zero
                    if func.ret_type != MirType::Void {
                        let zero = mir_type_to_llvm(state.context, &func.ret_type);
                        let zero_val: BasicValueEnum<'ctx> = match zero {
                            inkwell::types::BasicTypeEnum::IntType(t) => t.const_zero().into(),
                            inkwell::types::BasicTypeEnum::FloatType(t) => t.const_zero().into(),
                            inkwell::types::BasicTypeEnum::PointerType(t) => t.const_zero().into(),
                            _ => state.context.i64_type().const_zero().into(),
                        };
                        state.builder.build_return(Some(&zero_val))
                            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
                    } else {
                        state.builder.build_return(None)
                            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
                    }
                }
            }
        }
        MirTerminator::Jump { target } => {
            if let Some(&target_bb) = state.llvm_blocks.get(target) {
                state.builder.build_unconditional_branch(target_bb)
                    .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
                // Only recurse if not already compiled
                if !compiled.contains(target) {
                    state.builder.position_at_end(target_bb);
                    if let Some(target_block) = all_blocks.iter().find(|b| b.id == *target) {
                        compile_block(state, target_block, all_blocks, compiled, func)?;
                    }
                }
            }
        }
        MirTerminator::Branch { cond, then_target, else_target } => {
            let cond_val = resolve_value(state, cond)?;
            let cond_int = match cond_val {
                BasicValueEnum::IntValue(v) => v,
                _ => return Err(CodegenError::LlvmError {
                    msg: "branch condition must be integer/bool".to_string(),
                }),
            };

            let then_bb = state.llvm_blocks.get(then_target).copied();
            let else_bb = state.llvm_blocks.get(else_target).copied();

            if let (Some(then_bb), Some(else_bb)) = (then_bb, else_bb) {
                state.builder.build_conditional_branch(cond_int, then_bb, else_bb)
                    .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;

                // Compile then block (only if not already compiled)
                if !compiled.contains(then_target) {
                    state.builder.position_at_end(then_bb);
                    if let Some(then_block) = all_blocks.iter().find(|b| b.id == *then_target) {
                        compile_block(state, then_block, all_blocks, compiled, func)?;
                    }
                }

                // Compile else block (only if not already compiled)
                if !compiled.contains(else_target) {
                    state.builder.position_at_end(else_bb);
                    if let Some(else_block) = all_blocks.iter().find(|b| b.id == *else_target) {
                        compile_block(state, else_block, all_blocks, compiled, func)?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn compile_stmt<'ctx>(state: &mut CodegenState<'ctx>, stmt: &MirStmt) -> Result<()> {
    match stmt {
        MirStmt::Assign { dest, value } => {
            let val = resolve_value(state, value)?;
            state.create_var(dest, val)?;
        }

        MirStmt::BinaryOp { dest, op, left, right } => {
            let l = resolve_value(state, left)?;
            let r = resolve_value(state, right)?;

            let result = match (l, r) {
                (BasicValueEnum::IntValue(lv), BasicValueEnum::IntValue(rv)) => {
                    BasicValueEnum::IntValue(compile_int_bin_op(state, op, lv, rv)?)
                }
                (BasicValueEnum::FloatValue(lv), BasicValueEnum::FloatValue(rv)) => {
                    compile_float_bin_op(state, op, lv, rv)?
                }
                _ => {
                    return Err(CodegenError::LlvmError {
                        msg: format!("invalid binary op types: left={:?}, right={:?}", l, r),
                    });
                }
            };

            state.create_var(dest, result)?;
        }

        MirStmt::UnaryOp { dest, op, operand } => {
            let val = resolve_value(state, operand)?;

            let result = match val {
                BasicValueEnum::IntValue(v) => {
                    match op {
                        MirUnaryOp::Neg => state.builder.build_int_neg(v, "negtmp")
                            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
                        MirUnaryOp::Not => state.builder.build_not(v, "nottmp")
                            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
                    }.into()
                }
                BasicValueEnum::FloatValue(v) => {
                    match op {
                        MirUnaryOp::Neg => state.builder.build_float_neg(v, "negtmp")
                            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
                        _ => return Err(CodegenError::LlvmError {
                            msg: "not on float not supported".to_string(),
                        }),
                    }.into()
                }
                _ => return Err(CodegenError::LlvmError {
                    msg: "invalid unary op type".to_string(),
                }),
            };

            state.create_var(dest, result)?;
        }

        MirStmt::Call { dest, func, args } => {
            let llvm_func = state.functions.get(func).copied().or_else(|| {
                state.module.get_function(func)
            }).ok_or_else(|| CodegenError::LlvmError {
                msg: format!("function '{}' not found", func),
            })?;

            let call_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args.iter()
                .map(|arg| resolve_value(state, arg).map(|v| v.into()))
                .collect::<Result<Vec<_>>>()?;

            let call = state.builder.build_call(llvm_func, &call_args, &format!("{}_call", func))
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;

            if let Some(dest_name) = dest {
                if let inkwell::values::ValueKind::Basic(ret_val) = call.try_as_basic_value() {
                    state.create_var(dest_name, ret_val)?;
                }
            }
        }

        MirStmt::Asm { instructions, outputs, inputs } => {
            // Build inline asm using context.create_inline_asm
            let asm_str = instructions.join("\n");

            // Build constraints string from outputs and inputs
            let mut constraints = Vec::new();
            let mut output_types: Vec<inkwell::types::BasicTypeEnum<'ctx>> = Vec::new();
            for (reg, _ty) in outputs {
                // Output constraint: "=r" for register output
                constraints.push("=r".to_string());
                // For now, assume i64 output
                output_types.push(state.context.i64_type().into());
            }
            for (_reg, _ty) in inputs {
                // Input constraint: "r" for register input
                constraints.push("r".to_string());
            }

            let constraints_str = constraints.join(",");

            // Build function type: (input_types...) -> output_type
            // If no outputs, return void
            let asm_fn_type = if output_types.is_empty() {
                state.context.void_type().fn_type(&[], false)
            } else {
                // Single output: return that type, no inputs for now
                output_types[0].fn_type(&[], false)
            };

            let asm_ptr = state.context.create_inline_asm(
                asm_fn_type,
                asm_str,
                constraints_str,
                true,  // sideeffects
                false, // alignstack
                Some(inkwell::InlineAsmDialect::Intel),
                false, // can_throw
            );

            let call = state.builder.build_indirect_call(
                asm_fn_type,
                asm_ptr,
                &[],
                "asm_call",
            ).map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;

            // Store output to destination variable
            if let Some((dest, _)) = outputs.first() {
                if let inkwell::values::ValueKind::Basic(ret_val) = call.try_as_basic_value() {
                    state.create_var(dest, ret_val)?;
                }
            }
        }

        MirStmt::Cast { dest, value, target_ty } => {
            let val = resolve_value(state, value)?;
            let target_llvm = mir_type_to_llvm(state.context, target_ty);

            let casted = match val {
                BasicValueEnum::IntValue(v) => {
                    match target_llvm {
                        inkwell::types::BasicTypeEnum::IntType(t) => {
                            state.builder.build_int_cast(v, t, "casttmp")
                                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into()
                        }
                        inkwell::types::BasicTypeEnum::FloatType(t) => {
                            state.builder.build_unsigned_int_to_float(v, t, "casttmp")
                                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into()
                        }
                        _ => val,
                    }
                }
                BasicValueEnum::FloatValue(v) => {
                    match target_llvm {
                        inkwell::types::BasicTypeEnum::IntType(t) => {
                            state.builder.build_float_to_unsigned_int(v, t, "casttmp")
                                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into()
                        }
                        inkwell::types::BasicTypeEnum::FloatType(t) => {
                            state.builder.build_float_cast(v, t, "casttmp")
                                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into()
                        }
                        _ => val,
                    }
                }
                _ => val,
            };

            state.create_var(dest, casted)?;
        }

        MirStmt::Match { dest, scrutinee, arms } => {
            let scrut_val = resolve_value(state, scrutinee)?;

            // Allocate result variable
            let result_ty = match scrut_val {
                BasicValueEnum::IntValue(v) => v.get_type(),
                _ => state.context.i64_type(),
            };
            let result_alloca = state.builder.build_alloca(result_ty, &format!("{}_result", dest))
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;

            let current_fn = state.builder.get_insert_block()
                .and_then(|b| b.get_parent())
                .ok_or_else(|| CodegenError::LlvmError { msg: "no current function".to_string() })?;

            let merge_block = state.context.append_basic_block(current_fn, "match_merge");
            let mut arm_blocks: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();

            // Create blocks for each arm
            for (i, _) in arms.iter().enumerate() {
                arm_blocks.push(state.context.append_basic_block(current_fn, &format!("match_arm_{}", i)));
            }

            // For enum matching, use discriminant-based switch
            // For now, treat scrutinee as an integer discriminant
            let scrut_int = match scrut_val {
                BasicValueEnum::IntValue(v) => v,
                _ => {
                    return Err(CodegenError::LlvmError {
                        msg: "match scrutinee must be integer (enum discriminant)".to_string(),
                    });
                }
            };

            // Build switch: each arm with a literal pattern becomes a switch case
            let mut cases: Vec<(inkwell::values::IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> = Vec::new();
            let mut default_block: Option<inkwell::basic_block::BasicBlock<'ctx>> = None;

            for (i, arm) in arms.iter().enumerate() {
                match &arm.pattern {
                    MirPattern::Literal { value } => {
                        let case_val = result_ty.const_int(*value as u64, true);
                        cases.push((case_val, arm_blocks[i]));
                    }
                    MirPattern::Wildcard => {
                        default_block = Some(arm_blocks[i]);
                    }
                    MirPattern::EnumVariant { variant, .. } => {
                        // For enum variants, we'd need discriminant info
                        // For now, treat as sequential (0, 1, 2, ...)
                        let idx = i as u64;
                        let case_val = result_ty.const_int(idx, true);
                        cases.push((case_val, arm_blocks[i]));
                    }
                    MirPattern::Binding { name: _ } => {
                        // Binding pattern - matches anything, treat as default
                        default_block = Some(arm_blocks[i]);
                    }
                }
            }

            if let Some(def) = default_block {
                state.builder.build_switch(scrut_int, def, &cases)
                    .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
            } else if !cases.is_empty() {
                // No default, use last case as fallback
                let fallback = arm_blocks.last().copied().unwrap();
                state.builder.build_switch(scrut_int, fallback, &cases)
                    .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
            }

            // Compile each arm
            for (i, arm) in arms.iter().enumerate() {
                state.builder.position_at_end(arm_blocks[i]);

                // Compile body statements
                for body_stmt in &arm.body {
                    compile_stmt(state, body_stmt)?;
                }

                // Store the result value
                let body_val = resolve_value(state, &arm.body_result)?;
                state.builder.build_store(result_alloca, body_val)
                    .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;

                state.builder.build_unconditional_branch(merge_block)
                    .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
            }

            // Position at merge block
            state.builder.position_at_end(merge_block);
            let result = state.builder.build_load(result_ty, result_alloca, &dest)
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?;
            state.create_var(dest, result)?;
        }
    }

    Ok(())
}

fn compile_int_bin_op<'ctx>(
    state: &mut CodegenState<'ctx>,
    op: &MirBinOp,
    l: inkwell::values::IntValue<'ctx>,
    r: inkwell::values::IntValue<'ctx>,
) -> Result<inkwell::values::IntValue<'ctx>> {
    // Normalize to same bit width — use the larger of the two
    let l_bits = l.get_type().get_bit_width();
    let r_bits = r.get_type().get_bit_width();
    let (l, r, _target_ty) = if l_bits != r_bits {
        let target = state.context.custom_width_int_type(std::cmp::max(l_bits, r_bits));
        let l = if l_bits < r_bits {
            state.builder.build_int_s_extend(l, target, "sextmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?
        } else {
            l
        };
        let r = if r_bits < l_bits {
            state.builder.build_int_s_extend(r, target, "sextmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?
        } else {
            r
        };
        (l, r, target)
    } else {
        (l, r, l.get_type())
    };

    let result = match op {
        MirBinOp::Add => state.builder.build_int_add(l, r, "addtmp")
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
        MirBinOp::Sub => state.builder.build_int_sub(l, r, "subtmp")
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
        MirBinOp::Mul => state.builder.build_int_mul(l, r, "multmp")
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
        MirBinOp::Div => state.builder.build_int_unsigned_div(l, r, "divtmp")
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
        MirBinOp::Mod => state.builder.build_int_unsigned_rem(l, r, "modtmp")
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
        MirBinOp::Eq => {
            return Ok(state.builder.build_int_compare(inkwell::IntPredicate::EQ, l, r, "eqtmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into());
        }
        MirBinOp::Ne => {
            return Ok(state.builder.build_int_compare(inkwell::IntPredicate::NE, l, r, "netmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into());
        }
        MirBinOp::Lt => {
            return Ok(state.builder.build_int_compare(inkwell::IntPredicate::ULT, l, r, "lttmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into());
        }
        MirBinOp::Gt => {
            return Ok(state.builder.build_int_compare(inkwell::IntPredicate::UGT, l, r, "gttmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into());
        }
        MirBinOp::Le => {
            return Ok(state.builder.build_int_compare(inkwell::IntPredicate::ULE, l, r, "letmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into());
        }
        MirBinOp::Ge => {
            return Ok(state.builder.build_int_compare(inkwell::IntPredicate::UGE, l, r, "getmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into());
        }
        MirBinOp::And => state.builder.build_and(l, r, "andtmp")
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
        MirBinOp::Or => state.builder.build_or(l, r, "ortmp")
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
    };
    Ok(result)
}

fn compile_float_bin_op<'ctx>(
    state: &mut CodegenState<'ctx>,
    op: &MirBinOp,
    l: inkwell::values::FloatValue<'ctx>,
    r: inkwell::values::FloatValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>> {
    let result = match op {
        MirBinOp::Add => state.builder.build_float_add(l, r, "faddtmp")
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
        MirBinOp::Sub => state.builder.build_float_sub(l, r, "fsubtmp")
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
        MirBinOp::Mul => state.builder.build_float_mul(l, r, "fmultmp")
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
        MirBinOp::Div => state.builder.build_float_div(l, r, "fdivtmp")
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
        MirBinOp::Mod => state.builder.build_float_rem(l, r, "fmodtmp")
            .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?,
        MirBinOp::Eq => {
            return Ok(state.builder.build_float_compare(inkwell::FloatPredicate::OEQ, l, r, "feqtmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into());
        }
        MirBinOp::Ne => {
            return Ok(state.builder.build_float_compare(inkwell::FloatPredicate::ONE, l, r, "fnetmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into());
        }
        MirBinOp::Lt => {
            return Ok(state.builder.build_float_compare(inkwell::FloatPredicate::OLT, l, r, "flttmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into());
        }
        MirBinOp::Gt => {
            return Ok(state.builder.build_float_compare(inkwell::FloatPredicate::OGT, l, r, "fgttmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into());
        }
        MirBinOp::Le => {
            return Ok(state.builder.build_float_compare(inkwell::FloatPredicate::OLE, l, r, "fletmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into());
        }
        MirBinOp::Ge => {
            return Ok(state.builder.build_float_compare(inkwell::FloatPredicate::OGE, l, r, "fgetmp")
                .map_err(|e| CodegenError::LlvmError { msg: e.to_string() })?.into());
        }
        _ => return Err(CodegenError::LlvmError {
            msg: "and/or on floats not supported".to_string(),
        }),
    };
    Ok(BasicValueEnum::FloatValue(result))
}

fn resolve_value<'ctx>(
    state: &mut CodegenState<'ctx>,
    value: &MirValue,
) -> Result<BasicValueEnum<'ctx>> {
    match value {
        MirValue::Int(n, ty) => {
            let llvm_ty = mir_type_to_llvm(state.context, ty);
            match llvm_ty {
                inkwell::types::BasicTypeEnum::IntType(t) => {
                    let is_signed = matches!(ty, MirType::I8 | MirType::I16 | MirType::I32 | MirType::I64 | MirType::I128);
                    Ok(t.const_int(*n as u64, !is_signed).into())
                }
                _ => Ok(state.context.i64_type().const_int(*n as u64, false).into()),
            }
        }
        MirValue::Float(n, ty) => {
            let llvm_ty = mir_type_to_llvm(state.context, ty);
            match llvm_ty {
                inkwell::types::BasicTypeEnum::FloatType(t) => {
                    Ok(t.const_float(*n).into())
                }
                _ => Ok(state.context.f64_type().const_float(*n).into()),
            }
        }
        MirValue::Var(name) => {
            state.load_var(name)
        }
        MirValue::Const(name) => {
            if name.starts_with('"') {
                let s = name.trim_matches('"');
                let string_global = state.module.add_global(
                    state.context.i8_type().array_type(s.len() as u32 + 1),
                    Some(AddressSpace::default()),
                    &format!("_str_{}", state.allocas.len()),
                );
                string_global.set_initializer(&state.context.const_string(s.as_bytes(), true));
                string_global.set_constant(true);
                Ok(string_global.as_pointer_value().into())
            } else {
                Err(CodegenError::LlvmError {
                    msg: format!("unknown constant '{}'", name),
                })
            }
        }
    }
}

// ─── Binary Emission ──────────────────────────────────────

pub fn emit_binary(module: &LlvmModule, output: &Path) -> Result<()> {
    use std::io::Write;
    use std::process::Command;

    let ir_path = output.with_extension("ll");
    let mut ir_file = std::fs::File::create(&ir_path)
        .map_err(|e| CodegenError::EmitError { msg: e.to_string() })?;
    ir_file.write_all(module.ir.as_bytes())
        .map_err(|e| CodegenError::EmitError { msg: e.to_string() })?;

    // Use llc to compile IR to object file
    let obj_path = output.with_extension("o");
    let llc_status = Command::new("llc-18")
        .arg("-filetype=obj")
        .arg(&ir_path)
        .arg("-o")
        .arg(&obj_path)
        .status()
        .map_err(|e| CodegenError::EmitError { msg: format!("llc failed: {}", e) })?;

    if !llc_status.success() {
        return Err(CodegenError::EmitError {
            msg: "llc compilation failed".to_string(),
        });
    }

    // Link with cc to produce final binary
    let cc_status = Command::new("cc")
        .arg(&obj_path)
        .arg("-o")
        .arg(output)
        .status()
        .map_err(|e| CodegenError::EmitError { msg: format!("linking failed: {}", e) })?;

    if !cc_status.success() {
        return Err(CodegenError::EmitError {
            msg: "linking failed".to_string(),
        });
    }

    // Clean up temp files
    let _ = std::fs::remove_file(&ir_path);
    let _ = std::fs::remove_file(&obj_path);

    Ok(())
}
