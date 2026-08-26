use crate::sema::{CheckedProgram, CheckedFn, Ty};
use std::fmt;

// ─── MIR (Mid-level IR) ──────────────────────────────────
// Simpler than AST, closer to machine code.
// All control flow is explicit, all types are resolved.

#[derive(Debug, Clone)]
pub struct MirProgram {
    pub functions: Vec<MirFunction>,
    pub structs: Vec<MirStruct>,
    pub enums: Vec<MirEnum>,
}

#[derive(Debug, Clone)]
pub struct MirEnum {
    pub name: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MirStruct {
    pub name: String,
    pub fields: Vec<(String, MirType)>,
}

#[derive(Debug, Clone)]
pub struct MirFunction {
    pub name: String,
    pub params: Vec<(String, MirType)>,
    pub ret_type: MirType,
    pub blocks: Vec<MirBlock>,
    /// Kept for symbol visibility when the compiler grows multi-module support.
    #[allow(dead_code)]
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct MirBlock {
    pub id: usize,
    pub stmts: Vec<MirStmt>,
    pub terminator: MirTerminator,
}

#[derive(Debug, Clone)]
pub enum MirStmt {
    Assign {
        dest: String,
        value: MirValue,
    },
    BinaryOp {
        dest: String,
        op: MirBinOp,
        left: MirValue,
        right: MirValue,
    },
    UnaryOp {
        dest: String,
        op: MirUnaryOp,
        operand: MirValue,
    },
    Call {
        dest: Option<String>,
        func: String,
        args: Vec<MirValue>,
    },
    Asm {
        instructions: Vec<String>,
        outputs: Vec<(String, String)>,
        inputs: Vec<(String, String)>,
    },
    Cast {
        dest: String,
        value: MirValue,
        target_ty: MirType,
    },
    Match {
        dest: String,
        scrutinee: MirValue,
        arms: Vec<MirMatchArm>,
    },
    FieldAccess {
        dest: String,
        struct_var: String,
        struct_name: String,
        field_index: u32,
        field_ty: MirType,
    },
    StoreField {
        struct_var: String,
        struct_name: String,
        field_index: u32,
        value: MirValue,
        field_ty: MirType,
    },
}

#[derive(Debug, Clone)]
pub struct MirMatchArm {
    pub pattern: MirPattern,
    pub guard: Option<MirValue>,
    pub body: Vec<MirStmt>, // body statements
    pub body_result: MirValue, // the result value of this arm
}

#[derive(Debug, Clone)]
pub enum MirPattern {
    Wildcard,
    Binding { name: String },
    EnumVariant {
        enum_name: String,
        variant: String,
        bindings: Vec<String>,
    },
    Literal { value: i64 },
}

#[derive(Debug, Clone)]
pub enum MirBinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
}

#[derive(Debug, Clone)]
pub enum MirUnaryOp {
    Neg, Not,
}

#[derive(Debug, Clone)]
pub enum MirTerminator {
    Return(Option<MirValue>),
    Jump { target: usize },
    Branch {
        cond: MirValue,
        then_target: usize,
        else_target: usize,
    },
}

#[derive(Debug, Clone)]
pub enum MirValue {
    Int(i64, MirType),
    Float(f64, MirType),
    Var(String),
    Const(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MirType {
    Void,
    I8, I16, I32, I64, I128,
    U8, U16, U32, U64, U128,
    F16, F32, F64,
    Bool,
    Char,
    Str,
    Ptr(Box<MirType>),
    Ref { mutable: bool, inner: Box<MirType> },
    Array(Box<MirType>, usize),
    Slice(Box<MirType>),
    Struct(String),
    Enum(String),
    Generic(String, Vec<MirType>),
    GenericParam(String),
}

impl fmt::Display for MirType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MirType::Void => write!(f, "void"),
            MirType::I8 => write!(f, "i8"),
            MirType::I16 => write!(f, "i16"),
            MirType::I32 => write!(f, "i32"),
            MirType::I64 => write!(f, "i64"),
            MirType::I128 => write!(f, "i128"),
            MirType::U8 => write!(f, "u8"),
            MirType::U16 => write!(f, "u16"),
            MirType::U32 => write!(f, "u32"),
            MirType::U64 => write!(f, "u64"),
            MirType::U128 => write!(f, "u128"),
            MirType::F16 => write!(f, "f16"),
            MirType::F32 => write!(f, "f32"),
            MirType::F64 => write!(f, "f64"),
            MirType::Bool => write!(f, "bool"),
            MirType::Char => write!(f, "char"),
            MirType::Str => write!(f, "str"),
            MirType::Ptr(inner) => write!(f, "*{}", inner),
            MirType::Ref { mutable, inner } => {
                if *mutable { write!(f, "&mut {}", inner) }
                else { write!(f, "&{}", inner) }
            }
            MirType::Array(inner, len) => write!(f, "[{}; {}]", inner, len),
            MirType::Slice(inner) => write!(f, "[{}]", inner),
            MirType::Struct(name) => write!(f, "{}", name),
            MirType::Enum(name) => write!(f, "{}", name),
            MirType::Generic(name, args) => {
                write!(f, "{}<", name)?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", a)?;
                }
                write!(f, ">")
            }
            MirType::GenericParam(name) => write!(f, "{}", name),
        }
    }
}

