//! Recursive-descent parser: tokens -> AST. Operator precedence (low to high):
//! `or`, `and`, comparison, `+ -`, `* /`, unary, postfix (call/method/field).

use crate::ast::*;
use crate::error::Diagnostic;
use crate::lexer::lex;
use crate::span::Span;
use crate::token::{StrPart, Token, TokenKind};

pub fn parse(src: &str) -> Result<Program, Diagnostic> {
    let tokens = lex(src)?;
    let mut p = Parser::new(tokens);
    p.parse_program()
}

/// Parse a single expression from raw source (used for `${...}` interpolation).
fn parse_expr_str(src: &str, anchor: Span) -> Result<Expr, Diagnostic> {
    let tokens = lex(src).map_err(|d| Diagnostic::parse(d.message, anchor))?;
    let mut p = Parser::new(tokens);
    let e = p.expr()?;
    p.expect(TokenKind::Eof, "end of interpolation expression")?;
    Ok(e)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn span(&self) -> Span {
        self.tokens[self.pos].span
    }

    fn bump(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn check(&self, k: &TokenKind) -> bool {
        std::mem::discriminant(self.peek_kind()) == std::mem::discriminant(k)
    }

    fn eat(&mut self, k: &TokenKind) -> bool {
        if self.check(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, k: TokenKind, what: &str) -> Result<Token, Diagnostic> {
        if self.check(&k) {
            Ok(self.bump())
        } else {
            Err(Diagnostic::parse(
                format!("expected {}, found {}", what, describe(self.peek_kind())),
                self.span(),
            ))
        }
    }

    fn expect_ident(&mut self, what: &str) -> Result<(String, Span), Diagnostic> {
        let span = self.span();
        match self.peek_kind().clone() {
            TokenKind::Ident(name) => {
                self.bump();
                Ok((name, span))
            }
            other => Err(Diagnostic::parse(
                format!("expected {}, found {}", what, describe(&other)),
                span,
            )),
        }
    }

    // ---- top level ----

    fn parse_program(&mut self) -> Result<Program, Diagnostic> {
        let mut items = Vec::new();
        while !self.check(&TokenKind::Eof) {
            items.push(self.item()?);
        }
        Ok(Program { items })
    }

    fn item(&mut self) -> Result<Item, Diagnostic> {
        match self.peek_kind() {
            TokenKind::Fn => Ok(Item::Fn(self.fn_decl()?)),
            TokenKind::Type => Ok(Item::Type(self.type_decl()?)),
            _ => Ok(Item::Stmt(self.stmt()?)),
        }
    }

    fn fn_decl(&mut self) -> Result<FnDecl, Diagnostic> {
        let span = self.span();
        self.expect(TokenKind::Fn, "`fn`")?;
        let (name, _) = self.expect_ident("a function name")?;
        self.expect(TokenKind::LParen, "`(`")?;
        let mut params = Vec::new();
        while !self.check(&TokenKind::RParen) {
            let (pname, pspan) = self.expect_ident("a parameter name")?;
            let ty = if self.eat(&TokenKind::Colon) {
                Some(self.type_expr()?)
            } else {
                None
            };
            params.push(Param {
                name: pname,
                ty,
                span: pspan,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen, "`)`")?;
        let ret = if self.eat(&TokenKind::Colon) {
            Some(self.type_expr()?)
        } else {
            None
        };
        let requires = if self.eat(&TokenKind::Requires) {
            self.capability_list()?
        } else {
            Vec::new()
        };
        let body = self.block()?;
        Ok(FnDecl {
            name,
            params,
            ret,
            requires,
            body,
            span,
        })
    }

    /// One or more comma-separated capabilities: `kind(param), kind, ...`.
    fn capability_list(&mut self) -> Result<Vec<Capability>, Diagnostic> {
        let mut caps = vec![self.capability()?];
        while self.eat(&TokenKind::Comma) {
            caps.push(self.capability()?);
        }
        Ok(caps)
    }

    /// A single capability: `kind` or `kind(param)`.
    fn capability(&mut self) -> Result<Capability, Diagnostic> {
        let (kind, span) = self.expect_ident("a capability kind")?;
        let param = if self.eat(&TokenKind::LParen) {
            let (p, _) = self.expect_ident("a capability parameter")?;
            self.expect(TokenKind::RParen, "`)` to close the capability parameter")?;
            Some(p)
        } else {
            None
        };
        Ok(Capability { kind, param, span })
    }

    fn type_decl(&mut self) -> Result<TypeDecl, Diagnostic> {
        let span = self.span();
        self.expect(TokenKind::Type, "`type`")?;
        let (name, _) = self.expect_ident("a type name")?;
        self.expect(TokenKind::Eq, "`=`")?;
        let ty = self.type_expr()?;
        Ok(TypeDecl { name, ty, span })
    }

    fn type_expr(&mut self) -> Result<TypeExpr, Diagnostic> {
        let span = self.span();
        match self.peek_kind().clone() {
            TokenKind::LBrace => {
                self.bump();
                let mut fields = Vec::new();
                while !self.check(&TokenKind::RBrace) {
                    let (fname, _) = self.expect_ident("a field name")?;
                    self.expect(TokenKind::Colon, "`:`")?;
                    let fty = self.type_expr()?;
                    fields.push((fname, fty));
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RBrace, "`}`")?;
                Ok(TypeExpr::Record(fields, span))
            }
            TokenKind::OneOf => {
                self.bump();
                self.expect(TokenKind::LBrace, "`{`")?;
                let mut variants = Vec::new();
                while !self.check(&TokenKind::RBrace) {
                    let (vname, vspan) = self.expect_ident("a variant name")?;
                    let mut vfields = Vec::new();
                    if self.eat(&TokenKind::LParen) {
                        while !self.check(&TokenKind::RParen) {
                            let (fname, _) = self.expect_ident("a variant field name")?;
                            self.expect(TokenKind::Colon, "`:`")?;
                            let fty = self.type_expr()?;
                            vfields.push((fname, fty));
                            if !self.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                        self.expect(TokenKind::RParen, "`)`")?;
                    }
                    variants.push(VariantDef {
                        name: vname,
                        fields: vfields,
                        span: vspan,
                    });
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RBrace, "`}`")?;
                Ok(TypeExpr::OneOf(variants, span))
            }
            TokenKind::Ident(base) => {
                self.bump();
                if self.eat(&TokenKind::In) {
                    let lo = self.number("a lower bound")?;
                    self.expect(TokenKind::DotDot, "`..`")?;
                    let hi = self.number("an upper bound")?;
                    Ok(TypeExpr::Refined { base, lo, hi, span })
                } else {
                    Ok(TypeExpr::Named(base, span))
                }
            }
            TokenKind::Oracle => {
                self.bump();
                Ok(TypeExpr::Named("oracle".to_string(), span))
            }
            other => Err(Diagnostic::parse(
                format!("expected a type, found {}", describe(&other)),
                span,
            )),
        }
    }

    fn number(&mut self, what: &str) -> Result<f64, Diagnostic> {
        match self.peek_kind().clone() {
            TokenKind::Number(n) => {
                self.bump();
                Ok(n)
            }
            other => Err(Diagnostic::parse(
                format!("expected {}, found {}", what, describe(&other)),
                self.span(),
            )),
        }
    }

    fn block(&mut self) -> Result<Vec<Stmt>, Diagnostic> {
        self.expect(TokenKind::LBrace, "`{`")?;
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            stmts.push(self.stmt()?);
            self.eat(&TokenKind::Comma); // tolerate separators
        }
        self.expect(TokenKind::RBrace, "`}` to close the block")?;
        Ok(stmts)
    }

    // ---- statements ----

    fn stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let span = self.span();
        match self.peek_kind().clone() {
            TokenKind::Let => {
                self.bump();
                let (name, _) = self.expect_ident("a binding name")?;
                let ty = if self.eat(&TokenKind::Colon) {
                    Some(self.type_expr()?)
                } else {
                    None
                };
                self.expect(TokenKind::Eq, "`=`")?;
                let value = self.expr()?;
                Ok(Stmt::Let {
                    name,
                    ty,
                    value,
                    span,
                })
            }
            TokenKind::Var => {
                self.bump();
                let (name, _) = self.expect_ident("a variable name")?;
                let ty = if self.eat(&TokenKind::Colon) {
                    Some(self.type_expr()?)
                } else {
                    None
                };
                self.expect(TokenKind::Eq, "`=`")?;
                let value = self.expr()?;
                Ok(Stmt::Var {
                    name,
                    ty,
                    value,
                    span,
                })
            }
            TokenKind::Oracle => {
                self.bump();
                let (name, _) = self.expect_ident("an oracle name")?;
                self.expect(TokenKind::Eq, "`=`")?;
                self.expect(TokenKind::Summon, "`summon`")?;
                let model = self.string_literal_simple("a model id")?;
                Ok(Stmt::Summon { name, model, span })
            }
            TokenKind::Print => {
                self.bump();
                let value = self.expr()?;
                Ok(Stmt::Print { value, span })
            }
            TokenKind::While => {
                self.bump();
                let cond = self.expr()?;
                let body = self.block()?;
                Ok(Stmt::While { cond, body, span })
            }
            TokenKind::If => {
                self.bump();
                let cond = self.expr()?;
                let then_branch = self.block()?;
                let else_branch = if self.eat(&TokenKind::Else) {
                    Some(self.block()?)
                } else {
                    None
                };
                Ok(Stmt::If {
                    cond,
                    then_branch,
                    else_branch,
                    span,
                })
            }
            TokenKind::Return => {
                self.bump();
                let value = if self.check(&TokenKind::RBrace) || self.check(&TokenKind::Eof) {
                    None
                } else {
                    Some(self.expr()?)
                };
                Ok(Stmt::Return { value, span })
            }
            TokenKind::With => {
                self.bump();
                self.expect(TokenKind::Grant, "`grant` (the capabilities to grant)")?;
                let caps = self.capability_list()?;
                let body = self.block()?;
                Ok(Stmt::Grant { caps, body, span })
            }
            TokenKind::Divine => self.divine_stmt(),
            TokenKind::Enact => self.enact_stmt(),
            TokenKind::Ident(name) => {
                // assignment `name = expr` vs expression statement
                if matches!(&self.tokens[self.pos + 1].kind, TokenKind::Eq) {
                    self.bump(); // ident
                    self.bump(); // '='
                    let value = self.expr()?;
                    Ok(Stmt::Assign { name, value, span })
                } else {
                    Ok(Stmt::Expr(self.expr()?))
                }
            }
            _ => Ok(Stmt::Expr(self.expr()?)),
        }
    }

    fn divine_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let span = self.span();
        self.expect(TokenKind::Divine, "`divine`")?;
        let (name, _) = self.expect_ident("a result name")?;
        self.expect(TokenKind::Colon, "`:` then the output type")?;
        let out_ty = self.type_expr()?;
        self.expect(TokenKind::From, "`from` (the inference inputs)")?;
        self.expect(TokenKind::LParen, "`(`")?;
        let mut inputs = Vec::new();
        while !self.check(&TokenKind::RParen) {
            inputs.push(self.expr()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen, "`)`")?;
        self.expect(TokenKind::Using, "`using` (the oracle)")?;
        let (oracle, oracle_span) = self.expect_ident("an oracle name")?;
        // Discharge clause is optional in the grammar; the type system enforces it.
        let (threshold, fallback) = if self.eat(&TokenKind::With) {
            self.expect(TokenKind::Confidence, "`confidence`")?;
            self.expect(TokenKind::Ge, "`>=`")?;
            let t = self.number("a confidence threshold")?;
            self.expect(TokenKind::Fallback, "`fallback <expr>`")?;
            let f = self.expr()?;
            (Some(t), Some(f))
        } else {
            (None, None)
        };
        Ok(Stmt::Divine(DivineStmt {
            name,
            out_ty,
            inputs,
            oracle,
            oracle_span,
            threshold,
            fallback,
            span,
        }))
    }

    fn enact_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let span = self.span();
        self.expect(TokenKind::Enact, "`enact`")?;
        let subject = self.expr()?;
        self.expect(TokenKind::LBrace, "`{` then the action arms")?;
        let mut arms = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let aspan = self.span();
            let (variant, _) = self.expect_ident("a variant name")?;
            let mut bindings = Vec::new();
            if self.eat(&TokenKind::LParen) {
                while !self.check(&TokenKind::RParen) {
                    let (b, _) = self.expect_ident("a field binding")?;
                    bindings.push(b);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RParen, "`)`")?;
            }
            self.expect(TokenKind::FatArrow, "`=>`")?;
            let body = self.block()?;
            arms.push(EnactArm {
                variant,
                bindings,
                body,
                span: aspan,
            });
            self.eat(&TokenKind::Comma);
        }
        self.expect(TokenKind::RBrace, "`}` to close enact")?;
        Ok(Stmt::Enact {
            subject,
            arms,
            span,
        })
    }

    fn string_literal_simple(&mut self, what: &str) -> Result<String, Diagnostic> {
        let span = self.span();
        match self.peek_kind().clone() {
            TokenKind::Str(parts) => {
                self.bump();
                let mut s = String::new();
                for part in parts {
                    match part {
                        StrPart::Lit(t) => s.push_str(&t),
                        StrPart::Expr(_) => {
                            return Err(Diagnostic::parse(
                                "interpolation is not allowed here",
                                span,
                            ))
                        }
                    }
                }
                Ok(s)
            }
            other => Err(Diagnostic::parse(
                format!("expected {}, found {}", what, describe(&other)),
                span,
            )),
        }
    }

    // ---- expressions (precedence climbing) ----

    fn expr(&mut self) -> Result<Expr, Diagnostic> {
        self.or_expr()
    }

    fn or_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.and_expr()?;
        while self.check(&TokenKind::Or) {
            let span = self.span();
            self.bump();
            let rhs = self.and_expr()?;
            lhs = Expr::Binary {
                op: BinOp::Or,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn and_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.cmp_expr()?;
        while self.check(&TokenKind::And) {
            let span = self.span();
            self.bump();
            let rhs = self.cmp_expr()?;
            lhs = Expr::Binary {
                op: BinOp::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn cmp_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.add_expr()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Le => BinOp::Le,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::Ge => BinOp::Ge,
                TokenKind::EqEq => BinOp::Eq,
                TokenKind::Ne => BinOp::Ne,
                _ => break,
            };
            let span = self.span();
            self.bump();
            let rhs = self.add_expr()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn add_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.mul_expr()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            let span = self.span();
            self.bump();
            let rhs = self.mul_expr()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn mul_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.unary_expr()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                _ => break,
            };
            let span = self.span();
            self.bump();
            let rhs = self.unary_expr()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn unary_expr(&mut self) -> Result<Expr, Diagnostic> {
        let span = self.span();
        match self.peek_kind() {
            TokenKind::Not => {
                self.bump();
                let rhs = self.unary_expr()?;
                Ok(Expr::Unary {
                    op: UnOp::Not,
                    rhs: Box::new(rhs),
                    span,
                })
            }
            TokenKind::Minus => {
                self.bump();
                let rhs = self.unary_expr()?;
                Ok(Expr::Unary {
                    op: UnOp::Neg,
                    rhs: Box::new(rhs),
                    span,
                })
            }
            _ => self.postfix_expr(),
        }
    }

    fn postfix_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut e = self.primary()?;
        loop {
            if self.check(&TokenKind::Dot) {
                let span = self.span();
                self.bump();
                let (name, _) = self.expect_ident("a field or method name")?;
                if self.check(&TokenKind::LParen) {
                    let args = self.call_args()?;
                    e = Expr::Method {
                        recv: Box::new(e),
                        method: name,
                        args,
                        span,
                    };
                } else {
                    e = Expr::Field {
                        recv: Box::new(e),
                        field: name,
                        span,
                    };
                }
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn call_args(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        self.expect(TokenKind::LParen, "`(`")?;
        let mut args = Vec::new();
        while !self.check(&TokenKind::RParen) {
            args.push(self.expr()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen, "`)`")?;
        Ok(args)
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let span = self.span();
        match self.peek_kind().clone() {
            TokenKind::Number(n) => {
                self.bump();
                Ok(Expr::Number(n, span))
            }
            TokenKind::True => {
                self.bump();
                Ok(Expr::Bool(true, span))
            }
            TokenKind::False => {
                self.bump();
                Ok(Expr::Bool(false, span))
            }
            TokenKind::Str(parts) => {
                self.bump();
                let mut segs = Vec::new();
                for part in parts {
                    match part {
                        StrPart::Lit(t) => segs.push(StrSeg::Lit(t)),
                        StrPart::Expr(raw) => {
                            let e = parse_expr_str(&raw, span)?;
                            segs.push(StrSeg::Interp(Box::new(e)));
                        }
                    }
                }
                Ok(Expr::Str(segs, span))
            }
            TokenKind::LParen => {
                self.bump();
                let e = self.expr()?;
                self.expect(TokenKind::RParen, "`)`")?;
                Ok(e)
            }
            TokenKind::LBracket => {
                self.bump();
                let mut items = Vec::new();
                while !self.check(&TokenKind::RBracket) {
                    items.push(self.expr()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RBracket, "`]` to close the list")?;
                Ok(Expr::List { items, span })
            }
            TokenKind::Ident(name) => {
                self.bump();
                let is_variant = name.chars().next().is_some_and(|c| c.is_uppercase());
                if self.check(&TokenKind::LParen) {
                    if is_variant {
                        let fields = self.variant_fields()?;
                        Ok(Expr::Variant { name, fields, span })
                    } else {
                        let args = self.call_args()?;
                        Ok(Expr::Call {
                            callee: name,
                            args,
                            span,
                        })
                    }
                } else if is_variant {
                    Ok(Expr::Variant {
                        name,
                        fields: Vec::new(),
                        span,
                    })
                } else {
                    Ok(Expr::Ident(name, span))
                }
            }
            other => Err(Diagnostic::parse(
                format!("expected an expression, found {}", describe(&other)),
                span,
            )),
        }
    }

    fn variant_fields(&mut self) -> Result<Vec<(String, Expr)>, Diagnostic> {
        self.expect(TokenKind::LParen, "`(`")?;
        let mut fields = Vec::new();
        while !self.check(&TokenKind::RParen) {
            // `field: expr` form
            let (fname, fspan) = self.expect_ident("a field name")?;
            if self.eat(&TokenKind::Colon) {
                let value = self.expr()?;
                fields.push((fname, value));
            } else {
                // shorthand: bare identifier is both field and value
                fields.push((fname.clone(), Expr::Ident(fname, fspan)));
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen, "`)`")?;
        Ok(fields)
    }
}

fn describe(k: &TokenKind) -> String {
    match k {
        TokenKind::Eof => "end of input".to_string(),
        TokenKind::Ident(s) => format!("identifier `{}`", s),
        TokenKind::Number(n) => format!("number `{}`", n),
        TokenKind::Str(_) => "a glyph literal".to_string(),
        other => format!("`{:?}`", other),
    }
}
