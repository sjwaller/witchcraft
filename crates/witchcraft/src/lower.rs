//! Lowering: type-checked AST → [`ir`]. Assumes the program already passed
//! `typeck::check_program` (the backend never sees ill-typed programs, design
//! D1). Control flow becomes basic blocks; locals become named slots; `divine`
//! becomes a runtime `Decode` plus a confidence branch; `enact` becomes a tag
//! `Switch`. Inference is never evaluated here.

use std::collections::HashMap;

use crate::ast::*;
use crate::error::Diagnostic;
use crate::grammar::{self, Grammar};
use crate::ir::{
    Block, BlockId, Function, GrammarId, Instr, LocalId, Operand, Terminator, Tmp, VariantTagId,
};
use crate::typeck::{build_type_table, resolve_type};
use crate::types::Type;

pub fn lower_program(prog: &Program) -> Result<crate::ir::Program, Diagnostic> {
    lower_program_weaken(prog, false)
}

/// Lower a program, optionally **weakening** every `divine` output type to free
/// text (as if the type were deleted). This is the compiled litmus knob: building
/// the same program weakened vs not, under one seed, must change generation.
pub fn lower_program_weaken(
    prog: &Program,
    weaken: bool,
) -> Result<crate::ir::Program, Diagnostic> {
    let types = build_type_table(prog);
    let mut ctx = LowerCtx {
        types,
        oracles: HashMap::new(),
        grammars: Vec::new(),
        variant_names: Vec::new(),
        functions: Vec::new(),
        weaken,
    };

    // Oracles are resolved statically (declared, referenced by name in `divine`).
    collect_oracles(prog, &mut ctx);

    // User functions.
    for item in &prog.items {
        if let Item::Fn(f) = item {
            let func = ctx.lower_function(&f.name, &f.params, &f.body)?;
            ctx.functions.push(func);
        }
    }

    // Top-level statements form `main`.
    let main_stmts: Vec<Stmt> = prog
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Stmt(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    let main = ctx.lower_function("main", &[], &main_stmts)?;

    Ok(crate::ir::Program {
        functions: ctx.functions,
        main,
        grammars: ctx.grammars,
        variant_names: ctx.variant_names,
    })
}

fn collect_oracles(prog: &Program, ctx: &mut LowerCtx) {
    fn walk(stmts: &[Stmt], ctx: &mut LowerCtx) {
        for s in stmts {
            match s {
                Stmt::Summon { name, model, .. } => {
                    ctx.oracles.insert(name.clone(), model.clone());
                }
                Stmt::While { body, .. } => walk(body, ctx),
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    walk(then_branch, ctx);
                    if let Some(eb) = else_branch {
                        walk(eb, ctx);
                    }
                }
                Stmt::Enact { arms, .. } => {
                    for a in arms {
                        walk(&a.body, ctx);
                    }
                }
                Stmt::Grant { body, .. } => walk(body, ctx),
                _ => {}
            }
        }
    }
    for item in &prog.items {
        match item {
            Item::Stmt(s) => walk(std::slice::from_ref(s), ctx),
            Item::Fn(f) => walk(&f.body, ctx),
            Item::Type(_) => {}
        }
    }
}

struct LowerCtx {
    types: HashMap<String, Type>,
    oracles: HashMap<String, String>,
    grammars: Vec<Grammar>,
    variant_names: Vec<String>,
    functions: Vec<Function>,
    weaken: bool,
}

impl LowerCtx {
    fn intern_variant(&mut self, name: &str) -> VariantTagId {
        if let Some(i) = self.variant_names.iter().position(|n| n == name) {
            return i as VariantTagId;
        }
        self.variant_names.push(name.to_string());
        (self.variant_names.len() - 1) as VariantTagId
    }

    fn add_grammar(&mut self, g: Grammar) -> GrammarId {
        self.grammars.push(g);
        (self.grammars.len() - 1) as GrammarId
    }

