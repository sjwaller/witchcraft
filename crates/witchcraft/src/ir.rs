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
        /// The oracle binding name and its model id, for provenance.
        oracle: String,
        model: String,
        inputs: Vec<Operand>,
    },
    /// Wrap a value + confidence into an inferred value (undischarged `divine`).
    MakeInferred {
        dst: Tmp,
        val: Operand,
        conf: Operand,
        oracle: String,
        model: String,
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
}

impl Function {
    pub fn block(&self, id: BlockId) -> &Block {
        &self.blocks[id as usize]
    }
}
