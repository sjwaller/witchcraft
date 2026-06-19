//! Structural tests for lowering (type-checked AST -> IR). These assert the
//! *shape* the backend depends on, not generated code: that `divine` becomes a
//! runtime `Decode` plus a confidence branch (never evaluated at lower time),
//! that `enact` becomes a tag `Switch`, and that host control flow becomes
//! basic blocks. Equivalence of *behaviour* is covered once a backend exists.

use witchcraft::ir::{Function, Instr, Program, Terminator};
use witchcraft::lower_source;

const ACTION_TYPES: &str = "\
type Action = one_of {
    Draft(reply: glyph),
    Escalate,
    AskClarify(question: glyph),
}
type Disposition = { urgency: spark in 0..10, action: Action }
";

fn lower(src: &str) -> Program {
    lower_source(src).unwrap_or_else(|ds| {
        panic!(
            "expected program to lower, got: {}",
            ds.iter().map(|d| d.render()).collect::<Vec<_>>().join("; ")
        )
    })
}

fn func<'a>(p: &'a Program, name: &str) -> &'a Function {
    if p.main.name == name {
        return &p.main;
    }
    p.functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no function named `{}`", name))
}

fn all_instrs(f: &Function) -> Vec<&Instr> {
    f.blocks.iter().flat_map(|b| b.instrs.iter()).collect()
}

fn terminators(f: &Function) -> Vec<&Terminator> {
    f.blocks.iter().map(|b| &b.term).collect()
}

#[test]
fn host_function_has_params_and_returns_last_expression() {
    let p = lower("define add(a, b) { a + b }");
    let add = func(&p, "add");
    assert_eq!(add.params.len(), 2, "two declared parameters");
    assert!(
        all_instrs(add).iter().any(|i| matches!(
            i,
            Instr::Bin {
                op: witchcraft::ast::BinOp::Add,
                ..
            }
        )),
        "the body adds its parameters"
    );
    assert!(
        terminators(add)
            .iter()
            .any(|t| matches!(t, Terminator::Return(Some(_)))),
        "fall-through returns the last expression's value"
    );
}

#[test]
fn top_level_statements_become_main() {
    let p = lower("define add(a, b) { a + b }\nspeak add(2, 3)");
    let main = func(&p, "main");
    assert!(
        all_instrs(main)
            .iter()
            .any(|i| matches!(i, Instr::Speak { .. })),
        "the top-level speak lives in main"
    );
    assert!(
        all_instrs(main)
            .iter()
            .any(|i| matches!(i, Instr::Call { callee, .. } if callee == "add")),
        "the call to add is lowered in main"
    );
}

#[test]
fn while_loop_lowers_to_branching_blocks() {
    let p = lower("var n = 0\nwhile n < 3 { speak n n = n + 1 }");
    let main = func(&p, "main");
    assert!(
        main.blocks.len() >= 4,
        "a loop introduces header/body/exit blocks, got {}",
        main.blocks.len()
    );
    assert!(
        terminators(main)
            .iter()
            .any(|t| matches!(t, Terminator::Branch { .. })),
        "the loop condition lowers to a branch"
    );
}

#[test]
fn divine_with_threshold_emits_decode_grammar_and_discharge_branch() {
    let src = format!(
        "{ACTION_TYPES}
oracle o = summon \"m\"
divine d: Disposition from (\"t\") using o with confidence >= 0.8 fallback \"fb\"
speak d.urgency
"
    );
    let p = lower(&src);
    assert_eq!(p.grammars.len(), 1, "one divine site compiles one grammar");

    let main = func(&p, "main");
    let intent = all_instrs(main)
        .into_iter()
        .find_map(|i| match i {
            Instr::Decode { intent, .. } => Some(intent.clone()),
            _ => None,
        })
        .expect("divine lowers to a runtime Decode (inference is never resolved at lower time)");
    assert_eq!(
        intent, "m",
        "the decode carries the oracle's semantic intent (the manifest binds it to a model)"
    );

    assert!(
        terminators(main)
            .iter()
            .any(|t| matches!(t, Terminator::Branch { .. })),
        "discharge lowers to a confidence branch"
    );
    // The failure side of discharge returns (the fallback unwinds the function).
    assert!(
        terminators(main)
            .iter()
            .any(|t| matches!(t, Terminator::Return(_))),
        "the fallback path returns"
    );
}

#[test]
fn undischarged_divine_emits_make_inferred_and_no_branch() {
    let src = format!(
        "{ACTION_TYPES}
oracle o = summon \"m\"
divine d: Disposition from (\"t\") using o
speak \"ok\"
"
    );
    let p = lower(&src);
    let main = func(&p, "main");
    assert!(
        all_instrs(main)
            .iter()
            .any(|i| matches!(i, Instr::MakeInferred { .. })),
        "an undischarged divine binds an inferred value"
    );
    assert!(
        all_instrs(main).iter().all(|i| !matches!(
            i,
            Instr::Bin {
                op: witchcraft::ast::BinOp::Ge,
                ..
            }
        )),
        "no confidence comparison without a threshold"
    );
}

#[test]
fn enact_lowers_to_a_tag_switch_and_interns_variants() {
    let src = format!(
        "{ACTION_TYPES}
oracle o = summon \"m\"
divine d: Disposition from (\"t\") using o with confidence >= 0.0 fallback \"fb\"
enact d.action {{
    Draft(reply) => {{ speak \"drafted\" }}
    Escalate => {{ speak \"escalated\" }}
    AskClarify(question) => {{ speak \"asked\" }}
}}
"
    );
    let p = lower(&src);
    let main = func(&p, "main");
    let switch = terminators(main)
        .into_iter()
        .find_map(|t| match t {
            Terminator::Switch { arms, .. } => Some(arms.clone()),
            _ => None,
        })
        .expect("enact lowers to a Switch on the variant tag");
    assert_eq!(switch.len(), 3, "one switch arm per enact arm");

    for v in ["Draft", "Escalate", "AskClarify"] {
        assert!(
            p.variant_names.iter().any(|n| n == v),
            "variant `{}` is interned",
            v
        );
    }
    assert!(
        all_instrs(main)
            .iter()
            .any(|i| matches!(i, Instr::VariantTag { .. })),
        "dispatch reads the variant's tag"
    );
}

#[test]
fn logical_and_short_circuits_through_blocks() {
    // `false and <rhs>` must not evaluate the rhs; lowering proves this with a branch.
    let p = lower("var hit = false\nlet x = false and (hit == false)\nspeak x");
    let main = func(&p, "main");
    assert!(
        terminators(main)
            .iter()
            .any(|t| matches!(t, Terminator::Branch { .. })),
        "short-circuit `and` lowers to control flow"
    );
}
