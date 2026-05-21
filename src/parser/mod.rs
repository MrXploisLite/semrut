pub mod ast;

use crate::lexer::Token;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParserError {
    #[error("expected {expected}, got {got} at {pos}")]
    UnexpectedToken {
        expected: String,
        got: String,
        pos: String,
    },

    #[error("unexpected end of file at {pos}")]
    UnexpectedEof { pos: String },
}

type Result<T> = std::result::Result<T, ParserError>;

pub use ast::*;

/// Simple recursive descent parser (handwritten for now, LALRPOP later)
pub fn parse(tokens: &[Token], _filename: &str) -> Result<Program> {
    let mut p = Parser { tokens, pos: 0 };
    p.parse_program()
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: &str) -> Result<&Token> {
        let tok = self.advance();
        let desc = token_desc(&tok.kind);
        if desc != expected && !matches_expected(&tok.kind, expected) {
            return Err(ParserError::UnexpectedToken {
                expected: expected.to_string(),
                got: desc,
                pos: tok.span.start.to_string(),
            });
        }
        Ok(tok)
    }

    fn parse_program(&mut self) -> Result<Program> {
        let mut items = Vec::new();

        while self.peek().kind != crate::lexer::TokenKind::Eof {
            let item = self.parse_item()?;
            items.push(item);
        }

        Ok(Program { items })
    }

    fn parse_item(&mut self) -> Result<Item> {
        // Check for pub
        let is_pub = if matches_kind(&self.peek().kind, "Pub") {
            self.advance();
            true
        } else {
            false
        };

        match &self.peek().kind {
            crate::lexer::TokenKind::Fn => self.parse_fn_item(is_pub),
            crate::lexer::TokenKind::Struct => self.parse_struct_item(is_pub),
            crate::lexer::TokenKind::Enum => self.parse_enum_item(is_pub),
            crate::lexer::TokenKind::Const => self.parse_const_item(is_pub),
            crate::lexer::TokenKind::Impl => self.parse_impl_item(),
            _ => Err(ParserError::UnexpectedToken {
                expected: "fn, struct, enum, const, or impl".to_string(),
                got: token_desc(&self.peek().kind),
                pos: self.peek().span.start.to_string(),
            }),
        }
    }

    fn parse_fn_item(&mut self, is_pub: bool) -> Result<Item> {
        self.expect("Fn")?;
        let name_tok = self.advance();
        let name = match &name_tok.kind {
            crate::lexer::TokenKind::Ident(s) => s.clone(),
            _ => {
                return Err(ParserError::UnexpectedToken {
                    expected: "identifier".to_string(),
                    got: token_desc(&name_tok.kind),
                    pos: name_tok.span.start.to_string(),
                });
            }
        };

        // Parse optional type parameters: <T, U>
        let type_params = self.parse_type_params()?;

        self.expect("LParen")?;
        let mut params = Vec::new();
        if !matches_kind(&self.peek().kind, "RParen") {
            loop {
                let param = self.parse_param()?;
                params.push(param);
                if !matches_kind(&self.peek().kind, "Comma") {
                    break;
                }
                self.advance();
            }
        }
        self.expect("RParen")?;

        let ret_type = if matches_kind(&self.peek().kind, "Arrow") {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_block()?;

        Ok(Item::Fn(FnItem {
            name,
            type_params,
            params,
            ret_type,
            body,
            is_pub,
        }))
    }

    fn parse_type_params(&mut self) -> Result<Vec<String>> {
        if !matches_kind(&self.peek().kind, "Lt") {
            return Ok(Vec::new());
        }
        self.advance(); // consume '<'

        let mut params = Vec::new();
        if !matches_kind(&self.peek().kind, "Gt") {
            loop {
                let tok = self.advance();
                let name = match &tok.kind {
                    crate::lexer::TokenKind::Ident(s) => s.clone(),
                    _ => {
                        return Err(ParserError::UnexpectedToken {
                            expected: "type parameter name".to_string(),
                            got: token_desc(&tok.kind),
                            pos: tok.span.start.to_string(),
                        });
                    }
                };
                params.push(name);

                if matches_kind(&self.peek().kind, "Comma") {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect("Gt")?;
        Ok(params)
    }

    fn parse_param(&mut self) -> Result<Param> {
        let name_tok = self.advance();
        let name = match &name_tok.kind {
            crate::lexer::TokenKind::Ident(s) => s.clone(),
            _ => {
                return Err(ParserError::UnexpectedToken {
                    expected: "parameter name".to_string(),
                    got: token_desc(&name_tok.kind),
                    pos: name_tok.span.start.to_string(),
                });
            }
        };

        self.expect("Colon")?;
        let ty = self.parse_type()?;

        Ok(Param { name, ty })
    }

    fn parse_type(&mut self) -> Result<Type> {
        match &self.peek().kind {
            crate::lexer::TokenKind::Ident(s) => {
                let name = s.clone();
                self.advance();
                // Check for generic: vec128<f32> or Pair<T, U>
                if matches_kind(&self.peek().kind, "Lt") {
                    self.advance();
                    let mut args = Vec::new();
                    if !matches_kind(&self.peek().kind, "Gt") {
                        loop {
                            let inner = self.parse_type()?;
                            args.push(inner);
                            if matches_kind(&self.peek().kind, "Comma") {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect("Gt")?;
                    Ok(Type::Generic { name, args })
                } else {
                    Ok(Type::Named(name))
                }
            }
            crate::lexer::TokenKind::Amp => {
                self.advance();
                let mutable = if matches_kind(&self.peek().kind, "Mut") {
                    self.advance();
                    true
                } else {
                    false
                };
                let inner = self.parse_type()?;
                Ok(Type::Ref {
                    mutable,
                    inner: Box::new(inner),
                })
            }
            crate::lexer::TokenKind::LBracket => {
                self.advance();
                let inner = self.parse_type()?;
                if matches_kind(&self.peek().kind, "Semi") {
                    self.advance();
                    let len_tok = self.advance();
                    let len = match &len_tok.kind {
                        crate::lexer::TokenKind::NumberLit(n) => *n as usize,
                        _ => {
                            return Err(ParserError::UnexpectedToken {
                                expected: "array length".to_string(),
                                got: token_desc(&len_tok.kind),
                                pos: len_tok.span.start.to_string(),
                            });
                        }
                    };
                    self.expect("RBracket")?;
                    Ok(Type::Array {
                        inner: Box::new(inner),
                        len,
                    })
                } else {
                    self.expect("RBracket")?;
                    Ok(Type::Slice {
                        inner: Box::new(inner),
                    })
                }
            }
            _ => Err(ParserError::UnexpectedToken {
                expected: "type".to_string(),
                got: token_desc(&self.peek().kind),
                pos: self.peek().span.start.to_string(),
            }),
        }
    }

    fn parse_block(&mut self) -> Result<Block> {
        self.expect("LBrace")?;
        let mut stmts = Vec::new();

        while !matches_kind(&self.peek().kind, "RBrace") {
            let stmt = self.parse_stmt()?;
            stmts.push(stmt);
        }

        self.expect("RBrace")?;
        Ok(Block { stmts })
    }

    fn parse_stmt(&mut self) -> Result<Stmt> {
        match &self.peek().kind {
            crate::lexer::TokenKind::Let => self.parse_let_stmt(),
            crate::lexer::TokenKind::Return => self.parse_return_stmt(),
            crate::lexer::TokenKind::If => self.parse_if_stmt(),
            crate::lexer::TokenKind::While => self.parse_while_stmt(),
            crate::lexer::TokenKind::Loop => self.parse_loop_stmt(),
            crate::lexer::TokenKind::Unsafe => self.parse_unsafe_block(),
            crate::lexer::TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Stmt::Block(block))
            }
            _ => {
                // Expression statement
                let expr = self.parse_expr()?;
                if matches_kind(&self.peek().kind, "Semi") {
                    self.advance();
                    Ok(Stmt::Expr(expr, true))
                } else {
                    Ok(Stmt::Expr(expr, false))
                }
            }
        }
    }

    fn parse_let_stmt(&mut self) -> Result<Stmt> {
        self.expect("Let")?;
        let mutable = if matches_kind(&self.peek().kind, "Mut") {
            self.advance();
            true
        } else {
            false
        };

        let name_tok = self.advance();
        let name = match &name_tok.kind {
            crate::lexer::TokenKind::Ident(s) => s.clone(),
            _ => {
                return Err(ParserError::UnexpectedToken {
                    expected: "identifier".to_string(),
                    got: token_desc(&name_tok.kind),
                    pos: name_tok.span.start.to_string(),
                });
            }
        };

        let ty = if matches_kind(&self.peek().kind, "Colon") {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect("Eq")?;
        let value = self.parse_expr()?;

        // Optional semicolon
        if matches_kind(&self.peek().kind, "Semi") {
            self.advance();
        }

        Ok(Stmt::Let(LetStmt {
            name,
            ty,
            value,
            mutable,
        }))
    }

    fn parse_return_stmt(&mut self) -> Result<Stmt> {
        self.expect("Return")?;
        let value = if !matches_kind(&self.peek().kind, "Semi")
            && !matches_kind(&self.peek().kind, "RBrace")
        {
            Some(self.parse_expr()?)
        } else {
            None
        };

        if matches_kind(&self.peek().kind, "Semi") {
            self.advance();
        }

        Ok(Stmt::Return(ReturnStmt { value }))
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt> {
        self.expect("If")?;
        let cond = self.parse_expr()?;
        let then_block = self.parse_block()?;
        let else_block = if matches_kind(&self.peek().kind, "Else") {
            self.advance();
            if matches_kind(&self.peek().kind, "If") {
                let inner = self.parse_if_stmt()?;
                Some(Block {
                    stmts: vec![inner],
                })
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };

        Ok(Stmt::If(IfStmt {
            cond,
            then_block,
            else_block,
        }))
    }

    fn parse_while_stmt(&mut self) -> Result<Stmt> {
        self.expect("While")?;
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::While(WhileStmt { cond, body }))
    }

    fn parse_loop_stmt(&mut self) -> Result<Stmt> {
        self.expect("Loop")?;
        let body = self.parse_block()?;
        Ok(Stmt::Loop(LoopStmt { body }))
    }

    fn parse_unsafe_block(&mut self) -> Result<Stmt> {
        self.expect("Unsafe")?;
        let body = self.parse_block()?;
        Ok(Stmt::Unsafe(body))
    }

    fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr> {
        let left = self.parse_or()?;

        if matches_kind(&self.peek().kind, "Eq") {
            // Check if left is an identifier (simple assignment)
            self.advance();
            let right = self.parse_assignment()?;
            Ok(Expr::Assign(Box::new(left), Box::new(right)))
        } else {
            Ok(left)
        }
    }

    fn parse_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_and()?;

        while matches_kind(&self.peek().kind, "OrOr") {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Binary {
                op: BinOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_equality()?;

        while matches_kind(&self.peek().kind, "AndAnd") {
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::Binary {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr> {
        let mut left = self.parse_comparison()?;

        loop {
            let op = match &self.peek().kind {
                crate::lexer::TokenKind::EqEq => BinOp::Eq,
                crate::lexer::TokenKind::NeEq => BinOp::Ne,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut left = self.parse_additive()?;

        loop {
            let op = match &self.peek().kind {
                crate::lexer::TokenKind::Lt => BinOp::Lt,
                crate::lexer::TokenKind::Gt => BinOp::Gt,
                crate::lexer::TokenKind::Le => BinOp::Le,
                crate::lexer::TokenKind::Ge => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr> {
        let mut left = self.parse_multiplicative()?;

        loop {
            let op = match &self.peek().kind {
                crate::lexer::TokenKind::Plus => BinOp::Add,
                crate::lexer::TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr> {
        let mut left = self.parse_unary()?;

        loop {
            let op = match &self.peek().kind {
                crate::lexer::TokenKind::Star => BinOp::Mul,
                crate::lexer::TokenKind::Slash => BinOp::Div,
                crate::lexer::TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        match &self.peek().kind {
            crate::lexer::TokenKind::Minus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    operand: Box::new(expr),
                })
            }
            crate::lexer::TokenKind::Bang => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    operand: Box::new(expr),
                })
            }
            crate::lexer::TokenKind::Amp => {
                self.advance();
                let mutable = if matches_kind(&self.peek().kind, "Mut") {
                    self.advance();
                    true
                } else {
                    false
                };
                let expr = self.parse_unary()?;
                Ok(Expr::RefExpr {
                    mutable,
                    operand: Box::new(expr),
                })
            }
            crate::lexer::TokenKind::Star => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::Deref(Box::new(expr)))
            }
            _ => self.parse_call(),
        }
    }

    fn parse_call(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;

        loop {
            match &self.peek().kind {
                crate::lexer::TokenKind::LParen => {
                    self.advance();
                    let mut args = Vec::new();
                    if !matches_kind(&self.peek().kind, "RParen") {
                        loop {
                            let arg = self.parse_expr()?;
                            args.push(arg);
                            if !matches_kind(&self.peek().kind, "Comma") {
                                break;
                            }
                            self.advance();
                        }
                    }
                    self.expect("RParen")?;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                    };
                }
                crate::lexer::TokenKind::Dot => {
                    self.advance();
                    let field_tok = self.advance();
                    let field = match &field_tok.kind {
                        crate::lexer::TokenKind::Ident(s) => s.clone(),
                        _ => {
                            return Err(ParserError::UnexpectedToken {
                                expected: "field name".to_string(),
                                got: token_desc(&field_tok.kind),
                                pos: field_tok.span.start.to_string(),
                            });
                        }
                    };

                    // Check if it's a method call
                    if matches_kind(&self.peek().kind, "LParen") {
                        self.advance();
                        let mut args = Vec::new();
                        if !matches_kind(&self.peek().kind, "RParen") {
                            loop {
                                let arg = self.parse_expr()?;
                                args.push(arg);
                                if !matches_kind(&self.peek().kind, "Comma") {
                                    break;
                                }
                                self.advance();
                            }
                        }
                        self.expect("RParen")?;
                        expr = Expr::MethodCall {
                            receiver: Box::new(expr),
                            method: field,
                            args,
                        };
                    } else {
                        expr = Expr::FieldAccess {
                            receiver: Box::new(expr),
                            field,
                        };
                    }
                }
                crate::lexer::TokenKind::DoubleColon => {
                    // Type::method() or Type::CONST
                    self.advance();
                    let name_tok = self.advance();
                    let name = match &name_tok.kind {
                        crate::lexer::TokenKind::Ident(s) => s.clone(),
                        _ => {
                            return Err(ParserError::UnexpectedToken {
                                expected: "identifier after ::".to_string(),
                                got: token_desc(&name_tok.kind),
                                pos: name_tok.span.start.to_string(),
                            });
                        }
                    };

                    // Check if it's a call
                    if matches_kind(&self.peek().kind, "LParen") {
                        self.advance();
                        let mut args = Vec::new();
                        if !matches_kind(&self.peek().kind, "RParen") {
                            loop {
                                let arg = self.parse_expr()?;
                                args.push(arg);
                                if !matches_kind(&self.peek().kind, "Comma") {
                                    break;
                                }
                                self.advance();
                            }
                        }
                        self.expect("RParen")?;
                        // expr should be a Var (the type name)
                        let type_name = match &expr {
                            Expr::Var(name) => name.clone(),
                            _ => {
                                return Err(ParserError::UnexpectedToken {
                                    expected: "type name before ::".to_string(),
                                    got: "expression".to_string(),
                                    pos: "0".to_string(),
                                });
                            }
                        };
                        expr = Expr::StaticCall {
                            type_name,
                            method: name,
                            args,
                        };
                    } else {
                        // Type::CONST or Type::FIELD
                        let type_name = match &expr {
                            Expr::Var(name) => name.clone(),
                            _ => {
                                return Err(ParserError::UnexpectedToken {
                                    expected: "type name before ::".to_string(),
                                    got: "expression".to_string(),
                                    pos: "0".to_string(),
                                });
                            }
                        };
                        expr = Expr::PathAccess {
                            type_name,
                            name,
                        };
                    }
                }
                crate::lexer::TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect("RBracket")?;
                    expr = Expr::Index {
                        target: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        match &self.peek().kind {
            crate::lexer::TokenKind::NumberLit(n) => {
                let val = *n;
                self.advance();
                Ok(Expr::IntLit(val))
            }
            crate::lexer::TokenKind::FloatLit(n) => {
                let val = *n;
                self.advance();
                Ok(Expr::FloatLit(val))
            }
            crate::lexer::TokenKind::StringLit(s) => {
                let val = s.clone();
                self.advance();
                Ok(Expr::StringLit(val))
            }
            crate::lexer::TokenKind::CharLit(c) => {
                let val = *c;
                self.advance();
                Ok(Expr::CharLit(val))
            }
            crate::lexer::TokenKind::True => {
                self.advance();
                Ok(Expr::BoolLit(true))
            }
            crate::lexer::TokenKind::False => {
                self.advance();
                Ok(Expr::BoolLit(false))
            }
            crate::lexer::TokenKind::Undefined => {
                self.advance();
                Ok(Expr::Undefined)
            }
            crate::lexer::TokenKind::Ident(s) => {
                let name = s.clone();
                self.advance();
                Ok(Expr::Var(name))
            }
            crate::lexer::TokenKind::Match => {
                self.advance();
                let scrutinee = self.parse_expr()?;
                self.expect("LBrace")?;
                let mut arms = Vec::new();
                while !matches_kind(&self.peek().kind, "RBrace") {
                    let pattern = self.parse_pattern()?;
                    let guard = if matches_kind(&self.peek().kind, "If") {
                        self.advance();
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    self.expect("FatArrow")?;
                    let body = self.parse_expr()?;
                    if matches_kind(&self.peek().kind, "Comma") {
                        self.advance();
                    }
                    arms.push(MatchArm { pattern, guard, body });
                }
                self.expect("RBrace")?;
                Ok(Expr::Match { scrutinee: Box::new(scrutinee), arms })
            }
            crate::lexer::TokenKind::Asm => {
                self.advance();
                self.expect("LBrace")?;
                let mut instructions = Vec::new();
                let mut outputs = Vec::new();
                let mut inputs = Vec::new();

                while !matches_kind(&self.peek().kind, "RBrace") {
                    // Check for output: out("reg") var
                    if matches_kind(&self.peek().kind, "Ident") {
                        let ident = match &self.peek().kind {
                            crate::lexer::TokenKind::Ident(s) => s.clone(),
                            _ => unreachable!(),
                        };
                        if ident == "out" || ident == "in" {
                            self.advance();
                            self.expect("LParen")?;
                            let constraint_tok = self.advance();
                            let constraint = match &constraint_tok.kind {
                                crate::lexer::TokenKind::StringLit(s) => s.clone(),
                                _ => {
                                    return Err(ParserError::UnexpectedToken {
                                        expected: "string literal".to_string(),
                                        got: token_desc(&constraint_tok.kind),
                                        pos: constraint_tok.span.start.to_string(),
                                    });
                                }
                            };
                            self.expect("RParen")?;
                            let var_tok = self.advance();
                            let var = match &var_tok.kind {
                                crate::lexer::TokenKind::Ident(s) => s.clone(),
                                _ => {
                                    return Err(ParserError::UnexpectedToken {
                                        expected: "identifier".to_string(),
                                        got: token_desc(&var_tok.kind),
                                        pos: var_tok.span.start.to_string(),
                                    });
                                }
                            };

                            if ident == "out" {
                                outputs.push((constraint, var));
                            } else {
                                inputs.push((constraint, var));
                            }

                            if matches_kind(&self.peek().kind, "Comma") {
                                self.advance();
                            }
                            continue;
                        }
                    }

                    // Instruction string
                    let instr_tok = self.advance();
                    let instr = match &instr_tok.kind {
                        crate::lexer::TokenKind::StringLit(s) => s.clone(),
                        _ => {
                            return Err(ParserError::UnexpectedToken {
                                expected: "instruction string".to_string(),
                                got: token_desc(&instr_tok.kind),
                                pos: instr_tok.span.start.to_string(),
                            });
                        }
                    };
                    instructions.push(instr);

                    if matches_kind(&self.peek().kind, "Comma") {
                        self.advance();
                    }
                }

                self.expect("RBrace")?;

                Ok(Expr::AsmBlock(AsmBlock {
                    instructions,
                    outputs,
                    inputs,
                }))
            }
            crate::lexer::TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect("RParen")?;
                Ok(expr)
            }
            _ => Err(ParserError::UnexpectedToken {
                expected: "expression".to_string(),
                got: token_desc(&self.peek().kind),
                pos: self.peek().span.start.to_string(),
            }),
        }
    }

    fn parse_struct_item(&mut self, _is_pub: bool) -> Result<Item> {
        self.expect("Struct")?;
        let name_tok = self.advance();
        let name = match &name_tok.kind {
            crate::lexer::TokenKind::Ident(s) => s.clone(),
            _ => {
                return Err(ParserError::UnexpectedToken {
                    expected: "identifier".to_string(),
                    got: token_desc(&name_tok.kind),
                    pos: name_tok.span.start.to_string(),
                });
            }
        };

        let type_params = self.parse_type_params()?;

        self.expect("LBrace")?;
        let mut fields = Vec::new();
        while !matches_kind(&self.peek().kind, "RBrace") {
            let field_name_tok = self.advance();
            let field_name = match &field_name_tok.kind {
                crate::lexer::TokenKind::Ident(s) => s.clone(),
                _ => {
                    return Err(ParserError::UnexpectedToken {
                        expected: "field name".to_string(),
                        got: token_desc(&field_name_tok.kind),
                        pos: field_name_tok.span.start.to_string(),
                    });
                }
            };
            self.expect("Colon")?;
            let field_ty = self.parse_type()?;
            if matches_kind(&self.peek().kind, "Comma") {
                self.advance();
            }
            fields.push(StructField {
                name: field_name,
                ty: field_ty,
            });
        }
        self.expect("RBrace")?;

        Ok(Item::Struct(StructItem { name, type_params, fields }))
    }

    fn parse_enum_item(&mut self, _is_pub: bool) -> Result<Item> {
        self.expect("Enum")?;
        let name_tok = self.advance();
        let name = match &name_tok.kind {
            crate::lexer::TokenKind::Ident(s) => s.clone(),
            _ => {
                return Err(ParserError::UnexpectedToken {
                    expected: "identifier".to_string(),
                    got: token_desc(&name_tok.kind),
                    pos: name_tok.span.start.to_string(),
                });
            }
        };

        let type_params = self.parse_type_params()?;

        self.expect("LBrace")?;
        let mut variants = Vec::new();
        while !matches_kind(&self.peek().kind, "RBrace") {
            let var_name_tok = self.advance();
            let var_name = match &var_name_tok.kind {
                crate::lexer::TokenKind::Ident(s) => s.clone(),
                _ => {
                    return Err(ParserError::UnexpectedToken {
                        expected: "variant name".to_string(),
                        got: token_desc(&var_name_tok.kind),
                        pos: var_name_tok.span.start.to_string(),
                    });
                }
            };

            // Check for tuple variant: Some(i64, i64)
            let mut fields = Vec::new();
            if matches_kind(&self.peek().kind, "LParen") {
                self.advance();
                if !matches_kind(&self.peek().kind, "RParen") {
                    loop {
                        let ty = self.parse_type()?;
                        fields.push(ty);
                        if matches_kind(&self.peek().kind, "Comma") {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect("RParen")?;
            }

            variants.push(EnumVariant { name: var_name, fields });

            if matches_kind(&self.peek().kind, "Comma") {
                self.advance();
            }
        }
        self.expect("RBrace")?;

        Ok(Item::Enum(EnumItem { name, type_params, variants }))
    }

    fn parse_const_item(&mut self, _is_pub: bool) -> Result<Item> {
        self.expect("Const")?;
        let name_tok = self.advance();
        let name = match &name_tok.kind {
            crate::lexer::TokenKind::Ident(s) => s.clone(),
            _ => {
                return Err(ParserError::UnexpectedToken {
                    expected: "identifier".to_string(),
                    got: token_desc(&name_tok.kind),
                    pos: name_tok.span.start.to_string(),
                });
            }
        };
        self.expect("Colon")?;
        let ty = self.parse_type()?;
        self.expect("Eq")?;
        let value = self.parse_expr()?;
        if matches_kind(&self.peek().kind, "Semi") {
            self.advance();
        }

        Ok(Item::Const(ConstItem { name, ty, value }))
    }

    fn parse_impl_item(&mut self) -> Result<Item> {
        self.expect("Impl")?;

        // Parse impl type parameters: impl<T> Type<T>
        let type_params = self.parse_type_params()?;

        // Parse target type (e.g., `MyStruct` or `Vec<T>`)
        let target_type = self.parse_type()?;

        self.expect("LBrace")?;

        let mut methods = Vec::new();
        while !matches_kind(&self.peek().kind, "RBrace") {
            // Parse method (same as function but without pub)
            self.expect("Fn")?;
            let name_tok = self.advance();
            let name = match &name_tok.kind {
                crate::lexer::TokenKind::Ident(s) => s.clone(),
                _ => {
                    return Err(ParserError::UnexpectedToken {
                        expected: "method name".to_string(),
                        got: token_desc(&name_tok.kind),
                        pos: name_tok.span.start.to_string(),
                    });
                }
            };

            self.expect("LParen")?;
            let mut params = Vec::new();
            if !matches_kind(&self.peek().kind, "RParen") {
                loop {
                    let name_tok = self.advance();
                    let param_name = match &name_tok.kind {
                        crate::lexer::TokenKind::Ident(s) => s.clone(),
                        _ => {
                            return Err(ParserError::UnexpectedToken {
                                expected: "parameter name".to_string(),
                                got: token_desc(&name_tok.kind),
                                pos: name_tok.span.start.to_string(),
                            });
                        }
                    };
                    self.expect("Colon")?;
                    let ty = self.parse_type()?;
                    params.push(Param { name: param_name, ty });

                    if matches_kind(&self.peek().kind, "Comma") {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            self.expect("RParen")?;

            let ret_type = if matches_kind(&self.peek().kind, "Arrow") {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };

            let body = self.parse_block()?;

            methods.push(FnItem {
                name,
                type_params: Vec::new(), // methods inherit impl's type params
                params,
                ret_type,
                body,
                is_pub: false,
            });
        }

        self.expect("RBrace")?;

        Ok(Item::Impl(ImplItem { target_type, type_params, methods }))
    }

    fn parse_pattern(&mut self) -> Result<Pattern> {
        // Wildcard: _
        if matches_kind(&self.peek().kind, "Underscore") {
            self.advance();
            return Ok(Pattern::Wildcard);
        }

        // Try to parse as EnumVariant::Some(binds) or just identifier
        let first_tok = self.advance();
        let first = match &first_tok.kind {
            crate::lexer::TokenKind::Ident(s) => s.clone(),
            crate::lexer::TokenKind::NumberLit(n) => {
                return Ok(Pattern::Literal { value: *n as i64 });
            }
            _ => {
                return Err(ParserError::UnexpectedToken {
                    expected: "pattern".to_string(),
                    got: token_desc(&first_tok.kind),
                    pos: first_tok.span.start.to_string(),
                });
            }
        };

        // Check if it's Enum::Variant(binds)
        if matches_kind(&self.peek().kind, "DoubleColon") {
            self.advance();
            let variant_tok = self.advance();
            let variant = match &variant_tok.kind {
                crate::lexer::TokenKind::Ident(s) => s.clone(),
                _ => {
                    return Err(ParserError::UnexpectedToken {
                        expected: "variant name".to_string(),
                        got: token_desc(&variant_tok.kind),
                        pos: variant_tok.span.start.to_string(),
                    });
                }
            };

            let mut bindings = Vec::new();
            if matches_kind(&self.peek().kind, "LParen") {
                self.advance();
                if !matches_kind(&self.peek().kind, "RParen") {
                    loop {
                        let bind_tok = self.advance();
                        let bind = match &bind_tok.kind {
                            crate::lexer::TokenKind::Ident(s) => s.clone(),
                            crate::lexer::TokenKind::Underscore => "_".to_string(),
                            _ => {
                                return Err(ParserError::UnexpectedToken {
                                    expected: "binding name".to_string(),
                                    got: token_desc(&bind_tok.kind),
                                    pos: bind_tok.span.start.to_string(),
                                });
                            }
                        };
                        bindings.push(bind);
                        if matches_kind(&self.peek().kind, "Comma") {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect("RParen")?;
            }

            Ok(Pattern::EnumVariant {
                enum_name: first,
                variant,
                bindings,
            })
        } else if matches_kind(&self.peek().kind, "LParen") {
            // Variant(binds) without enum name
            let mut bindings = Vec::new();
            self.advance();
            if !matches_kind(&self.peek().kind, "RParen") {
                loop {
                    let bind_tok = self.advance();
                    let bind = match &bind_tok.kind {
                        crate::lexer::TokenKind::Ident(s) => s.clone(),
                        crate::lexer::TokenKind::Underscore => "_".to_string(),
                        _ => {
                            return Err(ParserError::UnexpectedToken {
                                expected: "binding name".to_string(),
                                got: token_desc(&bind_tok.kind),
                                pos: bind_tok.span.start.to_string(),
                            });
                        }
                    };
                    bindings.push(bind);
                    if matches_kind(&self.peek().kind, "Comma") {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            self.expect("RParen")?;

            Ok(Pattern::EnumVariant {
                enum_name: String::new(), // Will be resolved from type context
                variant: first,
                bindings,
            })
        } else {
            // Simple binding
            Ok(Pattern::Binding { name: first })
        }
    }
}

// Helpers
fn token_desc(kind: &crate::lexer::TokenKind) -> String {
    match kind {
        crate::lexer::TokenKind::Eof => "EOF".to_string(),
        crate::lexer::TokenKind::Ident(s) => format!("identifier `{}`", s),
        crate::lexer::TokenKind::NumberLit(n) => format!("number `{}`", n),
        crate::lexer::TokenKind::FloatLit(n) => format!("float `{}`", n),
        crate::lexer::TokenKind::StringLit(s) => format!("string `\"{}\"`", s),
        crate::lexer::TokenKind::CharLit(c) => format!("char `'{}'`", c),
        _ => format!("{:?}", kind),
    }
}

fn matches_kind(kind: &crate::lexer::TokenKind, name: &str) -> bool {
    token_desc(kind).contains(name) || format!("{:?}", kind).contains(name)
}

fn matches_expected(kind: &crate::lexer::TokenKind, expected: &str) -> bool {
    // For flexible matching
    match (kind, expected) {
        (_, "LParen") => matches!(kind, crate::lexer::TokenKind::LParen),
        (_, "RParen") => matches!(kind, crate::lexer::TokenKind::RParen),
        (_, "LBrace") => matches!(kind, crate::lexer::TokenKind::LBrace),
        (_, "RBrace") => matches!(kind, crate::lexer::TokenKind::RBrace),
        (_, "LBracket") => matches!(kind, crate::lexer::TokenKind::LBracket),
        (_, "RBracket") => matches!(kind, crate::lexer::TokenKind::RBracket),
        (_, "Comma") => matches!(kind, crate::lexer::TokenKind::Comma),
        (_, "Semi") => matches!(kind, crate::lexer::TokenKind::Semi),
        (_, "Colon") => matches!(kind, crate::lexer::TokenKind::Colon),
        (_, "Eq") => matches!(kind, crate::lexer::TokenKind::Eq),
        (_, "Arrow") => matches!(kind, crate::lexer::TokenKind::Arrow),
        (_, "Mut") => matches!(kind, crate::lexer::TokenKind::Mut),
        (_, "Gt") => matches!(kind, crate::lexer::TokenKind::Gt),
        (_, "Lt") => matches!(kind, crate::lexer::TokenKind::Lt),
        _ => false,
    }
}
