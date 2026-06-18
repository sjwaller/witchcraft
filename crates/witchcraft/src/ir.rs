//! The lowering IR: a small, explicit, backend-facing representation that sits
//! between the type checker and code generation (design D1). It is deliberately
//! NOT the AST — control flow is reduced to basic blocks with terminators, and
//! locals are named slots (so a backend like Cranelift can construct SSA via its
//! own variable mechanism without us emitting phi nodes).
//!
//! Operands are either an SSA temporary (`Tmp`, the result of one instruction)
//! or an immediate scalar. Everything heap-shaped (glyph/record/variant/inferred)
//! is produced by an instruction into a `Tmp`.

use crate::ast::{BinOp, UnOp};
use crate::grammar::Grammar;

pub type Tmp = u32;
pub type LocalId = u32;
pub type BlockId = u32;
pub type GrammarId = u32;
/// Interned variant-name id, shared by `MakeVariant`, `VariantTag`, and `enact`
/// dispatch so construction and matching agree on the tag.
pub type VariantTagId = u32;

#[derive(Clone, Debug, PartialEq)]
pub enum Operand {
    Tmp(Tmp),
    Spark(f64),
    Bool(bool),
    Unit,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Instr {
    /// Read a local slot into a fresh temporary.
    LoadLocal {
        dst: Tmp,
        local: LocalId,
    },
    /// Write an operand into a local slot.
    StoreLocal {
        local: LocalId,
        src: Operand,
    },

    /// A glyph (text) literal.
    Glyph {
        dst: Tmp,
        text: String,
    },
    /// Render any value to its glyph form (for interpolation / print).
    Render {
        dst: Tmp,
        val: Operand,
    },
    /// Concatenate glyph operands left to right.
    Concat {
        dst: Tmp,
        parts: Vec<Operand>,
    },

    Bin {
        dst: Tmp,
        op: BinOp,
        lhs: Operand,
        rhs: Operand,
    },
    Un {
        dst: Tmp,
        op: UnOp,
        val: Operand,
    },

    MakeRecord {
        dst: Tmp,
        fields: Vec<(String, Operand)>,
    },
    MakeVariant {
        dst: Tmp,
        name: String,
        tag: VariantTagId,
        fields: Vec<(String, Operand)>,
    },
    /// Read a record/variant field by name.
    Field {
        dst: Tmp,
        recv: Operand,
        field: String,
    },
    /// Read a variant payload by positional index (for `enact` bindings).
    VariantField {
        dst: Tmp,
        recv: Operand,
        index: u32,
    },
    /// Read a variant's interned tag id (for `enact` dispatch).
    VariantTag {
        dst: Tmp,
        recv: Operand,
    },
    /// A glyph rendering of a value's provenance (what `enact` binds to
    /// `provenance`); empty glyph when the value carries none.
    ProvenanceGlyph {
        dst: Tmp,
        recv: Operand,
    },

    Call {
        dst: Tmp,
        callee: String,
        args: Vec<Operand>,
    },
    Print {
        val: Operand,
    },

    /// The inference primitive: generate a value constrained by `grammar` via the
    /// runtime decoder, producing the value and a confidence scalar. Inference is
    /// a runtime call — never resolved during lowering (design D3).
    Decode {
        dst_val: Tmp,
        dst_conf: Tmp,
        grammar: GrammarId,
        /// The semantic intent (the oracle's summon string). The manifest binds
        /// it to a concrete engine; with no manifest the Mock serves it.
        intent: String,
        /// The evaluated `from (...)` input, rendered to a single glyph (the
        /// prompt the engine receives). Inference is still a runtime call.
        input: Operand,
    },
    /// Wrap a value + confidence into an inferred value (undischarged `divine`).
    /// Provenance is taken from the immediately-preceding [`Instr::Decode`]
    /// (the engine that produced the value), so it is faithful across engines.
    MakeInferred {
        dst: Tmp,
        val: Operand,
        conf: Operand,
    },

    /// A list literal `[a, b, ...]` (also produced by `nearest`/`recent`).
    MakeList {
        dst: Tmp,
        items: Vec<Operand>,
    },
    /// `oracle.embed(input)` — a deterministic embedding in the oracle's space.
    /// `oracle` is the binding name (provenance); `space` is the resolved model.
    Embed {
        dst: Tmp,
        oracle: String,
        space: String,
        input: Operand,
    },
    /// `similarity(a, b)` — cosine similarity of two embeddings (a spark).
    Similarity {
        dst: Tmp,
        lhs: Operand,
        rhs: Operand,
    },
    /// `nearest(query, candidates, k)` — the k nearest candidate embeddings.
    Nearest {
        dst: Tmp,
        query: Operand,
        candidates: Operand,
        k: Operand,
    },

    /// `memory <name> { scope, retention, audit }` registration.
    MemRegister {
        name: String,
        scope: String,
        retention: Option<f64>,
        audit: bool,
    },
    /// `mem.write(value)` — append to a governed memory store.
    MemWrite {
        name: String,
        value: Operand,
    },
    /// `mem.recent(k)` / `mem.nearest(..., k)` — newest-first retrieval (a list).
    /// `method` is the source op name for the audit log.
    MemRecent {
        dst: Tmp,
        name: String,
        method: String,
        k: Operand,
    },
    /// `advance(n)` — advance the logical clock (deterministic retention testing).
    Advance {
        n: Operand,
    },
    /// `audit_log()` — the accumulated audit records (a list of glyphs).
    AuditLog {
        dst: Tmp,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Terminator {
    Jump(BlockId),
    Branch {
        cond: Operand,
        then_blk: BlockId,
        else_blk: BlockId,
    },
    /// Dispatch on an interned variant tag (for `enact`).
    Switch {
        tag: Operand,
        arms: Vec<(VariantTagId, BlockId)>,
        default: BlockId,
    },
    Return(Option<Operand>),
    /// Statically unreachable (e.g. an exhaustive `enact`'s default). A backend
    /// may lower this to a trap.
    Unreachable,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub id: BlockId,
    pub instrs: Vec<Instr>,
    pub term: Terminator,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub params: Vec<LocalId>,
    pub num_locals: u32,
    pub blocks: Vec<Block>,
    pub entry: BlockId,
}

/// A program's inference need and the POLICY in force at its `divine` sites,
/// derived from the source's capability grants. A compiled binary resolves every
/// need against the manifest at load (refuse-to-start), exactly as the
/// interpreter does.
#[derive(Clone, Debug, PartialEq)]
pub struct Need {
    pub intent: String,
    pub allow_network: bool,
    pub allow_downgrade: bool,
}

#[derive(Clone, Debug)]
pub struct Program {
    /// User functions, by name.
    pub functions: Vec<Function>,
    /// The implicit entry function holding top-level statements.
    pub main: Function,
    /// Compiled grammars referenced by `Decode` instructions (by `GrammarId`).
    pub grammars: Vec<Grammar>,
    /// Interned variant names; a `VariantTagId` indexes this table.
    pub variant_names: Vec<String>,
    /// Inference needs + policies, for load-time manifest resolution.
    pub needs: Vec<Need>,
}

impl Function {
    pub fn block(&self, id: BlockId) -> &Block {
        &self.blocks[id as usize]
    }
}