impl fmt::Display for MirProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for func in &self.functions {
            writeln!(f, "fn {}(", func.name)?;
            for (i, (name, ty)) in func.params.iter().enumerate() {
                if i > 0 { write!(f, ", ")?; }
                write!(f, "{}: {}", name, ty)?;
            }
            writeln!(f, ") -> {} {{", func.ret_type)?;
            for block in &func.blocks {
                writeln!(f, "  bb{}:", block.id)?;
                for stmt in &block.stmts {
                    match stmt {
                        MirStmt::Assign { dest, value } => {
                            writeln!(f, "    {} = {:?}", dest, value)?;
                        }
                        MirStmt::BinaryOp { dest, op, left, right } => {
                            writeln!(f, "    {} = {:?} {:?} {:?}", dest, op, left, right)?;
                        }
                        MirStmt::UnaryOp { dest, op, operand } => {
                            writeln!(f, "    {} = {:?} {:?}", dest, op, operand)?;
                        }
                        MirStmt::Call { dest, func, args } => {
                            let dest_str = match dest {
                                Some(d) => format!("{} = ", d),
                                None => String::new(),
                            };
                            writeln!(f, "    {}call {}({:?})", dest_str, func, args)?;
                        }
                        MirStmt::Asm { instructions, .. } => {
                            writeln!(f, "    asm {{")?;
                            for instr in instructions {
                                writeln!(f, "      \"{}\"", instr)?;
                            }
                            writeln!(f, "    }}")?;
                        }
                        MirStmt::Cast { dest, value, target_ty } => {
                            writeln!(f, "    {} = cast {:?} to {}", dest, value, target_ty)?;
                        }
                        MirStmt::Match { dest, scrutinee, arms } => {
                            writeln!(f, "    {} = match {:?} {{", dest, scrutinee)?;
                            for arm in arms {
                                match &arm.pattern {
                                    MirPattern::Wildcard => write!(f, "      _")?,
                                    MirPattern::Binding { name } => write!(f, "      {}", name)?,
                                    MirPattern::EnumVariant { enum_name, variant, bindings } => {
                                        if enum_name.is_empty() {
                                            write!(f, "      {}", variant)?;
                                        } else {
                                            write!(f, "      {}::{}", enum_name, variant)?;
                                        }
                                        if !bindings.is_empty() {
                                            write!(f, "({})", bindings.join(", "))?;
                                        }
                                    }
                                    MirPattern::Literal { value } => write!(f, "      {}", value)?,
                                }
                                if arm.guard.is_some() {
                                    write!(f, " if guard")?;
                                }
                                writeln!(f, " => {:?}", arm.body_result)?;
                            }
                            writeln!(f, "    }}")?;
                        }
                        MirStmt::FieldAccess { dest, struct_var, struct_name, field_index, field_ty } => {
                            writeln!(f, "    {} = field {}.{}[{}] : {}", dest, struct_var, struct_name, field_index, field_ty)?;
                        }
                        MirStmt::StoreField { struct_var, struct_name, field_index, value, field_ty } => {
                            writeln!(f, "    {}.{}[{}] = {:?} : {}", struct_var, struct_name, field_index, value, field_ty)?;
                        }
                    }
                }
                match &block.terminator {
                    MirTerminator::Return(val) => {
                        writeln!(f, "    return {:?}", val)?;
                    }
                    MirTerminator::Jump { target } => {
                        writeln!(f, "    jump bb{}", target)?;
                    }
                    MirTerminator::Branch { cond, then_target, else_target } => {
                        writeln!(f, "    branch {:?} -> bb{}, bb{}", cond, then_target, else_target)?;
                    }
                }
            }
            writeln!(f, "}}")?;
        }
        Ok(())
    }
}

// ─── Conversion helpers ───────────────────────────────────

fn sema_ty_to_mir(ty: &Ty) -> MirType {
    match ty {
        Ty::I8 => MirType::I8,
        Ty::I16 => MirType::I16,
        Ty::I32 => MirType::I32,
        Ty::I64 => MirType::I64,
        Ty::I128 => MirType::I128,
        Ty::U8 => MirType::U8,
        Ty::U16 => MirType::U16,
        Ty::U32 => MirType::U32,
        Ty::U64 => MirType::U64,
        Ty::U128 => MirType::U128,
        Ty::F16 => MirType::F16,
        Ty::F32 => MirType::F32,
        Ty::F64 => MirType::F64,
        Ty::Bool => MirType::Bool,
        Ty::Char => MirType::Char,
        Ty::Str => MirType::Str,
        Ty::Void => MirType::Void,
        Ty::Never => MirType::Void,
        Ty::Ptr(inner) => MirType::Ptr(Box::new(sema_ty_to_mir(inner))),
        Ty::Ref { mutable, inner } => MirType::Ref {
            mutable: *mutable,
            inner: Box::new(sema_ty_to_mir(inner)),
        },
        Ty::Array(inner, len) => MirType::Array(
            Box::new(sema_ty_to_mir(inner)),
            *len,
        ),
        Ty::Slice(inner) => MirType::Slice(Box::new(sema_ty_to_mir(inner))),
        Ty::Struct(name) => MirType::Struct(name.clone()),
        Ty::Enum(name) => MirType::Enum(name.clone()),
        Ty::Fn(params, ret) => MirType::Generic(
            "fn".to_string(),
            params.iter().map(sema_ty_to_mir).chain(std::iter::once(sema_ty_to_mir(ret))).collect(),
        ),
        Ty::Generic(name, args) => MirType::Generic(
            name.clone(),
            args.iter().map(sema_ty_to_mir).collect(),
        ),
        Ty::GenericParam(name) => MirType::GenericParam(name.clone()),
    }
}

// ─── Builder ──────────────────────────────────────────────

pub fn build(program: &CheckedProgram) -> MirProgram {
    let mut functions = Vec::new();
    let structs = &program.structs;
    let enums = &program.enums;

    for func in &program.functions {
        let mir_func = build_function(func, structs, enums);
        functions.push(mir_func);
    }

    // Build impl methods
    for impl_item in &program.impls {
        let mangle_prefix = if let Some(ref trait_name) = impl_item.trait_name {
            trait_name.clone()
        } else {
            format!("{}", impl_item.target_type)
        };
        for method in &impl_item.methods {
            let mut mir_func = build_function(method, structs, enums);
            // Mangle name: TraitName_method or TypeName_method
            mir_func.name = format!("{}_{}", mangle_prefix, method.name);
            functions.push(mir_func);
        }
    }

    MirProgram {
        functions,
        structs: structs.iter().map(|s| MirStruct {
            name: s.name.clone(),
            fields: s.fields.iter().map(|(n, t)| (n.clone(), sema_ty_to_mir(t))).collect(),
        }).collect(),
        enums: enums.iter().map(|e| MirEnum {
            name: e.name.clone(),
            variants: e.variants.iter().map(|v| v.name.clone()).collect(),
        }).collect(),
    }
}

