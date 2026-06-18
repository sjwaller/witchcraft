//! The Witchcraft compiled runtime.
//!
//! `grimoire build` (group 5) links this crate into every native executable, so
//! a shipped artifact carries its own value model, reference counting, and (in
//! later groups) the decoder/oracle seam — and needs no Rust to run.
//!
//! Layout:
//! * [`value`] — the `#[repr(C)]` tagged value, constructors, accessors, display.
//! * [`heap`]  — the narrow alloc / [`retain`](heap::retain) /
//!   [`release`](heap::release) seam and reference-counting policy (design D8).
//! * [`abi`]   — the `extern "C"` symbols the Cranelift backend emits calls to.
//!
//! Logical and display semantics intentionally match the interpreter's value
//! model so the compiled and interpreted paths agree (design D6).

pub mod abi;
pub mod heap;
pub mod value;

pub use heap::{live_objects, release, retain};
pub use value::{
    boolean, concat, display, field, glyph, glyph_to_string, inferred, inferred_confidence,
    inferred_inner, provenance, record, render, spark, unit, variant, variant_field, variant_tag,
    Provenance, Value,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_are_unboxed_and_allocate_nothing() {
        let before = live_objects();
        let _ = spark(3.0);
        let _ = boolean(true);
        let _ = unit();
        assert_eq!(live_objects(), before, "scalars never touch the heap");
        assert_eq!(as_spark_round(spark(2.5)), 2.5);
    }

    fn as_spark_round(v: Value) -> f64 {
        value::as_spark(v)
    }

    #[test]
    fn glyph_allocates_and_release_reclaims() {
        let before = live_objects();
        let g = glyph("witch");
        assert_eq!(live_objects(), before + 1);
        assert_eq!(glyph_to_string(g), "witch");
        release(g);
        assert_eq!(
            live_objects(),
            before,
            "release at refcount zero frees the glyph"
        );
    }

    #[test]
    fn retain_then_release_balances() {
        let before = live_objects();
        let g = glyph("x");
        retain(g); // refcount 2
        release(g); // back to 1; still live
        assert_eq!(live_objects(), before + 1, "still one live object");
        release(g); // 0; freed
        assert_eq!(live_objects(), before);
    }

    #[test]
    fn releasing_a_record_releases_its_children() {
        let before = live_objects();
        let name = glyph("Ada");
        let rec = record(vec![("name".into(), name)], None); // owns the glyph's ref
        assert_eq!(live_objects(), before + 2, "record + its child glyph");
        // Field read borrows; we don't retain, so we must not release it.
        assert_eq!(glyph_to_string(field(rec, "name")), "Ada");
        release(rec);
        assert_eq!(
            live_objects(),
            before,
            "freeing the record frees the child glyph too"
        );
    }

    #[test]
    fn variant_tag_and_fields_round_trip() {
        let before = live_objects();
        let reply = glyph("hello");
        let v = variant("Draft", 0, vec![("reply".into(), reply)], None);
        assert_eq!(variant_tag(v), 0);
        assert_eq!(glyph_to_string(variant_field(v, 0)), "hello");
        release(v);
        assert_eq!(live_objects(), before);
    }

    #[test]
    fn inferred_carries_inner_confidence_and_provenance() {
        let before = live_objects();
        let prov = Provenance {
            oracle: "triage".into(),
            model: "mock".into(),
            seed: 1,
        };
        let inf = inferred(spark(7.0), 0.9, prov.clone());
        assert_eq!(value::as_spark(inferred_inner(inf)), 7.0);
        assert_eq!(inferred_confidence(inf), 0.9);
        assert_eq!(provenance(inf), Some(prov));
        release(inf);
        assert_eq!(live_objects(), before);
    }

    #[test]
    fn loop_local_glyphs_are_reclaimed_mid_run() {
        // The D8 motivation: a bounded program with a large loop allocating a
        // glyph per iteration must not grow without bound. Each iteration's value
        // is released as it falls out of scope, so live count stays flat.
        let baseline = live_objects();
        let mut peak = baseline;
        for i in 0..10_000 {
            let g = render(spark(i as f64)); // allocate a glyph this iteration
            assert_eq!(glyph_to_string(g), value::fmt_num(i as f64));
            release(g); // out of scope at end of iteration
            peak = peak.max(live_objects());
        }
        assert_eq!(
            live_objects(),
            baseline,
            "no values retained past their scope"
        );
        assert!(
            peak - baseline <= 1,
            "at most one loop-local value live at a time, saw {}",
            peak - baseline
        );
    }

    #[test]
    fn display_matches_interpreter_formatting() {
        // Mirrors witchcraft::value::Value::display so the compiled `print` agrees.
        assert_eq!(display(spark(3.0)), "3");
        assert_eq!(display(spark(2.5)), "2.5");
        assert_eq!(display(boolean(false)), "false");
        assert_eq!(display(unit()), "()");

        let g = glyph("hi");
        assert_eq!(display(g), "hi");
        release(g);

        let rec = record(vec![("urgency".into(), spark(8.0))], None);
        assert_eq!(display(rec), "{ urgency: 8 }");
        release(rec);

        let nullary = variant("Escalate", 1, vec![], None);
        assert_eq!(display(nullary), "Escalate");
        release(nullary);
    }

    #[test]
    fn concat_builds_a_glyph_from_parts() {
        let before = live_objects();
        let a = glyph("hi ");
        let b = render(spark(42.0));
        let joined = concat(&[a, b]);
        assert_eq!(glyph_to_string(joined), "hi 42");
        release(a);
        release(b);
        release(joined);
        assert_eq!(live_objects(), before);
    }
}