    fn lower_function(
        &mut self,
        name: &str,
        params: &[Param],
        body: &[Stmt],
    ) -> Result<Function, Diagnostic> {
        let mut fb = FnBuilder::new();
        fb.push_scope();
        let mut param_locals = Vec::new();
        for p in params {
            let l = fb.declare_local(&p.name);
            param_locals.push(l);
        }
        self.lower_block(&mut fb, body)?;
        // Fall-through return: the value of the last expression statement, else unit.
        let ret = fb.take_last_value();
        fb.set_term(Terminator::Return(ret));
        fb.pop_scope();
        Ok(Function {
            name: name.to_string(),
            params: param_locals,
            num_locals: fb.next_local,
            blocks: fb.finish(),
            entry: 0,
        })
    }

    fn lower_block(&mut self, fb: &mut FnBuilder, stmts: &[Stmt]) -> Result<(), Diagnostic> {
        fb.push_scope();
        for s in stmts {
            self.lower_stmt(fb, s)?;
        }
        fb.pop_scope();
        Ok(())
    }

    fn lower_stmt(&mut self, fb: &mut FnBuilder, s: &Stmt) -> Result<(), Diagnostic> {
        match s {
            Stmt::Let { name, value, .. } | Stmt::Var { name, value, .. } => {
                let op = self.lower_expr(fb, value)?;
                let l = fb.declare_local(name);
                fb.emit(Instr::StoreLocal { local: l, src: op });
                fb.clear_last_value();
            }
            Stmt::Assign { name, value, span } => {
                let op = self.lower_expr(fb, value)?;
                let l = fb.lookup_local(name).ok_or_else(|| {
                    Diagnostic::runtime(format!("undefined variable `{}`", name), *span)
                })?;
                fb.emit(Instr::StoreLocal { local: l, src: op });
                fb.clear_last_value();
            }
            Stmt::Print { value, .. } => {
                let op = self.lower_expr(fb, value)?;
                fb.emit(Instr::Print { val: op });
                fb.clear_last_value();
            }
            Stmt::Summon { .. } => {
                // Oracles are resolved statically; no runtime instruction.
                fb.clear_last_value();
            }
            Stmt::Return { value, .. } => {
                let op = match value {
                    Some(e) => Some(self.lower_expr(fb, e)?),
                    None => None,
                };
                fb.set_term(Terminator::Return(op));
                // Subsequent statements are dead; continue in a fresh block.
                let dead = fb.new_block();
                fb.switch_to(dead);
                fb.clear_last_value();
            }
            Stmt::While { cond, body, .. } => {
                let header = fb.new_block();
                let body_blk = fb.new_block();
                let exit = fb.new_block();
                fb.set_term(Terminator::Jump(header));
                fb.switch_to(header);
                let c = self.lower_expr(fb, cond)?;
                fb.set_term(Terminator::Branch {
                    cond: c,
                    then_blk: body_blk,
                    else_blk: exit,
                });
                fb.switch_to(body_blk);
                self.lower_block(fb, body)?;
                fb.set_term(Terminator::Jump(header));
                fb.switch_to(exit);
                fb.clear_last_value();
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                let c = self.lower_expr(fb, cond)?;
                let then_blk = fb.new_block();
                let else_blk = fb.new_block();
                let join = fb.new_block();
                fb.set_term(Terminator::Branch {
                    cond: c,
                    then_blk,
                    else_blk,
                });
                fb.switch_to(then_blk);
                self.lower_block(fb, then_branch)?;
                fb.set_term(Terminator::Jump(join));
                fb.switch_to(else_blk);
                if let Some(eb) = else_branch {
                    self.lower_block(fb, eb)?;
                }
                fb.set_term(Terminator::Jump(join));
                fb.switch_to(join);
                fb.clear_last_value();
            }
            Stmt::Divine(d) => {
                self.lower_divine(fb, d)?;
                fb.clear_last_value();
            }
            Stmt::Enact { subject, arms, .. } => {
                self.lower_enact(fb, subject, arms)?;
                fb.clear_last_value();
            }
            // Capabilities are erased before codegen: a grant region lowers to
            // its body as an ordinary lexical block.
            Stmt::Grant { body, .. } => {
                self.lower_block(fb, body)?;
                fb.clear_last_value();
            }
            Stmt::Expr(e) => {
                let op = self.lower_expr(fb, e)?;
                fb.set_last_value(op);
            }
        }
        Ok(())
    }

