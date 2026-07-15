use crate::sema::{CheckedProgram, CheckedFn, CheckedBlock, CheckedStmt, CheckedExpr, Ty};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OwnershipError {
    #[error("use of moved value `{name}`")]
    UseAfterMove { name: String },

    #[error("cannot borrow `{name}` as mutable, already borrowed")]
    DoubleMutBorrow { name: String },

    #[error("cannot borrow `{name}` as immutable, already mutably borrowed")]
    MutBorrowConflict { name: String },

    #[error("cannot move out of `{name}` because it is borrowed")]
    MoveWhileBorrowed { name: String },
}

type Result<T> = std::result::Result<T, OwnershipError>;

// ─── Ownership State ──────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum OwnershipState {
    /// Variable owns its value
    Owned,
    /// Value has been moved to another variable
    Moved { to: String },
    /// Immutable borrow (&T)
    ImmBorrow { count: usize },
    /// Mutable borrow (&mut T)
    MutBorrow,
}

// ─── Checker ──────────────────────────────────────────────

pub struct OwnershipChecker {
    /// Track ownership state per variable
    vars: HashMap<String, OwnershipState>,
    /// Track which variables are "copy types" (primitives)
    copy_types: Vec<Ty>,
    /// Track variables declared in current scope (for scope-based borrow release)
    scope_vars: Vec<HashSet<String>>,
}