fn build_function(
    func: &CheckedFn,
    structs: &[crate::sema::CheckedStruct],
    enums: &[crate::sema::CheckedEnum],
) -> MirFunction {
    let mut blocks = Vec::new();
    let mut block_id = 0;
    let mut temp_counter = 0;

    let params: Vec<(String, MirType)> = func.params.iter()
        .map(|(name, ty)| (name.clone(), sema_ty_to_mir(ty)))
        .collect();

    let ret_type = sema_ty_to_mir(&func.ret_type);

    // Build entry block (ID 0)
    build_block(&func.body, &mut blocks, 0, &mut block_id, &mut temp_counter, ret_type.clone(), structs, enums, &[]);

    MirFunction {
        name: func.name.clone(),
        params,
        ret_type,
        blocks,
        is_pub: func.is_pub,
    }
}

fn build_block(
    block: &crate::sema::CheckedBlock,
    all_blocks: &mut Vec<MirBlock>,
    current_id: usize,
    block_id: &mut usize,
    temp_counter: &mut usize,
    ret_type: MirType,
    structs: &[crate::sema::CheckedStruct],
    enums: &[crate::sema::CheckedEnum],
    loop_stack: &[(usize, usize)],
) {
    let mut stmts = Vec::new();
    let mut terminator = MirTerminator::Return(None);

    let mut i = 0;
    while i < block.stmts.len() {
        let stmt = &block.stmts[i];
        match stmt {
            crate::sema::CheckedStmt::Let { name, ty, value, mutable: _ } => {
                let (val, val_ty) = build_expr(value, &mut stmts, temp_counter, structs, enums);
                let mir_ty = sema_ty_to_mir(ty);
                // Use declared type if value is undefined/never
                let mut final_val = if matches!(val_ty, MirType::Void) {
                    // Create a zero-initialized value of the declared type
                    match &mir_ty {
                        MirType::I8 | MirType::I16 | MirType::I32 | MirType::I64 | MirType::I128 |
                        MirType::U8 | MirType::U16 | MirType::U32 | MirType::U64 | MirType::U128 |
                        MirType::Bool | MirType::Char => MirValue::Int(0, mir_ty.clone()),
                        MirType::F16 | MirType::F32 | MirType::F64 => MirValue::Float(0.0, mir_ty.clone()),
                        _ => val,
                    }
                } else {
                    val
                };
                // Widen/narrow int values whose type differs from the declaration
                // (e.g. a generic call defaulted to i32 but assigned to an i64 let).
                if val_ty != mir_ty {
                    let widen = matches!(mir_ty, MirType::I64) && matches!(val_ty, MirType::I32);
                    if widen {
                        let cast_dest = format!("_widen_{}", name);
                        stmts.push(MirStmt::Cast {
                            dest: cast_dest.clone(),
                            value: final_val,
                            target_ty: mir_ty.clone(),
                        });
                        final_val = MirValue::Var(cast_dest);
                    }
                }
                stmts.push(MirStmt::Assign {
                    dest: name.clone(),
                    value: final_val,
                });
                i += 1;
            }

            crate::sema::CheckedStmt::Expr(expr, _) => {
                let (val, _) = build_expr(expr, &mut stmts, temp_counter, structs, enums);
                // If this is the last statement and function has non-void return type, auto-return
                if i + 1 == block.stmts.len() && !matches!(ret_type, MirType::Void) {
                    terminator = MirTerminator::Return(Some(val));
                    break;
                }
                i += 1;
            }

            crate::sema::CheckedStmt::Return(value) => {
                let mir_val = match value {
                    Some(e) => {
                        let (val, val_ty) = build_expr(e, &mut stmts, temp_counter, structs, enums);
                        // Narrow/widen so the returned value matches the
                        // function's declared result type (enum tags are i64,
                        // but a function may return i32).
                        Some(coerce_int_width(
                            val,
                            &val_ty,
                            &ret_type,
                            &mut stmts,
                            temp_counter,
                        ))
                    }
                    None => None,
                };
                terminator = MirTerminator::Return(mir_val);
                break; // return ends the block
            }

            crate::sema::CheckedStmt::If { cond, then_block, else_block } => {
                let (cond_val, _) = build_expr(cond, &mut stmts, temp_counter, structs, enums);

                let then_id = *block_id + 1;
                let has_else = else_block.is_some();
                let else_id = if has_else { *block_id + 2 } else { 0 };
                let merge_id = if has_else { *block_id + 3 } else { *block_id + 2 };
                *block_id = merge_id;

                terminator = MirTerminator::Branch {
                    cond: cond_val,
                    then_target: then_id,
                    else_target: if has_else { else_id } else { merge_id },
                };

                // Build then block
                build_block(then_block, all_blocks, then_id, block_id, temp_counter, ret_type.clone(), structs, enums,
                    loop_stack);
                // Patch: add jump to merge at end of then block (only preserve explicit return with value)
                if let Some(b) = all_blocks.iter_mut().find(|b| b.id == then_id) {
                    match &b.terminator {
                        MirTerminator::Return(Some(_)) => {} // keep explicit return
                        _ => b.terminator = MirTerminator::Jump { target: merge_id },
                    }
                }

                // Build else block if exists
                if let Some(b) = else_block {
                    build_block(b, all_blocks, else_id, block_id, temp_counter, ret_type.clone(), structs, enums,
                    loop_stack);
                    if let Some(bb) = all_blocks.iter_mut().find(|bb| bb.id == else_id) {
                        match &bb.terminator {
                            MirTerminator::Return(Some(_)) => {} // keep explicit return
                            _ => bb.terminator = MirTerminator::Jump { target: merge_id },
                        }
                    }
                }

                // Check if there are remaining statements
                let remaining: Vec<_> = block.stmts[i+1..].iter().collect();
                if remaining.is_empty() && has_else {
                    // No remaining statements and both branches exist
                    // Check if both branches return — if so, no merge block needed
                    let then_returns = all_blocks.iter().any(|b| b.id == then_id && matches!(b.terminator, MirTerminator::Return(_)));
                    let else_returns = all_blocks.iter().any(|b| b.id == else_id && matches!(b.terminator, MirTerminator::Return(_)));
                    if then_returns && else_returns {
                        // Both branches return, no merge block needed
                        // Push current block and return
                        all_blocks.push(MirBlock {
                            id: current_id,
                            stmts,
                            terminator,
                        });
                        return;
                    }
                }

                // Remaining statements go into merge block
                let merge_block = build_remaining(&remaining, merge_id, block_id, temp_counter, ret_type.clone(), structs, enums, loop_stack);
                all_blocks.push(merge_block);

                // Push current block before returning
                all_blocks.push(MirBlock {
                    id: current_id,
                    stmts,
                    terminator,
                });
                return; // current block is done
            }

            crate::sema::CheckedStmt::While { cond, body } => {
                let header_id = *block_id + 1;
                let body_id = *block_id + 2;
                let exit_id = *block_id + 3;
                *block_id = exit_id;

                // Current block jumps to header
                terminator = MirTerminator::Jump { target: header_id };

                // Header block: evaluate cond, branch to body or exit
                let mut header_stmts = Vec::new();
                let (cond_val, _) = build_expr(cond, &mut header_stmts, temp_counter, structs, enums);
                header_stmts.push(MirStmt::Assign { dest: format!("_while_cond_{}", header_id), value: cond_val });
                all_blocks.push(MirBlock {
                    id: header_id,
                    stmts: header_stmts,
                    terminator: MirTerminator::Branch {
                        cond: MirValue::Var(format!("_while_cond_{}", header_id)),
                        then_target: body_id,
                        else_target: exit_id,
                    },
                });

                // Body block — push loop context so break/continue work
                let mut inner_stack = loop_stack.to_vec();
                inner_stack.push((header_id, exit_id));
                build_block(body, all_blocks, body_id, block_id, temp_counter, MirType::Void, structs, enums,
                    &inner_stack);
                if let Some(b) = all_blocks.iter_mut().find(|b| b.id == body_id) {
                    let already_set = matches!(&b.terminator,
                        MirTerminator::Jump { target } if *target != header_id
                    );
                    if !already_set {
                        b.terminator = MirTerminator::Jump { target: header_id };
                    }
                }

                // Remaining statements go into exit block
                let remaining: Vec<_> = block.stmts[i+1..].iter().collect();
                let exit_block = build_remaining(&remaining, exit_id, block_id, temp_counter, ret_type.clone(), structs, enums, loop_stack);
                all_blocks.push(exit_block);

                // Push current block before returning
                all_blocks.push(MirBlock {
                    id: current_id,
                    stmts,
                    terminator,
                });
                return; // current block is done
            }

            crate::sema::CheckedStmt::Loop { body } => {
                let body_id = *block_id + 1;
                let exit_id = *block_id + 2;
                *block_id = exit_id;

                terminator = MirTerminator::Jump { target: body_id };

                let mut inner_stack = loop_stack.to_vec();
                inner_stack.push((body_id, exit_id));
                build_block(body, all_blocks, body_id, block_id, temp_counter, MirType::Void, structs, enums,
                    &inner_stack);
                if let Some(b) = all_blocks.iter_mut().find(|b| b.id == body_id) {
                    let is_exit = matches!(&b.terminator, MirTerminator::Jump { target } if *target != body_id);
                    if !is_exit {
                        b.terminator = MirTerminator::Jump { target: body_id };
                    }
                }

                let remaining: Vec<_> = block.stmts[i+1..].iter().collect();
                let exit_block = build_remaining(&remaining, exit_id, block_id, temp_counter, ret_type.clone(), structs, enums, loop_stack);
                all_blocks.push(exit_block);

                // Push current block
                all_blocks.push(MirBlock {
                    id: current_id,
                    stmts,
                    terminator,
                });
                return;
            }

            crate::sema::CheckedStmt::For { var, start, end, body } => {
                // Lower for i in start..end to:
                // let i = start; while i < end { body; i = i + 1; }
                let header_id = *block_id + 1;
                let body_id = *block_id + 2;
                let exit_id = *block_id + 3;
                *block_id = exit_id;

                // Current block: i = start
                let (start_val, _) = build_expr(start, &mut stmts, temp_counter, structs, enums);
                stmts.push(MirStmt::Assign { dest: var.clone(), value: start_val });
                terminator = MirTerminator::Jump { target: header_id };

                // Header block: i < end?
                let mut header_stmts = Vec::new();
                let (end_val, _) = build_expr(end, &mut header_stmts, temp_counter, structs, enums);
                let cond_name = format!("_for_cond_{}", header_id);
                header_stmts.push(MirStmt::Assign {
                    dest: cond_name.clone(),
                    value: MirValue::Var(var.clone()),
                });
                // i < end
                let cmp_name = format!("_for_cmp_{}", header_id);
                let end_var_name = format!("_for_end_{}", header_id);
                header_stmts.push(MirStmt::Assign {
                    dest: end_var_name.clone(),
                    value: end_val,
                });
                header_stmts.push(MirStmt::BinaryOp {
                    dest: cmp_name.clone(),
                    op: MirBinOp::Lt,
                    left: MirValue::Var(var.clone()),
                    right: MirValue::Var(end_var_name),
                });
                all_blocks.push(MirBlock {
                    id: header_id,
                    stmts: header_stmts,
                    terminator: MirTerminator::Branch {
                        cond: MirValue::Var(cmp_name),
                        then_target: body_id,
                        else_target: exit_id,
                    },
                });

                // Body block with loop context for break/continue
                let mut inner_stack = loop_stack.to_vec();
                inner_stack.push((header_id, exit_id));
                build_block(body, all_blocks, body_id, block_id, temp_counter, MirType::Void, structs, enums,
                    &inner_stack);
                // At end of body, i = i + 1 and jump to header
                if let Some(b) = all_blocks.iter_mut().find(|b| b.id == body_id) {
                    let needs_inc = matches!(&b.terminator,
                        MirTerminator::Jump { target } if *target == body_id
                    ) || matches!(&b.terminator, MirTerminator::Return(None));
                    if needs_inc || matches!(&b.terminator, MirTerminator::Jump { target } if *target == body_id) {
                        let inc_name = format!("_for_inc_{}", body_id);
                        let one = MirValue::Int(1, MirType::I64);
                        b.stmts.push(MirStmt::BinaryOp {
                            dest: inc_name.clone(),
                            op: MirBinOp::Add,
                            left: MirValue::Var(var.clone()),
                            right: one,
                        });
                        b.stmts.push(MirStmt::Assign {
                            dest: var.clone(),
                            value: MirValue::Var(inc_name),
                        });
                        b.terminator = MirTerminator::Jump { target: header_id };
                    }
                }

                // Remaining statements in exit block
                let remaining: Vec<_> = block.stmts[i+1..].iter().collect();
                let exit_block = build_remaining(&remaining, exit_id, block_id, temp_counter, ret_type.clone(), structs, enums, loop_stack);
                all_blocks.push(exit_block);

                all_blocks.push(MirBlock {
                    id: current_id,
                    stmts,
                    terminator,
                });
                return;
            }

            crate::sema::CheckedStmt::Block(inner) => {
                // Inline the inner block's statements
                for inner_stmt in &inner.stmts {
                    match inner_stmt {
                        crate::sema::CheckedStmt::Let { name, ty: _, value, mutable: _ } => {
                            let (val, _) = build_expr(value, &mut stmts, temp_counter, structs, enums);
                            stmts.push(MirStmt::Assign { dest: name.clone(), value: val });
                        }
                        crate::sema::CheckedStmt::Expr(expr, _) => {
                            let _ = build_expr(expr, &mut stmts, temp_counter, structs, enums);
                        }
                        crate::sema::CheckedStmt::Return(value) => {
                            let mir_val = match value {
                                Some(e) => {
                                    let (val, _) = build_expr(e, &mut stmts, temp_counter, structs, enums);
                                    Some(val)
                                }
                                None => None,
                            };
                            terminator = MirTerminator::Return(mir_val);
                            break;
                        }
                        crate::sema::CheckedStmt::Break => {
                            if let Some((_header, exit)) = loop_stack.last() {
                                terminator = MirTerminator::Jump { target: *exit };
                                break;
                            }
                        }
                        crate::sema::CheckedStmt::Continue => {
                            if let Some((header, _exit)) = loop_stack.last() {
                                terminator = MirTerminator::Jump { target: *header };
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                i += 1;
            }

            crate::sema::CheckedStmt::Unsafe(ublock) => {
                for inner_stmt in &ublock.stmts {
                    match inner_stmt {
                        crate::sema::CheckedStmt::Expr(expr, _) => {
                            let _ = build_expr(expr, &mut stmts, temp_counter, structs, enums);
                        }
                        crate::sema::CheckedStmt::Return(value) => {
                            let mir_val = match value {
                                Some(e) => {
                                    let (val, _) = build_expr(e, &mut stmts, temp_counter, structs, enums);
                                    Some(val)
                                }
                                None => None,
                            };
                            terminator = MirTerminator::Return(mir_val);
                        }
                        crate::sema::CheckedStmt::Break => {
                            if let Some((_header, exit)) = loop_stack.last() {
                                terminator = MirTerminator::Jump { target: *exit };
                                break;
                            }
                        }
                        crate::sema::CheckedStmt::Continue => {
                            if let Some((header, _exit)) = loop_stack.last() {
                                terminator = MirTerminator::Jump { target: *header };
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                i += 1;
            }

            crate::sema::CheckedStmt::Break => {
                if let Some((_header, exit)) = loop_stack.last() {
                    let t = MirTerminator::Jump { target: *exit };
                    all_blocks.push(MirBlock { id: current_id, stmts, terminator: t });
                    return;
                }
            }
            crate::sema::CheckedStmt::Continue => {
                if let Some((header, _exit)) = loop_stack.last() {
                    let t = MirTerminator::Jump { target: *header };
                    all_blocks.push(MirBlock { id: current_id, stmts, terminator: t });
                    return;
                }
            }
        }
    }

    all_blocks.push(MirBlock {
        id: current_id,
        stmts,
        terminator,
    });
}

/// Build a block from remaining statements after a control flow split
fn build_remaining(
    remaining: &[&crate::sema::CheckedStmt],
    current_id: usize,
    block_id: &mut usize,
    temp_counter: &mut usize,
    ret_type: MirType,
    structs: &[crate::sema::CheckedStruct],
    enums: &[crate::sema::CheckedEnum],
    loop_stack: &[(usize, usize)],
) -> MirBlock {
    let mut stmts = Vec::new();
    let mut terminator = MirTerminator::Return(None);

    let mut i = 0;
    while i < remaining.len() {
        let stmt = remaining[i];
        match stmt {
            crate::sema::CheckedStmt::Let { name, ty: _, value, mutable: _ } => {
                let (val, _) = build_expr(value, &mut stmts, temp_counter, structs, enums);
                stmts.push(MirStmt::Assign { dest: name.clone(), value: val });
            }
            crate::sema::CheckedStmt::Expr(expr, _) => {
                let (val, _) = build_expr(expr, &mut stmts, temp_counter, structs, enums);
                // If this is the last statement and function has non-void return type, auto-return
                if i + 1 == remaining.len() && !matches!(ret_type, MirType::Void) {
                    terminator = MirTerminator::Return(Some(val));
                    break;
                }
            }
            crate::sema::CheckedStmt::Return(value) => {
                let mir_val = match value {
                    Some(e) => {
                        let (val, _) = build_expr(e, &mut stmts, temp_counter, structs, enums);
                        Some(val)
                    }
                    None => None,
                };
                terminator = MirTerminator::Return(mir_val);
                break;
            }
            crate::sema::CheckedStmt::If { cond, then_block, else_block } => {
                let (cond_val, _) = build_expr(cond, &mut stmts, temp_counter, structs, enums);
                let then_id = *block_id + 1;
                let has_else = else_block.is_some();
                let else_id = if has_else { *block_id + 2 } else { 0 };
                let merge_id = if has_else { *block_id + 3 } else { *block_id + 2 };
                *block_id = merge_id;

                terminator = MirTerminator::Branch {
                    cond: cond_val,
                    then_target: then_id,
                    else_target: if has_else { else_id } else { merge_id },
                };

                // For nested if in remaining, we just build the then block and create a merge
                build_block(then_block, &mut Vec::new(), then_id, block_id, temp_counter, ret_type.clone(), structs, enums,
                    loop_stack);
                // This is simplified — nested control flow in remaining is not fully handled
                break;
            }
            crate::sema::CheckedStmt::Block(inner) => {
                for s in &inner.stmts {
                    if let crate::sema::CheckedStmt::Let { name, ty: _, value, mutable: _ } = s {
                        let (val, _) = build_expr(value, &mut stmts, temp_counter, structs, enums);
                        stmts.push(MirStmt::Assign { dest: name.clone(), value: val });
                    } else if let crate::sema::CheckedStmt::Expr(expr, _) = s {
                        let _ = build_expr(expr, &mut stmts, temp_counter, structs, enums);
                    } else if let crate::sema::CheckedStmt::Return(value) = s {
                        let mir_val = match value {
                            Some(e) => { let (v, _) = build_expr(e, &mut stmts, temp_counter, structs, enums); Some(v) }
                            None => None,
                        };
                        terminator = MirTerminator::Return(mir_val);
                        break;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }

    MirBlock {
        id: current_id,
        stmts,
        terminator,
    }
}

/// Emit a cast when an integer value's width does not match the expected type.
/// Enum discriminants are i64 while a function may declare i32, so returns need
/// an explicit narrow/widen instead of emitting type-mismatched IR.
fn coerce_int_width(
    val: MirValue,
    from: &MirType,
    to: &MirType,
    stmts: &mut Vec<MirStmt>,
    temp_counter: &mut usize,
) -> MirValue {
    let int_width = |t: &MirType| match t {
        MirType::I8 | MirType::U8 => Some(8),
        MirType::I16 | MirType::U16 => Some(16),
        MirType::I32 | MirType::U32 => Some(32),
        MirType::I64 | MirType::U64 | MirType::Enum(_) => Some(64),
        _ => None,
    };
    match (int_width(from), int_width(to)) {
        (Some(a), Some(b)) if a != b => {
            let dest = format!("_coerce{}", temp_counter);
            *temp_counter += 1;
            stmts.push(MirStmt::Cast {
                dest: dest.clone(),
                value: val,
                target_ty: to.clone(),
            });
            MirValue::Var(dest)
        }
        _ => val,
    }
}

// ─── Expression Builder ───────────────────────────────────

fn build_expr(expr: &crate::sema::CheckedExpr, stmts: &mut Vec<MirStmt>, temp_counter: &mut usize, structs: &[crate::sema::CheckedStruct], enums: &[crate::sema::CheckedEnum]) -> (MirValue, MirType) {
    match expr {
        crate::sema::CheckedExpr::IntLit(n, ty) => {
            let mir_ty = sema_ty_to_mir(ty);
            (MirValue::Int(*n, mir_ty.clone()), mir_ty)
        }
        crate::sema::CheckedExpr::FloatLit(n, ty) => {
            let mir_ty = sema_ty_to_mir(ty);
            (MirValue::Float(*n, mir_ty.clone()), mir_ty)
        }
        crate::sema::CheckedExpr::StringLit(s) => {
            let name = format!("_str_{}", temp_counter);
            *temp_counter += 1;
            stmts.push(MirStmt::Assign {
                dest: name.clone(),
                value: MirValue::Const(format!("\"{}\"", s)),
            });
            (MirValue::Var(name), MirType::Str)
        }
        crate::sema::CheckedExpr::CharLit(c) => {
            (MirValue::Int(*c as i64, MirType::Char), MirType::Char)
        }
        crate::sema::CheckedExpr::BoolLit(b) => {
            (MirValue::Int(if *b { 1 } else { 0 }, MirType::Bool), MirType::Bool)
        }
        crate::sema::CheckedExpr::Undefined(_) => {
            (MirValue::Int(0, MirType::Void), MirType::Void)
        }
        crate::sema::CheckedExpr::Var(name, ty) => {
            (MirValue::Var(name.clone()), sema_ty_to_mir(ty))
        }
        crate::sema::CheckedExpr::Binary { op, left, right, result_ty } => {
            let (l_val, _) = build_expr(left, stmts, temp_counter, structs, enums);
            let (r_val, _) = build_expr(right, stmts, temp_counter, structs, enums);

            let dest = format!("_t{}", temp_counter);
            *temp_counter += 1;

            let mir_op = match op {
                crate::parser::ast::BinOp::Add => MirBinOp::Add,
                crate::parser::ast::BinOp::Sub => MirBinOp::Sub,
                crate::parser::ast::BinOp::Mul => MirBinOp::Mul,
                crate::parser::ast::BinOp::Div => MirBinOp::Div,
                crate::parser::ast::BinOp::Mod => MirBinOp::Mod,
                crate::parser::ast::BinOp::Eq => MirBinOp::Eq,
                crate::parser::ast::BinOp::Ne => MirBinOp::Ne,
                crate::parser::ast::BinOp::Lt => MirBinOp::Lt,
                crate::parser::ast::BinOp::Gt => MirBinOp::Gt,
                crate::parser::ast::BinOp::Le => MirBinOp::Le,
                crate::parser::ast::BinOp::Ge => MirBinOp::Ge,
                crate::parser::ast::BinOp::And => MirBinOp::And,
                crate::parser::ast::BinOp::Or => MirBinOp::Or,
            };

            stmts.push(MirStmt::BinaryOp {
                dest: dest.clone(),
                op: mir_op,
                left: l_val,
                right: r_val,
            });

            (MirValue::Var(dest), sema_ty_to_mir(result_ty))
        }
        crate::sema::CheckedExpr::Unary { op, operand, result_ty } => {
            let (val, _) = build_expr(operand, stmts, temp_counter, structs, enums);

            let dest = format!("_t{}", temp_counter);
            *temp_counter += 1;

            let mir_op = match op {
                crate::parser::ast::UnaryOp::Neg => MirUnaryOp::Neg,
                crate::parser::ast::UnaryOp::Not => MirUnaryOp::Not,
            };

            stmts.push(MirStmt::UnaryOp {
                dest: dest.clone(),
                op: mir_op,
                operand: val,
            });

            (MirValue::Var(dest), sema_ty_to_mir(result_ty))
        }
        crate::sema::CheckedExpr::Assign { target, value } => {
            let (val, _) = build_expr(value, stmts, temp_counter, structs, enums);
            let dest = match target.as_ref() {
                crate::sema::CheckedExpr::Var(name, _) => name.clone(),
                _ => {
                    let n = format!("_t{}", temp_counter);
                    *temp_counter += 1;
                    n
                }
            };
            stmts.push(MirStmt::Assign {
                dest: dest.clone(),
                value: val,
            });
            (MirValue::Var(dest), MirType::Void)
        }
        crate::sema::CheckedExpr::RefExpr { mutable: _, operand, result_ty } => {
            let (val, _) = build_expr(operand, stmts, temp_counter, structs, enums);
            let dest = format!("_t{}", temp_counter);
            *temp_counter += 1;
            stmts.push(MirStmt::Assign {
                dest: dest.clone(),
                value: val,
            });
            (MirValue::Var(dest), sema_ty_to_mir(result_ty))
        }
        crate::sema::CheckedExpr::Deref { operand, result_ty } => {
            let (val, _) = build_expr(operand, stmts, temp_counter, structs, enums);
            let dest = format!("_t{}", temp_counter);
            *temp_counter += 1;
            stmts.push(MirStmt::Assign {
                dest: dest.clone(),
                value: val,
            });
            (MirValue::Var(dest), sema_ty_to_mir(result_ty))
        }
        crate::sema::CheckedExpr::Call { callee, args, result_ty, mono_name } => {
            let func_name = match callee.as_ref() {
                crate::sema::CheckedExpr::Var(name, _) => {
                    // Generic calls resolve to their specialized instantiation.
                    mono_name.clone().unwrap_or_else(|| name.clone())
                }
                _ => "unknown".to_string(),
            };

            // Map stdlib functions to C equivalents
            let c_func_name = match func_name.as_str() {
                "print" | "print_int" => "printf",
                "alloc" => "malloc",
                "free" => "free",
                "memcpy" => "memcpy",
                "memset" => "memset",
                _ => &func_name,
            }.to_string();

            let mut mir_args = Vec::new();
            for arg in args {
                let (val, _) = build_expr(arg, stmts, temp_counter, structs, enums);
                mir_args.push(val);
            }

            // For print: add format string argument
            let mir_args = if func_name == "print" {
                let fmt = MirValue::Const("\"%s\"".to_string());
                let mut new_args = vec![fmt];
                new_args.extend(mir_args);
                new_args
            } else if func_name == "print_int" {
                let fmt = MirValue::Const("\"%d\n\"".to_string());
                let mut new_args = vec![fmt];
                new_args.extend(mir_args);
                new_args
            } else {
                mir_args
            };

            let dest = if result_ty != &crate::sema::Ty::Void {
                let d = format!("_t{}", temp_counter);
                *temp_counter += 1;
                Some(d.clone())
            } else {
                None
            };

            stmts.push(MirStmt::Call {
                dest: dest.clone(),
                func: c_func_name,
                args: mir_args,
            });

            let ret_ty = if let Some(d) = dest {
                (MirValue::Var(d), sema_ty_to_mir(result_ty))
            } else {
                (MirValue::Int(0, MirType::Void), MirType::Void)
            };
            ret_ty
        }
        crate::sema::CheckedExpr::MethodCall { receiver, method: _, mangled, args, result_ty } => {
            let (recv_val, _) = build_expr(receiver, stmts, temp_counter, structs, enums);
            let mut mir_args = vec![recv_val];
            for arg in args {
                let (val, _) = build_expr(arg, stmts, temp_counter, structs, enums);
                mir_args.push(val);
            }

            let dest = format!("_t{}", temp_counter);
            *temp_counter += 1;

            stmts.push(MirStmt::Call {
                dest: Some(dest.clone()),
                func: mangled.clone(),
                args: mir_args,
            });

            (MirValue::Var(dest), sema_ty_to_mir(result_ty))
        }
        crate::sema::CheckedExpr::FieldAccess { receiver, field, result_ty } => {
            let (recv_val, recv_ty) = build_expr(receiver, stmts, temp_counter, structs, enums);
            let dest = format!("_t{}", temp_counter);
            *temp_counter += 1;

            // Find struct type and field index
            let struct_name = match &recv_ty {
                MirType::Struct(name) => name.clone(),
                _ => String::new(),
            };
            let mut field_index = 0u32;
            let mut found = false;
            if let Some(struct_info) = structs.iter().find(|s| s.name == struct_name) {
                for (i, (fname, _fty)) in struct_info.fields.iter().enumerate() {
                    if fname == field {
                        field_index = i as u32;
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                // Fallback: use index 0
                field_index = 0;
            }

            let struct_var = match &recv_val {
                MirValue::Var(v) => v.clone(),
                _ => "unknown".to_string(),
            };

            stmts.push(MirStmt::FieldAccess {
                dest: dest.clone(),
                struct_var,
                struct_name,
                field_index,
                field_ty: sema_ty_to_mir(result_ty),
            });
            (MirValue::Var(dest), sema_ty_to_mir(result_ty))
        }
        crate::sema::CheckedExpr::StaticCall { type_name, method, args, result_ty, .. } => {
            // Mangled name: TypeName_method
            let func_name = format!("{}_{}", type_name, method);
            let mut mir_args = Vec::new();
            for arg in args {
                let (val, _) = build_expr(arg, stmts, temp_counter, structs, enums);
                mir_args.push(val);
            }

            let dest = format!("_t{}", temp_counter);
            *temp_counter += 1;

            stmts.push(MirStmt::Call {
                dest: Some(dest.clone()),
                func: func_name,
                args: mir_args,
            });

            (MirValue::Var(dest), sema_ty_to_mir(result_ty))
        }
        crate::sema::CheckedExpr::PathAccess { type_name, name, result_ty } => {
            // A unit enum variant lowers to its discriminant, matching how
            // MirPattern::EnumVariant compares tags at match sites.
            if let crate::sema::Ty::Enum(_) = result_ty {
                let tag = enums
                    .iter()
                    .find(|e| e.name == *type_name)
                    .and_then(|e| e.variants.iter().position(|v| v.name == *name))
                    .unwrap_or(0) as i64;
                return (MirValue::Int(tag, MirType::I64), MirType::I64);
            }
            // Associated const or unresolved path: treat as a variable access.
            let var_name = format!("{}_{}", type_name, name);
            (MirValue::Var(var_name), sema_ty_to_mir(result_ty))
        }
        crate::sema::CheckedExpr::Index { target, index, result_ty } => {
            let (target_val, _) = build_expr(target, stmts, temp_counter, structs, enums);
            let (index_val, _) = build_expr(index, stmts, temp_counter, structs, enums);
            let dest = format!("_t{}", temp_counter);
            *temp_counter += 1;
            let target_str = match &target_val {
                MirValue::Var(v) => v.clone(),
                _ => "unknown".to_string(),
            };
            let index_str = match &index_val {
                MirValue::Var(v) => v.clone(),
                MirValue::Int(n, _) => n.to_string(),
                _ => "?".to_string(),
            };
            stmts.push(MirStmt::Assign {
                dest: dest.clone(),
                value: MirValue::Var(format!("{}[{}]", target_str, index_str)),
            });
            (MirValue::Var(dest), sema_ty_to_mir(result_ty))
        }
        crate::sema::CheckedExpr::AsmBlock { instructions, outputs, inputs } => {
            stmts.push(MirStmt::Asm {
                instructions: instructions.clone(),
                outputs: outputs.clone(),
                inputs: inputs.clone(),
            });
            (MirValue::Int(0, MirType::Void), MirType::Void)
        }
        crate::sema::CheckedExpr::Cast { expr: inner, target_ty } => {
            let (val, _) = build_expr(inner, stmts, temp_counter, structs, enums);
            let dest = format!("_t{}", temp_counter);
            *temp_counter += 1;
            stmts.push(MirStmt::Cast {
                dest: dest.clone(),
                value: val,
                target_ty: sema_ty_to_mir(target_ty),
            });
            (MirValue::Var(dest), sema_ty_to_mir(target_ty))
        }
        crate::sema::CheckedExpr::Match { scrutinee, arms, result_ty } => {
            let (scrut_val, _) = build_expr(scrutinee, stmts, temp_counter, structs, enums);
            let dest = format!("_t{}", temp_counter);
            *temp_counter += 1;

            let mut mir_arms = Vec::new();
            for arm in arms {
                let mut arm_stmts = Vec::new();
                let mut arm_temp = *temp_counter;
                let (body_val, _) = build_expr(&arm.body, &mut arm_stmts, &mut arm_temp, structs, enums);
                *temp_counter = arm_temp;

                let mir_pattern = match &arm.pattern {
                    crate::sema::CheckedPattern::Wildcard => MirPattern::Wildcard,
                    crate::sema::CheckedPattern::Binding { name } => MirPattern::Binding { name: name.clone() },
                    crate::sema::CheckedPattern::EnumVariant { enum_name, variant, bindings } => {
                        MirPattern::EnumVariant {
                            enum_name: enum_name.clone(),
                            variant: variant.clone(),
                            bindings: bindings.clone(),
                        }
                    }
                    crate::sema::CheckedPattern::Literal { value } => MirPattern::Literal { value: *value },
                };

                let mir_guard = match &arm.guard {
                    Some(g) => {
                        let (gv, _) = build_expr(g, &mut arm_stmts, &mut arm_temp, structs, enums);
                        *temp_counter = arm_temp;
                        Some(gv)
                    }
                    None => None,
                };

                mir_arms.push(MirMatchArm {
                    pattern: mir_pattern,
                    guard: mir_guard,
                    body: arm_stmts,
                    body_result: body_val,
                });
            }

            stmts.push(MirStmt::Match {
                dest: dest.clone(),
                scrutinee: scrut_val,
                arms: mir_arms,
            });

            (MirValue::Var(dest), sema_ty_to_mir(result_ty))
        }
        crate::sema::CheckedExpr::StructLit { name, fields, result_ty, .. } => {
            let dest = format!("_t{}", temp_counter);
            *temp_counter += 1;
            let mir_struct_name = name.clone();
            let field_tys: Vec<MirType> = structs.iter()
                .find(|s| s.name == mir_struct_name)
                .map(|s| s.fields.iter().map(|f| sema_ty_to_mir(&f.1)).collect())
                .unwrap_or_default();
            for (i, (_field_name, field_expr)) in fields.iter().enumerate() {
                let (val, _ty) = build_expr(field_expr, stmts, temp_counter, structs, enums);
                let field_ty = field_tys.get(i).cloned().unwrap_or(MirType::Void);
                stmts.push(MirStmt::StoreField {
                    struct_var: dest.clone(),
                    struct_name: mir_struct_name.clone(),
                    field_index: i as u32,
                    value: val,
                    field_ty,
                });
            }
            (MirValue::Var(dest), sema_ty_to_mir(result_ty))
        }
    }
}