    fn lower_divine(&mut self, fb: &mut FnBuilder, d: &DivineStmt) -> Result<(), Diagnostic> {
        let out_ty = resolve_type(&d.out_ty, &self.types)
            .map_err(|e| Diagnostic::type_error(e.message, d.span))?;
        let g = grammar::compile(&out_ty, self.weaken, d.span)?;
        let gid = self.add_grammar(g);
        let model = self
            .oracles
            .get(&d.oracle)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        let mut inputs = Vec::new();
        for e in &d.inputs {
            inputs.push(self.lower_expr(fb, e)?);
        }

        let dst_val = fb.fresh_tmp();
        let dst_conf = fb.fresh_tmp();
        fb.emit(Instr::Decode {
            dst_val,
            dst_conf,
            grammar: gid,
            oracle: d.oracle.clone(),
            model,
            inputs,
        });

        match d.threshold {
            Some(threshold) => {
                // discharge: confidence >= θ ? bind value : run fallback and return
                let cmp = fb.fresh_tmp();
                fb.emit(Instr::Bin {
                    dst: cmp,
                    op: BinOp::Ge,
                    lhs: Operand::Tmp(dst_conf),
                    rhs: Operand::Spark(threshold),
                });
                let cont = fb.new_block();
                let fail = fb.new_block();
                fb.set_term(Terminator::Branch {
                    cond: Operand::Tmp(cmp),
                    then_blk: cont,
                    else_blk: fail,
                });
                // failure path: evaluate fallback and unwind (matches interpreter)
                fb.switch_to(fail);
                let fb_op = match &d.fallback {
                    Some(e) => Some(self.lower_expr(fb, e)?),
                    None => None,
                };
                fb.set_term(Terminator::Return(fb_op));
                // success path: bind the discharged value
                fb.switch_to(cont);
                let l = fb.declare_local(&d.name);
                fb.emit(Instr::StoreLocal {
                    local: l,
                    src: Operand::Tmp(dst_val),
                });
            }
            None => {
                // undischarged: bind an inferred value
                let inf = fb.fresh_tmp();
                fb.emit(Instr::MakeInferred {
                    dst: inf,
                    val: Operand::Tmp(dst_val),
                    conf: Operand::Tmp(dst_conf),
                    oracle: d.oracle.clone(),
                    model: self
                        .oracles
                        .get(&d.oracle)
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string()),
                });
                let l = fb.declare_local(&d.name);
                fb.emit(Instr::StoreLocal {
                    local: l,
                    src: Operand::Tmp(inf),
                });
            }
        }
        Ok(())
    }

    fn lower_enact(
        &mut self,
        fb: &mut FnBuilder,
        subject: &Expr,
        arms: &[EnactArm],
    ) -> Result<(), Diagnostic> {
        let subj = self.lower_expr(fb, subject)?;
        let tag = fb.fresh_tmp();
        fb.emit(Instr::VariantTag {
            dst: tag,
            recv: subj.clone(),
        });

        let join = fb.new_block();
        let default = fb.new_block();
        let mut arm_targets = Vec::new();
        let mut arm_blocks = Vec::new();
        for arm in arms {
            let tid = self.intern_variant(&arm.variant);
            let blk = fb.new_block();
            arm_targets.push((tid, blk));
            arm_blocks.push((blk, arm));
        }
        fb.set_term(Terminator::Switch {
            tag: Operand::Tmp(tag),
            arms: arm_targets,
            default,
        });

        // Exhaustiveness is guaranteed by typeck; default is unreachable.
        fb.switch_to(default);
        fb.set_term(Terminator::Unreachable);

        for (blk, arm) in arm_blocks {
            fb.switch_to(blk);
            fb.push_scope();
            for (i, binding) in arm.bindings.iter().enumerate() {
                let dst = fb.fresh_tmp();
                fb.emit(Instr::VariantField {
                    dst,
                    recv: subj.clone(),
                    index: i as u32,
                });
                let l = fb.declare_local(binding);
                fb.emit(Instr::StoreLocal {
                    local: l,
                    src: Operand::Tmp(dst),
                });
            }
            // Thread provenance into the arm, matching the interpreter, so an arm
            // body may reference `provenance`.
            let prov_dst = fb.fresh_tmp();
            fb.emit(Instr::ProvenanceGlyph {
                dst: prov_dst,
                recv: subj.clone(),
            });
            let prov_local = fb.declare_local("provenance");
            fb.emit(Instr::StoreLocal {
                local: prov_local,
                src: Operand::Tmp(prov_dst),
            });
            for s in &arm.body {
                self.lower_stmt(fb, s)?;
            }
            fb.pop_scope();
            fb.set_term(Terminator::Jump(join));
        }

        fb.switch_to(join);
        Ok(())
    }

    fn lower_expr(&mut self, fb: &mut FnBuilder, e: &Expr) -> Result<Operand, Diagnostic> {
        match e {
            Expr::Number(n, _) => Ok(Operand::Spark(*n)),
            Expr::Bool(b, _) => Ok(Operand::Bool(*b)),
            Expr::Str(segs, _) => self.lower_glyph(fb, segs),
            Expr::Ident(name, span) => {
                let l = fb.lookup_local(name).ok_or_else(|| {
                    Diagnostic::runtime(format!("undefined name `{}`", name), *span)
                })?;
                let dst = fb.fresh_tmp();
                fb.emit(Instr::LoadLocal { dst, local: l });
                Ok(Operand::Tmp(dst))
            }
            Expr::Unary { op, rhs, .. } => {
                let v = self.lower_expr(fb, rhs)?;
                let dst = fb.fresh_tmp();
                fb.emit(Instr::Un {
                    dst,
                    op: *op,
                    val: v,
                });
                Ok(Operand::Tmp(dst))
            }
            Expr::Binary { op, lhs, rhs, .. } => self.lower_binary(fb, *op, lhs, rhs),
            Expr::Call { callee, args, .. } => {
                let mut argv = Vec::new();
                for a in args {
                    argv.push(self.lower_expr(fb, a)?);
                }
                let dst = fb.fresh_tmp();
                fb.emit(Instr::Call {
                    dst,
                    callee: callee.clone(),
                    args: argv,
                });
                Ok(Operand::Tmp(dst))
            }
            Expr::Method { span, method, .. } => Err(Diagnostic::runtime(
                format!("method `{}` is not supported by the compiler yet", method),
                *span,
            )),
            Expr::Field { recv, field, .. } => {
                let r = self.lower_expr(fb, recv)?;
                let dst = fb.fresh_tmp();
                fb.emit(Instr::Field {
                    dst,
                    recv: r,
                    field: field.clone(),
                });
                Ok(Operand::Tmp(dst))
            }
            Expr::Variant { name, fields, .. } => {
                let mut fv = Vec::new();
                for (n, fe) in fields {
                    fv.push((n.clone(), self.lower_expr(fb, fe)?));
                }
                let tag = self.intern_variant(name);
                let dst = fb.fresh_tmp();
                fb.emit(Instr::MakeVariant {
                    dst,
                    name: name.clone(),
                    tag,
                    fields: fv,
                });
                Ok(Operand::Tmp(dst))
            }
            // Embeddings, lists, and governed memory are interpreter-only in v0.x;
            // the Cranelift ship path covers the host language + divine/enact core.
            Expr::List { span, .. } => Err(Diagnostic::runtime(
                "list literals are not supported by the compiler yet (use `witch run`)".to_string(),
                *span,
            )),
        }
    }

    fn lower_glyph(&mut self, fb: &mut FnBuilder, segs: &[StrSeg]) -> Result<Operand, Diagnostic> {
        // Single literal segment is a plain glyph constant.
        if let [StrSeg::Lit(t)] = segs {
            let dst = fb.fresh_tmp();
            fb.emit(Instr::Glyph {
                dst,
                text: t.clone(),
            });
            return Ok(Operand::Tmp(dst));
        }
        if segs.is_empty() {
            let dst = fb.fresh_tmp();
            fb.emit(Instr::Glyph {
                dst,
                text: String::new(),
            });
            return Ok(Operand::Tmp(dst));
        }
        let mut parts = Vec::new();
        for seg in segs {
            match seg {
                StrSeg::Lit(t) => {
                    let dst = fb.fresh_tmp();
                    fb.emit(Instr::Glyph {
                        dst,
                        text: t.clone(),
                    });
                    parts.push(Operand::Tmp(dst));
                }
                StrSeg::Interp(inner) => {
                    let v = self.lower_expr(fb, inner)?;
                    let dst = fb.fresh_tmp();
                    fb.emit(Instr::Render { dst, val: v });
                    parts.push(Operand::Tmp(dst));
                }
            }
        }
        let dst = fb.fresh_tmp();
        fb.emit(Instr::Concat { dst, parts });
        Ok(Operand::Tmp(dst))
    }

    fn lower_binary(
        &mut self,
        fb: &mut FnBuilder,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
    ) -> Result<Operand, Diagnostic> {
        // Short-circuiting logical operators lower to control flow + a result slot.
        if matches!(op, BinOp::And | BinOp::Or) {
            let result = fb.fresh_anon_local();
            let l = self.lower_expr(fb, lhs)?;
            let rhs_blk = fb.new_block();
            let short_blk = fb.new_block();
            let join = fb.new_block();
            // `and`: if lhs then evaluate rhs else short-circuit false.
            // `or`:  if lhs short-circuit true else evaluate rhs.
            let (then_blk, else_blk) = match op {
                BinOp::And => (rhs_blk, short_blk),
                _ => (short_blk, rhs_blk),
            };
            fb.set_term(Terminator::Branch {
                cond: l,
                then_blk,
                else_blk,
            });
            fb.switch_to(rhs_blk);
            let r = self.lower_expr(fb, rhs)?;
            fb.emit(Instr::StoreLocal {
                local: result,
                src: r,
            });
            fb.set_term(Terminator::Jump(join));
            fb.switch_to(short_blk);
            fb.emit(Instr::StoreLocal {
                local: result,
                src: Operand::Bool(matches!(op, BinOp::Or)),
            });
            fb.set_term(Terminator::Jump(join));
            fb.switch_to(join);
            let dst = fb.fresh_tmp();
            fb.emit(Instr::LoadLocal { dst, local: result });
            return Ok(Operand::Tmp(dst));
        }

        let l = self.lower_expr(fb, lhs)?;
        let r = self.lower_expr(fb, rhs)?;
        let dst = fb.fresh_tmp();
        fb.emit(Instr::Bin {
            dst,
            op,
            lhs: l,
            rhs: r,
        });
        Ok(Operand::Tmp(dst))
    }
}