impl OwnershipChecker {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
            copy_types: vec![
                Ty::I8, Ty::I16, Ty::I32, Ty::I64, Ty::I128,
                Ty::U8, Ty::U16, Ty::U32, Ty::U64, Ty::U128,
                Ty::F16, Ty::F32, Ty::F64,
                Ty::Bool, Ty::Char,
                // Pointers are copy types (just addresses)
                Ty::Ptr(Box::new(Ty::U8)), // generic pointer
            ],
            scope_vars: vec![HashSet::new()], // root scope
        }
    }

    fn is_copy_type(&self, ty: &Ty) -> bool {
        // All primitives are copy types
        if self.copy_types.iter().any(|t| t == ty) {
            return true;
        }
        // Any pointer type is a copy type
        if let Ty::Ptr(_) = ty {
            return true;
        }
        false
    }

    fn enter_scope(&mut self) {
        self.scope_vars.push(HashSet::new());
    }

    fn exit_scope(&mut self) {
        if let Some(vars_in_scope) = self.scope_vars.pop() {
            // Release borrows for variables declared in this scope
            for name in &vars_in_scope {
                if let Some(state) = self.vars.get(name) {
                    if matches!(state, OwnershipState::ImmBorrow { .. } | OwnershipState::MutBorrow) {
                        // Borrow ends at scope boundary
                        self.vars.remove(name);
                    }
                }
            }
        }
    }

    fn release_borrow(&mut self, name: &str) {
        if let Some(state) = self.vars.get(name) {
            if matches!(state, OwnershipState::ImmBorrow { .. } | OwnershipState::MutBorrow) {
                self.vars.remove(name);
            }
        }
    }

    pub fn check(program: &CheckedProgram) -> Result<()> {
        let mut checker = Self::new();
        for func in &program.functions {
            checker.check_function(func)?;
        }
        Ok(())
    }

    fn check_function(&mut self, func: &CheckedFn) -> Result<()> {
        self.vars.clear();

        // Register parameters as owned
        for (name, ty) in &func.params {
            if self.is_copy_type(ty) {
                self.vars.insert(name.clone(), OwnershipState::Owned);
            } else {
                self.vars.insert(name.clone(), OwnershipState::Owned);
            }
        }

        self.check_block(&func.body)?;
        Ok(())
    }

    fn check_block(&mut self, block: &CheckedBlock) -> Result<()> {
        self.enter_scope();

        for stmt in &block.stmts {
            self.check_stmt(stmt)?;
        }

        self.exit_scope();
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &CheckedStmt) -> Result<()> {
        match stmt {
            CheckedStmt::Let { name, ty, value, mutable: _ } => {
                // Check the value expression
                self.check_expr(value)?;

                // If value is a variable reference, it might be a move
                if let CheckedExpr::Var(var_name, _) = &**value {
                    if !self.is_copy_type(ty) {
                        // This is a move
                        match self.vars.get(var_name) {
                            Some(OwnershipState::Owned) => {
                                self.vars.insert(var_name.clone(), OwnershipState::Moved { to: name.clone() });
                            }
                            Some(OwnershipState::Moved { to: _ }) => {
                                return Err(OwnershipError::UseAfterMove { name: var_name.clone() });
                            }
                            Some(OwnershipState::MutBorrow) => {
                                return Err(OwnershipError::MoveWhileBorrowed { name: var_name.clone() });
                            }
                            Some(OwnershipState::ImmBorrow { count: c }) => {
                                // Can't move while immutably borrowed
                                if *c > 0 {
                                    return Err(OwnershipError::MoveWhileBorrowed { name: var_name.clone() });
                                }
                            }
                            None => {}
                        }
                    }
                }

                // New variable is owned
                self.vars.insert(name.clone(), OwnershipState::Owned);
                // Track in current scope
                if let Some(scope) = self.scope_vars.last_mut() {
                    scope.insert(name.clone());
                }
            }

            CheckedStmt::Expr(expr, _) => {
                self.check_expr(expr)?;
            }

            CheckedStmt::Return(value) => {
                if let Some(expr) = value {
                    self.check_expr(expr)?;
                }
            }

            CheckedStmt::If { cond, then_block, else_block } => {
                self.check_expr(cond)?;

                // Check then branch
                let saved = self.vars.clone();
                self.check_block(then_block)?;
                let then_state = self.vars.clone();
                self.vars = saved.clone();

                // Check else branch
                if let Some(else_blk) = else_block {
                    self.check_block(else_blk)?;
                }

                // Merge states: keep variables that are owned in both branches
                // For simplicity, we keep the more restrictive state
                for (name, state) in &then_state {
                    if let Some(existing) = self.vars.get(name) {
                        // If moved in one branch but owned in another, mark as moved
                        if matches!(state, OwnershipState::Moved { .. }) || matches!(existing, OwnershipState::Moved { .. }) {
                            self.vars.insert(name.clone(), OwnershipState::Moved { to: "branch".to_string() });
                        }
                    }
                }
            }

            CheckedStmt::While { cond, body } => {
                self.check_expr(cond)?;
                self.check_block(body)?;
            }

            CheckedStmt::Loop { body } => {
                self.check_block(body)?;
            }
            CheckedStmt::For { body, .. } => {
                self.check_block(body)?;
            }

            CheckedStmt::Block(inner) => {
                self.check_block(inner)?;
            }

            CheckedStmt::Unsafe(inner) => {
                // Skip ownership checks inside unsafe blocks
                let _ = inner;
            }

            CheckedStmt::Break | CheckedStmt::Continue => {}
        }

        Ok(())
    }

    fn check_expr(&mut self, expr: &CheckedExpr) -> Result<()> {
        match expr {
            CheckedExpr::Var(name, ty) => {
                // Using a variable
                match self.vars.get(name) {
                    Some(OwnershipState::Owned) => {
                        // Reading owned value is fine (copy if primitive)
                    }
                    Some(OwnershipState::Moved { to }) => {
                        return Err(OwnershipError::UseAfterMove { name: name.clone() });
                    }
                    Some(OwnershipState::MutBorrow) => {
                        // Can read through mutable borrow
                    }
                    Some(OwnershipState::ImmBorrow { count }) => {
                        // Can read through immutable borrow
                    }
                    None => {
                        // Variable not tracked (might be a parameter or global)
                    }
                }
            }

            CheckedExpr::Call { callee, args, result_ty } => {
                self.check_expr(callee)?;
                for arg in args {
                    self.check_expr(arg)?;

                    // If argument is a variable reference and not copy type, it's a move
                    if let CheckedExpr::Var(var_name, arg_ty) = arg {
                        if !self.is_copy_type(arg_ty) {
                            match self.vars.get(var_name) {
                                Some(OwnershipState::Owned) => {
                                    self.vars.insert(var_name.clone(), OwnershipState::Moved { to: "call".to_string() });
                                }
                                Some(OwnershipState::Moved { .. }) => {
                                    return Err(OwnershipError::UseAfterMove { name: var_name.clone() });
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }

            CheckedExpr::Binary { op: _, left, right, result_ty: _ } => {
                self.check_expr(left)?;
                self.check_expr(right)?;
            }

            CheckedExpr::Unary { op: _, operand, result_ty: _ } => {
                self.check_expr(operand)?;
            }

            CheckedExpr::Assign { target, value } => {
                self.check_expr(value)?;
                match &**target {
                    CheckedExpr::Var(var_name, _) => {
                        // Reassignment: old value is dropped, new value is owned
                        self.vars.insert(var_name.clone(), OwnershipState::Owned);
                    }
                    CheckedExpr::Deref { operand, .. } => {
                        // *borrow = value — this uses the borrow
                        self.check_expr(operand)?;
                        // After deref assignment, the borrow is "used" — release it
                        if let CheckedExpr::Var(borrow_name, _) = &**operand {
                            self.release_borrow(borrow_name);
                        }
                    }
                    _ => {}
                }
            }

            CheckedExpr::RefExpr { mutable, operand, result_ty: _ } => {
                self.check_expr(operand)?;

                if let CheckedExpr::Var(var_name, _) = &**operand {
                    if *mutable {
                        // Mutable borrow
                        match self.vars.get(var_name) {
                            Some(OwnershipState::Owned) => {
                                self.vars.insert(var_name.clone(), OwnershipState::MutBorrow);
                            }
                            Some(OwnershipState::MutBorrow) => {
                                return Err(OwnershipError::DoubleMutBorrow { name: var_name.clone() });
                            }
                            Some(OwnershipState::ImmBorrow { count }) => {
                                if *count > 0 {
                                    return Err(OwnershipError::MutBorrowConflict { name: var_name.clone() });
                                }
                                self.vars.insert(var_name.clone(), OwnershipState::MutBorrow);
                            }
                            Some(OwnershipState::Moved { .. }) => {
                                return Err(OwnershipError::UseAfterMove { name: var_name.clone() });
                            }
                            None => {}
                        }
                    } else {
                        // Immutable borrow
                        match self.vars.get(var_name) {
                            Some(OwnershipState::Owned) => {
                                self.vars.insert(var_name.clone(), OwnershipState::ImmBorrow { count: 1 });
                            }
                            Some(OwnershipState::ImmBorrow { count }) => {
                                self.vars.insert(var_name.clone(), OwnershipState::ImmBorrow { count: count + 1 });
                            }
                            Some(OwnershipState::MutBorrow) => {
                                return Err(OwnershipError::MutBorrowConflict { name: var_name.clone() });
                            }
                            Some(OwnershipState::Moved { .. }) => {
                                return Err(OwnershipError::UseAfterMove { name: var_name.clone() });
                            }
                            None => {}
                        }
                    }
                }
            }

            CheckedExpr::Deref { operand, result_ty: _ } => {
                self.check_expr(operand)?;
            }

            CheckedExpr::MethodCall { receiver, args, .. } => {
                self.check_expr(receiver)?;
                for arg in args {
                    self.check_expr(arg)?;
                }
            }

            CheckedExpr::StaticCall { type_name: _, method: _, args, .. } => {
                for arg in args {
                    self.check_expr(arg)?;
                }
            }

            CheckedExpr::FieldAccess { receiver, field: _, result_ty: _ } => {
                self.check_expr(receiver)?;
            }

            CheckedExpr::PathAccess { type_name: _, name: _, result_ty: _ } => {
                // Constants/enum variants — no ownership check needed
            }

            CheckedExpr::Index { target, index, result_ty: _ } => {
                self.check_expr(target)?;
                self.check_expr(index)?;
            }

            CheckedExpr::AsmBlock { instructions, outputs, inputs } => {
                // Check output variables
                for (name, _) in outputs {
                    // Output variables are borrowed mutably
                    if let Some(state) = self.vars.get(name) {
                        match state {
                            OwnershipState::MutBorrow => {
                                // Already mutably borrowed, that's fine for asm
                            }
                            OwnershipState::Owned => {
                                self.vars.insert(name.clone(), OwnershipState::MutBorrow);
                            }
                            _ => {}
                        }
                    }
                }
                // Check input variables
                for (name, _) in inputs {
                    if let Some(state) = self.vars.get(name) {
                        match state {
                            OwnershipState::Moved { .. } => {
                                return Err(OwnershipError::UseAfterMove { name: name.clone() });
                            }
                            _ => {}
                        }
                    }
                }
            }

            CheckedExpr::Cast { expr: inner, target_ty: _ } => {
                self.check_expr(inner)?;
            }

            CheckedExpr::Match { scrutinee, arms, result_ty: _ } => {
                self.check_expr(scrutinee)?;
                for arm in arms {
                    if let Some(ref guard) = arm.guard {
                        self.check_expr(guard)?;
                    }
                    self.check_expr(&arm.body)?;
                }
            }

            CheckedExpr::StructLit { fields, .. } => {
                for (_, field_expr) in fields {
                    self.check_expr(field_expr)?;
                }
            }

            // Literals don't need ownership tracking
            CheckedExpr::IntLit(_, _) |
            CheckedExpr::FloatLit(_, _) |
            CheckedExpr::StringLit(_) |
            CheckedExpr::CharLit(_) |
            CheckedExpr::BoolLit(_) |
            CheckedExpr::Undefined(_) => {}
        }

        Ok(())
    }
}