/// Per-function block builder with lexical scopes mapping names to local slots.
struct FnBuilder {
    blocks: Vec<Block>,
    cur: BlockId,
    next_tmp: u32,
    next_local: u32,
    scopes: Vec<HashMap<String, LocalId>>,
    last_value: Option<Operand>,
}

impl FnBuilder {
    fn new() -> Self {
        let entry = Block {
            id: 0,
            instrs: Vec::new(),
            term: Terminator::Unreachable,
        };
        FnBuilder {
            blocks: vec![entry],
            cur: 0,
            next_tmp: 0,
            next_local: 0,
            scopes: Vec::new(),
            last_value: None,
        }
    }

    fn new_block(&mut self) -> BlockId {
        let id = self.blocks.len() as BlockId;
        self.blocks.push(Block {
            id,
            instrs: Vec::new(),
            term: Terminator::Unreachable,
        });
        id
    }

    fn switch_to(&mut self, id: BlockId) {
        self.cur = id;
    }

    fn emit(&mut self, instr: Instr) {
        self.blocks[self.cur as usize].instrs.push(instr);
    }

    fn set_term(&mut self, term: Terminator) {
        self.blocks[self.cur as usize].term = term;
    }

    fn fresh_tmp(&mut self) -> Tmp {
        let t = self.next_tmp;
        self.next_tmp += 1;
        t
    }

    fn fresh_anon_local(&mut self) -> LocalId {
        let l = self.next_local;
        self.next_local += 1;
        l
    }

    fn declare_local(&mut self, name: &str) -> LocalId {
        let l = self.next_local;
        self.next_local += 1;
        self.scopes
            .last_mut()
            .expect("a scope is open")
            .insert(name.to_string(), l);
        l
    }

    fn lookup_local(&self, name: &str) -> Option<LocalId> {
        for scope in self.scopes.iter().rev() {
            if let Some(l) = scope.get(name) {
                return Some(*l);
            }
        }
        None
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn set_last_value(&mut self, op: Operand) {
        self.last_value = Some(op);
    }

    fn clear_last_value(&mut self) {
        self.last_value = None;
    }

    fn take_last_value(&mut self) -> Option<Operand> {
        self.last_value.take()
    }

    fn finish(self) -> Vec<Block> {
        self.blocks
    }
}
