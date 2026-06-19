//! The Witchcraft Cranelift backend (codegen group 3).
//!
//! Lowering IR ([`witchcraft::ir`]) → Cranelift IR → native code, linked against
//! [`witchcraft_runtime`]. The backend is written against the generic
//! [`cranelift_module::Module`] trait so the same codegen drives both the JIT
//! (in-process execution / the equivalence harness) now and an object module for
//! `grimoire build` (group 5) later.
//!
//! ## Value ABI
//! A runtime value is the 16-byte `#[repr(C)]` `{ tag, bits }` (see
//! `witchcraft_runtime::value`). In Cranelift it travels as **two `i64`s**
//! `(tag, bits)`; on the supported targets a by-value `{u64,u64}` struct occupies
//! exactly two integer registers, so the runtime's `extern "C"` functions are
//! called with `(tag, bits)` pairs and return values the same way.
//!
//! ## Memory
//! Codegen emits the reference-counting discipline (group 2.3) for the host
//! subset: locals own their stored value (released on overwrite and at scope
//! exit), `LoadLocal` retains, function arguments transfer ownership to the
//! callee, and temporaries are released after the runtime call that borrows
//! them. `release`/`retain` are no-ops for unboxed scalars, so the rule is
//! uniform.
//!
//! ## Scope
//! The full v0.1 language compiles here: the host language (scalars, glyphs +
//! interpolation, arithmetic/comparison, `if`/`while`, `fn`/calls, `print`) plus
//! the AI-native core — `divine` (embedded grammar + runtime decode + discharge),
//! `enact` (variant-tag dispatch), records/variants, inferred values, and
//! provenance threading.

use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::FloatCC;
use cranelift_codegen::ir::types::{F64, I32, I64, I8};
use cranelift_codegen::ir::{
    AbiParam, InstBuilder, MemFlags, Signature, StackSlotData, StackSlotKind, Value as CValue,
};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Switch, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, DataDescription, FuncId, Linkage, Module};

use witchcraft::ast::{BinOp, UnOp};
use witchcraft::ir::{self, Instr, Operand, Terminator};

const TAG_SPARK: i64 = witchcraft_runtime::value::TAG_SPARK as i64;
const TAG_BOOL: i64 = witchcraft_runtime::value::TAG_BOOL as i64;

/// The runtime functions the backend calls, as resolved `FuncId`s in a module.
#[derive(Clone, Copy)]
struct Runtime {
    glyph: FuncId,
    render: FuncId,
    concat2: FuncId,
    speak: FuncId,
    listen: FuncId,
    retain: FuncId,
    release: FuncId,
    equals: FuncId,
    field: FuncId,
    variant_field: FuncId,
    variant_tag: FuncId,
    provenance_glyph: FuncId,
    divine: FuncId,
    make_inferred: FuncId,
    builder_new: FuncId,
    builder_push: FuncId,
    record_finish: FuncId,
    variant_finish: FuncId,
    list_finish: FuncId,
    embed: FuncId,
    similarity: FuncId,
    nearest: FuncId,
    mem_register: FuncId,
    mem_write: FuncId,
    mem_recent: FuncId,
    advance: FuncId,
    audit_log: FuncId,
    parse_seed: FuncId,
    set_seed: FuncId,
    setup_manifest: FuncId,
}

/// Knobs for a compiled run, mirroring the interpreter's `RunConfig`.
#[derive(Clone, Default)]
pub struct RunOptions {
    pub seed: u64,
    /// Fault-injection: force every discharge to see this confidence.
    pub force_confidence: Option<f64>,
    /// A deployment manifest (TOML subset). When present, every inference need is
    /// resolved against it at load (refuse-to-start), and compiled `divine`
    /// routes through the bound engine — the *compiled engine-swap*. Requires the
    /// `engines` feature (the JIT path); the shipped `grimoire` staticlib is
    /// Mock-only. With no manifest the built-in deterministic Mock serves every
    /// need (byte-identical to the interpreter's offline default).
    pub manifest: Option<String>,
}

impl RunOptions {
    pub fn seed(seed: u64) -> Self {
        RunOptions {
            seed,
            force_confidence: None,
            manifest: None,
        }
    }

    /// Set the deployment manifest for this run.
    pub fn with_manifest(mut self, manifest: impl Into<String>) -> Self {
        self.manifest = Some(manifest.into());
        self
    }
}

/// JIT-compile `prog` and run its `main` in-process under `seed`, printing to
/// stdout. The runtime value model, refcounting, decoder, and print sink link in.
pub fn run(prog: &ir::Program, seed: u64) -> Result<(), String> {
    let (mut module, main_id) = jit_compile(prog)?;
    module
        .finalize_definitions()
        .map_err(|e| format!("finalize: {e}"))?;
    let code = module.get_finalized_function(main_id);
    apply_options(&RunOptions::seed(seed));
    bind_manifest(prog, &RunOptions::seed(seed))?;
    unsafe { call_main(code) };
    // Keep the module alive until execution has finished.
    drop(module);
    Ok(())
}

/// Like [`run`], but captures `print` output and returns it (for the equivalence
/// harness — compiled output compared against the interpreter).
pub fn run_capture(prog: &ir::Program, seed: u64) -> Result<String, String> {
    run_capture_with(prog, RunOptions::seed(seed))
}

/// [`run_capture`] with explicit options (seed + fault injection).
pub fn run_capture_with(prog: &ir::Program, options: RunOptions) -> Result<String, String> {
    let (mut module, main_id) = jit_compile(prog)?;
    module
        .finalize_definitions()
        .map_err(|e| format!("finalize: {e}"))?;
    let code = module.get_finalized_function(main_id);
    apply_options(&options);
    // Refuse to start if a need cannot be bound under its policy (mirrors the
    // interpreter's load-time resolution). Must run after the seed is set so
    // deterministic engines (the Mock) are seeded identically.
    bind_manifest(prog, &options)?;
    witchcraft_runtime::begin_capture();
    unsafe { call_main(code) };
    let out = witchcraft_runtime::end_capture();
    drop(module);
    Ok(out)
}

fn apply_options(options: &RunOptions) {
    witchcraft_runtime::set_seed(options.seed);
    witchcraft_runtime::set_force_confidence(options.force_confidence);
}

/// Install the run's manifest into the runtime engine bridge and resolve every
/// inference need against it (refuse-to-start). With no manifest, any previously
/// installed manifest is cleared so the built-in Mock decoder serves each need.
#[cfg(feature = "engines")]
fn bind_manifest(prog: &ir::Program, options: &RunOptions) -> Result<(), String> {
    match &options.manifest {
        Some(src) => {
            witchcraft_runtime::engines::set_manifest(src)?;
            let needs: Vec<(String, bool, bool)> = prog
                .needs
                .iter()
                .map(|n| (n.intent.clone(), n.allow_network, n.allow_downgrade))
                .collect();
            witchcraft_runtime::engines::resolve_needs(&needs)
        }
        None => {
            witchcraft_runtime::engines::clear();
            Ok(())
        }
    }
}

#[cfg(not(feature = "engines"))]
fn bind_manifest(_prog: &ir::Program, options: &RunOptions) -> Result<(), String> {
    if options.manifest.is_some() {
        return Err(
            "a manifest was supplied but this build lacks the `engines` feature".to_string(),
        );
    }
    Ok(())
}

/// # Safety
/// `code` must be the finalized `witch_main` of a module kept alive for the call.
unsafe fn call_main(code: *const u8) {
    let main_fn =
        std::mem::transmute::<*const u8, extern "C" fn() -> witchcraft_runtime::Value>(code);
    let result = main_fn();
    witchcraft_runtime::release(result);
}

fn jit_compile(prog: &ir::Program) -> Result<(JITModule, FuncId), String> {
    let mut flags = settings::builder();
    flags
        .set("use_colocated_libcalls", "false")
        .map_err(|e| e.to_string())?;
    // JIT code is loaded at a known absolute address; no PIC needed.
    flags.set("is_pic", "false").map_err(|e| e.to_string())?;
    let isa_builder = cranelift_native::builder().map_err(|e| e.to_string())?;
    let isa = isa_builder
        .finish(settings::Flags::new(flags))
        .map_err(|e| e.to_string())?;

    let mut builder = JITBuilder::with_isa(isa, default_libcall_names());
    register_runtime_symbols(&mut builder);
    let mut module = JITModule::new(builder);
    let main_id = build(&mut module, prog, false)?;
    Ok((module, main_id))
}

/// Compile `prog` to a relocatable native **object file** with a C `main` entry
/// point, ready to be linked (with `witchcraft-runtime`) into a self-contained
/// executable. This is the ship path behind `grimoire build`.
pub fn compile_object(prog: &ir::Program) -> Result<Vec<u8>, String> {
    let mut flags = settings::builder();
    flags
        .set("use_colocated_libcalls", "false")
        .map_err(|e| e.to_string())?;
    // Position-independent code: required for PIE executables on the supported
    // targets (notably macOS arm64).
    flags.set("is_pic", "true").map_err(|e| e.to_string())?;
    let isa_builder = cranelift_native::builder().map_err(|e| e.to_string())?;
    let isa = isa_builder
        .finish(settings::Flags::new(flags))
        .map_err(|e| e.to_string())?;

    let builder = cranelift_object::ObjectBuilder::new(
        isa,
        "witchcraft",
        cranelift_module::default_libcall_names(),
    )
    .map_err(de)?;
    let mut module = cranelift_object::ObjectModule::new(builder);
    build(&mut module, prog, true)?;
    let mut product = module.finish();
    set_macho_platform(&mut product.object);
    product.emit().map_err(de)
}

/// macOS `ld` rejects a Mach-O object with no platform (`unknown platform`).
/// cranelift-object does not emit a build-version load command, so we add one.
#[cfg(target_os = "macos")]
fn set_macho_platform(object: &mut cranelift_object::object::write::Object) {
    use cranelift_object::object::{macho, write::MachOBuildVersion};
    let mut bv = MachOBuildVersion::default();
    bv.platform = macho::PLATFORM_MACOS;
    bv.minos = 11 << 16; // 11.0.0, packed as X.Y.Z
    bv.sdk = 11 << 16;
    object.set_macho_build_version(bv);
}

#[cfg(not(target_os = "macos"))]
fn set_macho_platform(_object: &mut cranelift_object::object::write::Object) {}

fn register_runtime_symbols(builder: &mut JITBuilder) {
    use witchcraft_runtime::abi::*;
    builder.symbol("w_glyph", w_glyph as *const u8);
    builder.symbol("w_render", w_render as *const u8);
    builder.symbol("w_concat2", w_concat2 as *const u8);
    builder.symbol("w_speak", w_speak as *const u8);
    builder.symbol("w_listen", w_listen as *const u8);
    builder.symbol("w_retain", w_retain as *const u8);
    builder.symbol("w_release", w_release as *const u8);
    builder.symbol("w_equals", w_equals as *const u8);
    builder.symbol("w_field", w_field as *const u8);
    builder.symbol("w_variant_field", w_variant_field as *const u8);
    builder.symbol("w_variant_tag", w_variant_tag as *const u8);
    builder.symbol("w_provenance_glyph", w_provenance_glyph as *const u8);
    builder.symbol("w_divine", w_divine as *const u8);
    builder.symbol("w_make_inferred", w_make_inferred as *const u8);
    builder.symbol("w_builder_new", w_builder_new as *const u8);
    builder.symbol("w_builder_push", w_builder_push as *const u8);
    builder.symbol("w_record_finish", w_record_finish as *const u8);
    builder.symbol("w_variant_finish", w_variant_finish as *const u8);
    builder.symbol("w_list_finish", w_list_finish as *const u8);
    builder.symbol("w_embed", w_embed as *const u8);
    builder.symbol("w_similarity", w_similarity as *const u8);
    builder.symbol("w_nearest", w_nearest as *const u8);
    builder.symbol("w_mem_register", w_mem_register as *const u8);
    builder.symbol("w_mem_write", w_mem_write as *const u8);
    builder.symbol("w_mem_recent", w_mem_recent as *const u8);
    builder.symbol("w_advance", w_advance as *const u8);
    builder.symbol("w_audit_log", w_audit_log as *const u8);
    builder.symbol("w_parse_seed", w_parse_seed as *const u8);
    builder.symbol("w_set_seed", w_set_seed as *const u8);
    builder.symbol("w_setup_manifest", w_setup_manifest as *const u8);
}

/// Declare the runtime functions and every Witchcraft function, then generate
/// each body. Returns the `FuncId` of `witch_main`. When `emit_entry` is set, a
/// C `main` entry point is also emitted (for object/executable builds). Generic
/// over the module kind (JIT or object).
fn build<M: Module>(
    module: &mut M,
    prog: &ir::Program,
    emit_entry: bool,
) -> Result<FuncId, String> {
    let rt = declare_runtime(module)?;
    let grammar_bytes = encode_grammars(prog);

    let mut fn_ids: HashMap<String, FuncId> = HashMap::new();
    for f in &prog.functions {
        let id = declare_function(module, &f.name, f.params.len())?;
        fn_ids.insert(f.name.clone(), id);
    }
    let main_id = declare_function(module, "witch_main", 0)?;

    let mut fbctx = FunctionBuilderContext::new();
    for f in &prog.functions {
        let id = fn_ids[&f.name];
        gen_function(module, &mut fbctx, &rt, &fn_ids, &grammar_bytes, f, id)?;
    }
    gen_function(
        module,
        &mut fbctx,
        &rt,
        &fn_ids,
        &grammar_bytes,
        &prog.main,
        main_id,
    )?;

    if emit_entry {
        gen_entry(module, &mut fbctx, &rt, prog, main_id)?;
    }

    Ok(main_id)
}

/// Serialise each `divine` site's compiled grammar to bytes for embedding in the
/// artifact (data section). Variant tags use the program's interned ids so a
/// decoded variant dispatches correctly through a compiled `enact`. The runtime
/// reconstructs the grammar from these bytes at the call site (so a built
/// executable carries the type as a generation constraint — the litmus property).
fn encode_grammars(prog: &ir::Program) -> Vec<Vec<u8>> {
    let name_to_tag: HashMap<&str, u32> = prog
        .variant_names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i as u32))
        .collect();
    prog.grammars
        .iter()
        .map(|g| witchcraft_runtime::encode(&convert_grammar(g, &name_to_tag)))
        .collect()
}

/// Emit the C `main(argc, argv)` entry point: parse `--seed`, seed the runtime,
/// install the deployment manifest and resolve the program's inference needs
/// (`--manifest`, refuse-to-start), run `witch_main`, release its result, and
/// return 0. The needs are embedded in the artifact so the standalone binary
/// resolves them exactly as the JIT path does — the compiled engine-swap.
fn gen_entry<M: Module>(
    module: &mut M,
    fbctx: &mut FunctionBuilderContext,
    rt: &Runtime,
    prog: &ir::Program,
    main_id: FuncId,
) -> Result<(), String> {
    let ptr_ty = module.target_config().pointer_type();
    let call = module.target_config().default_call_conv;
    let mut sig = Signature::new(call);
    sig.params.push(AbiParam::new(I32)); // argc
    sig.params.push(AbiParam::new(ptr_ty)); // argv
    sig.returns.push(AbiParam::new(I32)); // exit code
    let entry_id = module
        .declare_function("main", Linkage::Export, &sig)
        .map_err(de)?;

    // Serialise the program's needs (intent + policy) into a read-only data
    // object the entry passes to `w_setup_manifest`.
    let needs: Vec<(String, bool, bool)> = prog
        .needs
        .iter()
        .map(|n| (n.intent.clone(), n.allow_network, n.allow_downgrade))
        .collect();
    let needs_bytes = witchcraft_runtime::encode_needs(&needs);
    let needs_data = module
        .declare_data(
            &format!("witch_needs_{}", next_data_id()),
            Linkage::Local,
            false,
            false,
        )
        .map_err(de)?;
    let mut needs_desc = DataDescription::new();
    needs_desc.define(needs_bytes.clone().into_boxed_slice());
    module.define_data(needs_data, &needs_desc).map_err(de)?;

    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    {
        let mut b = FunctionBuilder::new(&mut ctx.func, fbctx);
        let parse_seed = module.declare_func_in_func(rt.parse_seed, b.func);
        let set_seed = module.declare_func_in_func(rt.set_seed, b.func);
        let setup_manifest = module.declare_func_in_func(rt.setup_manifest, b.func);
        let release = module.declare_func_in_func(rt.release, b.func);
        let witch_main = module.declare_func_in_func(main_id, b.func);
        let needs_gv = module.declare_data_in_func(needs_data, b.func);

        let blk = b.create_block();
        b.append_block_params_for_function_params(blk);
        b.switch_to_block(blk);
        b.seal_block(blk);
        let argc = b.block_params(blk)[0];
        let argv = b.block_params(blk)[1];

        let seed_call = b.ins().call(parse_seed, &[argc, argv]);
        let seed = b.inst_results(seed_call)[0];
        b.ins().call(set_seed, &[seed]);

        // Resolve needs against the manifest (refuse-to-start) before running.
        let needs_ptr = b.ins().global_value(ptr_ty, needs_gv);
        let needs_ptr = if ptr_ty != I64 {
            b.ins().uextend(I64, needs_ptr)
        } else {
            needs_ptr
        };
        let needs_len = b.ins().iconst(I64, needs_bytes.len() as i64);
        b.ins()
            .call(setup_manifest, &[argc, argv, needs_ptr, needs_len]);

        let main_call = b.ins().call(witch_main, &[]);
        let res = b.inst_results(main_call);
        let (rtag, rbits) = (res[0], res[1]);
        b.ins().call(release, &[rtag, rbits]);

        let zero = b.ins().iconst(I32, 0);
        b.ins().return_(&[zero]);
        b.finalize();
    }
    module.define_function(entry_id, &mut ctx).map_err(de)?;
    module.clear_context(&mut ctx);
    Ok(())
}

fn convert_grammar(
    g: &witchcraft::grammar::Grammar,
    name_to_tag: &HashMap<&str, u32>,
) -> witchcraft_runtime::Grammar {
    use witchcraft::grammar::Grammar as Fe;
    use witchcraft_runtime::{Grammar as Rt, GrammarVariant};
    match g {
        Fe::Number { lo, hi } => Rt::Number { lo: *lo, hi: *hi },
        Fe::Bool => Rt::Bool,
        Fe::Text { max_len } => Rt::Text { max_len: *max_len },
        Fe::Record(fields) => Rt::Record(
            fields
                .iter()
                .map(|(n, sub)| (n.clone(), convert_grammar(sub, name_to_tag)))
                .collect(),
        ),
        Fe::OneOf(variants) => Rt::OneOf(
            variants
                .iter()
                .map(|v| GrammarVariant {
                    name: v.name.clone(),
                    tag: name_to_tag
                        .get(v.name.as_str())
                        .copied()
                        .unwrap_or(u32::MAX),
                    fields: v
                        .fields
                        .iter()
                        .map(|(n, sub)| (n.clone(), convert_grammar(sub, name_to_tag)))
                        .collect(),
                })
                .collect(),
        ),
    }
}

fn declare_runtime<M: Module>(module: &mut M) -> Result<Runtime, String> {
    let call = module.target_config().default_call_conv;
    let i64p = || AbiParam::new(I64);

    let mut import = |name: &str, params: Vec<AbiParam>, returns: Vec<AbiParam>| {
        let sig = Signature {
            params,
            returns,
            call_conv: call,
        };
        module
            .declare_function(name, Linkage::Import, &sig)
            .map_err(de)
    };

    let value = || vec![i64p(), i64p()];
    let glyph = import("w_glyph", vec![i64p(), i64p()], value())?;
    let render = import("w_render", value(), value())?;
    let concat2 = import("w_concat2", vec![i64p(), i64p(), i64p(), i64p()], value())?;
    let speak = import("w_speak", value(), vec![])?;
    let listen = import("w_listen", value(), value())?;
    let retain = import("w_retain", value(), vec![])?;
    let release = import("w_release", value(), vec![])?;
    let equals = import(
        "w_equals",
        vec![i64p(), i64p(), i64p(), i64p()],
        vec![AbiParam::new(I8)],
    )?;
    // value + (name ptr, len) -> value
    let field = import("w_field", vec![i64p(), i64p(), i64p(), i64p()], value())?;
    // value + index -> value
    let variant_field = import("w_variant_field", vec![i64p(), i64p(), i64p()], value())?;
    // value -> u32
    let variant_tag = import("w_variant_tag", value(), vec![AbiParam::new(I32)])?;
    let provenance_glyph = import("w_provenance_glyph", value(), value())?;
    // (grammar ptr,len), (intent ptr,len), input value, conf_out ptr -> value
    let divine = import(
        "w_divine",
        vec![i64p(), i64p(), i64p(), i64p(), i64p(), i64p(), i64p()],
        value(),
    )?;
    // inner value, confidence (f64) -> value (provenance from the last decode)
    let make_inferred = import(
        "w_make_inferred",
        vec![i64p(), i64p(), AbiParam::new(F64)],
        value(),
    )?;
    let builder_new = import("w_builder_new", vec![], vec![i64p()])?;
    // builder, (name ptr,len), value
    let builder_push = import(
        "w_builder_push",
        vec![i64p(), i64p(), i64p(), i64p(), i64p()],
        vec![],
    )?;
    let record_finish = import("w_record_finish", vec![i64p()], value())?;
    // builder, (name ptr,len), tag(u32) -> value
    let variant_finish = import(
        "w_variant_finish",
        vec![i64p(), i64p(), i64p(), AbiParam::new(I32)],
        value(),
    )?;
    // builder -> value (list)
    let list_finish = import("w_list_finish", vec![i64p()], value())?;
    // (oracle ptr,len), (space ptr,len), input value -> value (embedding)
    let embed = import(
        "w_embed",
        vec![i64p(), i64p(), i64p(), i64p(), i64p(), i64p()],
        value(),
    )?;
    // a value, b value -> value (spark)
    let similarity = import(
        "w_similarity",
        vec![i64p(), i64p(), i64p(), i64p()],
        value(),
    )?;
    // query value, candidates value, k value -> value (list)
    let nearest = import(
        "w_nearest",
        vec![i64p(), i64p(), i64p(), i64p(), i64p(), i64p()],
        value(),
    )?;
    // (name ptr,len), (scope ptr,len), has_retention(i8), retention(f64), audit(i8)
    let mem_register = import(
        "w_mem_register",
        vec![
            i64p(),
            i64p(),
            i64p(),
            i64p(),
            AbiParam::new(I8),
            AbiParam::new(F64),
            AbiParam::new(I8),
        ],
        vec![],
    )?;
    // (name ptr,len), value
    let mem_write = import("w_mem_write", vec![i64p(), i64p(), i64p(), i64p()], vec![])?;
    // (name ptr,len), (method ptr,len), k value -> value (list)
    let mem_recent = import(
        "w_mem_recent",
        vec![i64p(), i64p(), i64p(), i64p(), i64p(), i64p()],
        value(),
    )?;
    // n value
    let advance = import("w_advance", value(), vec![])?;
    // () -> value (list)
    let audit_log = import("w_audit_log", vec![], value())?;
    // (argc, argv) -> seed; used only by the compiled executable entry point.
    let parse_seed = import(
        "w_parse_seed",
        vec![AbiParam::new(I32), i64p()],
        vec![i64p()],
    )?;
    let set_seed = import("w_set_seed", vec![i64p()], vec![])?;
    // (argc, argv, needs_ptr, needs_len); installs the manifest + resolves needs
    // (refuse-to-start). Used only by the compiled executable entry point.
    let setup_manifest = import(
        "w_setup_manifest",
        vec![AbiParam::new(I32), i64p(), i64p(), i64p()],
        vec![],
    )?;

    Ok(Runtime {
        glyph,
        render,
        concat2,
        speak,
        listen,
        retain,
        release,
        equals,
        field,
        variant_field,
        variant_tag,
        provenance_glyph,
        divine,
        make_inferred,
        builder_new,
        builder_push,
        record_finish,
        variant_finish,
        list_finish,
        embed,
        similarity,
        nearest,
        mem_register,
        mem_write,
        mem_recent,
        advance,
        audit_log,
        parse_seed,
        set_seed,
        setup_manifest,
    })
}

fn declare_function<M: Module>(
    module: &mut M,
    name: &str,
    n_params: usize,
) -> Result<FuncId, String> {
    let sig = user_signature(module, n_params);
    module
        .declare_function(name, Linkage::Local, &sig)
        .map_err(de)
}

/// A Witchcraft function: each `Value` parameter is two `i64`s; it returns one
/// `Value` (two `i64`s).
fn user_signature<M: Module>(module: &M, n_params: usize) -> Signature {
    let mut sig = Signature::new(module.target_config().default_call_conv);
    for _ in 0..n_params {
        sig.params.push(AbiParam::new(I64));
        sig.params.push(AbiParam::new(I64));
    }
    sig.returns.push(AbiParam::new(I64));
    sig.returns.push(AbiParam::new(I64));
    sig
}

#[allow(clippy::too_many_arguments)]
fn gen_function<M: Module>(
    module: &mut M,
    fbctx: &mut FunctionBuilderContext,
    rt: &Runtime,
    fn_ids: &HashMap<String, FuncId>,
    grammar_bytes: &[Vec<u8>],
    func: &ir::Function,
    func_id: FuncId,
) -> Result<(), String> {
    let mut ctx = module.make_context();
    ctx.func.signature = user_signature(module, func.params.len());

    {
        let mut b = FunctionBuilder::new(&mut ctx.func, fbctx);

        let cl_blocks: Vec<_> = func.blocks.iter().map(|_| b.create_block()).collect();
        let entry = cl_blocks[func.entry as usize];
        b.append_block_params_for_function_params(entry);

        // One pair of variables (tag, bits) per local slot.
        let locals: Vec<(Variable, Variable)> = (0..func.num_locals)
            .map(|_| (b.declare_var(I64), b.declare_var(I64)))
            .collect();

        // Resolve the func refs this body may call (runtime + user functions).
        let refs = Refs {
            glyph: module.declare_func_in_func(rt.glyph, b.func),
            render: module.declare_func_in_func(rt.render, b.func),
            concat2: module.declare_func_in_func(rt.concat2, b.func),
            speak: module.declare_func_in_func(rt.speak, b.func),
            listen: module.declare_func_in_func(rt.listen, b.func),
            retain: module.declare_func_in_func(rt.retain, b.func),
            release: module.declare_func_in_func(rt.release, b.func),
            equals: module.declare_func_in_func(rt.equals, b.func),
            field: module.declare_func_in_func(rt.field, b.func),
            variant_field: module.declare_func_in_func(rt.variant_field, b.func),
            variant_tag: module.declare_func_in_func(rt.variant_tag, b.func),
            provenance_glyph: module.declare_func_in_func(rt.provenance_glyph, b.func),
            divine: module.declare_func_in_func(rt.divine, b.func),
            make_inferred: module.declare_func_in_func(rt.make_inferred, b.func),
            builder_new: module.declare_func_in_func(rt.builder_new, b.func),
            builder_push: module.declare_func_in_func(rt.builder_push, b.func),
            record_finish: module.declare_func_in_func(rt.record_finish, b.func),
            variant_finish: module.declare_func_in_func(rt.variant_finish, b.func),
            list_finish: module.declare_func_in_func(rt.list_finish, b.func),
            embed: module.declare_func_in_func(rt.embed, b.func),
            similarity: module.declare_func_in_func(rt.similarity, b.func),
            nearest: module.declare_func_in_func(rt.nearest, b.func),
            mem_register: module.declare_func_in_func(rt.mem_register, b.func),
            mem_write: module.declare_func_in_func(rt.mem_write, b.func),
            mem_recent: module.declare_func_in_func(rt.mem_recent, b.func),
            advance: module.declare_func_in_func(rt.advance, b.func),
            audit_log: module.declare_func_in_func(rt.audit_log, b.func),
        };
        let mut user_refs: HashMap<String, _> = HashMap::new();
        for (name, id) in fn_ids {
            user_refs.insert(name.clone(), module.declare_func_in_func(*id, b.func));
        }

        {
            let mut g = Gen {
                module,
                b: &mut b,
                refs,
                user_refs,
                grammar_bytes,
                locals,
                tmps: HashMap::new(),
            };

            for (bi, blk) in func.blocks.iter().enumerate() {
                g.b.switch_to_block(cl_blocks[bi]);
                if bi as u32 == func.entry {
                    g.init_entry(func, entry);
                }
                for instr in &blk.instrs {
                    g.instr(instr)?;
                }
                g.terminator(&blk.term, &cl_blocks)?;
            }

            g.b.seal_all_blocks();
        }
        b.finalize();
    }

    module.define_function(func_id, &mut ctx).map_err(de)?;
    module.clear_context(&mut ctx);
    Ok(())
}

fn de<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Func refs valid within one function body.
struct Refs {
    glyph: cranelift_codegen::ir::FuncRef,
    render: cranelift_codegen::ir::FuncRef,
    concat2: cranelift_codegen::ir::FuncRef,
    speak: cranelift_codegen::ir::FuncRef,
    listen: cranelift_codegen::ir::FuncRef,
    retain: cranelift_codegen::ir::FuncRef,
    release: cranelift_codegen::ir::FuncRef,
    equals: cranelift_codegen::ir::FuncRef,
    field: cranelift_codegen::ir::FuncRef,
    variant_field: cranelift_codegen::ir::FuncRef,
    variant_tag: cranelift_codegen::ir::FuncRef,
    provenance_glyph: cranelift_codegen::ir::FuncRef,
    divine: cranelift_codegen::ir::FuncRef,
    make_inferred: cranelift_codegen::ir::FuncRef,
    builder_new: cranelift_codegen::ir::FuncRef,
    builder_push: cranelift_codegen::ir::FuncRef,
    record_finish: cranelift_codegen::ir::FuncRef,
    variant_finish: cranelift_codegen::ir::FuncRef,
    list_finish: cranelift_codegen::ir::FuncRef,
    embed: cranelift_codegen::ir::FuncRef,
    similarity: cranelift_codegen::ir::FuncRef,
    nearest: cranelift_codegen::ir::FuncRef,
    mem_register: cranelift_codegen::ir::FuncRef,
    mem_write: cranelift_codegen::ir::FuncRef,
    mem_recent: cranelift_codegen::ir::FuncRef,
    advance: cranelift_codegen::ir::FuncRef,
    audit_log: cranelift_codegen::ir::FuncRef,
}

struct Gen<'a, 'b, M: Module> {
    module: &'a mut M,
    b: &'a mut FunctionBuilder<'b>,
    refs: Refs,
    user_refs: HashMap<String, cranelift_codegen::ir::FuncRef>,
    /// `GrammarId` -> serialised grammar bytes embedded for `divine`.
    grammar_bytes: &'a [Vec<u8>],
    /// Local slot -> its `(tag, bits)` Cranelift variables.
    locals: Vec<(Variable, Variable)>,
    /// IR temporary -> its `(tag, bits)` Cranelift values.
    tmps: HashMap<u32, (CValue, CValue)>,
}

impl<M: Module> Gen<'_, '_, M> {
    fn init_entry(&mut self, func: &ir::Function, entry: cranelift_codegen::ir::Block) {
        let zero = self.b.ins().iconst(I64, 0);
        for &(tag, bits) in &self.locals {
            self.b.def_var(tag, zero);
            self.b.def_var(bits, zero);
        }
        let params: Vec<CValue> = self.b.block_params(entry).to_vec();
        for (pi, &local) in func.params.iter().enumerate() {
            let (tag, bits) = self.locals[local as usize];
            self.b.def_var(tag, params[pi * 2]);
            self.b.def_var(bits, params[pi * 2 + 1]);
        }
    }

    /// Resolve an operand to `(tag, bits)`. Heap temporaries are looked up;
    /// scalars are materialised inline (no allocation).
    fn operand(&mut self, op: &Operand) -> (CValue, CValue) {
        match op {
            Operand::Tmp(t) => self.tmps[t],
            Operand::Spark(n) => {
                let tag = self.b.ins().iconst(I64, TAG_SPARK);
                let f = self.b.ins().f64const(*n);
                let bits = self.b.ins().bitcast(I64, MemFlags::new(), f);
                (tag, bits)
            }
            Operand::Bool(v) => {
                let tag = self.b.ins().iconst(I64, TAG_BOOL);
                let bits = self.b.ins().iconst(I64, *v as i64);
                (tag, bits)
            }
            Operand::Unit => {
                let z = self.b.ins().iconst(I64, 0);
                (z, z)
            }
        }
    }

    /// Release the value behind an operand if it is a temporary (the runtime
    /// borrows operands; the codegen owns temporaries). No-op for scalars.
    fn release_operand(&mut self, op: &Operand) {
        if let Operand::Tmp(_) = op {
            let (tag, bits) = self.operand(op);
            self.b.ins().call(self.refs.release, &[tag, bits]);
        }
    }

    fn call_value(
        &mut self,
        f: cranelift_codegen::ir::FuncRef,
        args: &[CValue],
    ) -> (CValue, CValue) {
        let call = self.b.ins().call(f, args);
        let res = self.b.inst_results(call);
        (res[0], res[1])
    }

    fn spark_from_f64(&mut self, f: CValue) -> (CValue, CValue) {
        let tag = self.b.ins().iconst(I64, TAG_SPARK);
        let bits = self.b.ins().bitcast(I64, MemFlags::new(), f);
        (tag, bits)
    }

    fn bool_value(&mut self, cond_i8: CValue) -> (CValue, CValue) {
        let tag = self.b.ins().iconst(I64, TAG_BOOL);
        let bits = self.b.ins().uextend(I64, cond_i8);
        (tag, bits)
    }

    fn f64_of(&mut self, op: &Operand) -> CValue {
        let (_, bits) = self.operand(op);
        self.b
            .ins()
            .bitcast(cranelift_codegen::ir::types::F64, MemFlags::new(), bits)
    }

    fn instr(&mut self, instr: &Instr) -> Result<(), String> {
        match instr {
            Instr::LoadLocal { dst, local } => {
                let (vtag, vbits) = self.locals[*local as usize];
                let tag = self.b.use_var(vtag);
                let bits = self.b.use_var(vbits);
                // The temporary becomes an independent owner of the value.
                self.b.ins().call(self.refs.retain, &[tag, bits]);
                self.tmps.insert(*dst, (tag, bits));
            }
            Instr::StoreLocal { local, src } => {
                // The local releases its previous value and takes ownership of src.
                let (vtag, vbits) = self.locals[*local as usize];
                let old_tag = self.b.use_var(vtag);
                let old_bits = self.b.use_var(vbits);
                self.b.ins().call(self.refs.release, &[old_tag, old_bits]);
                let (tag, bits) = self.operand(src);
                self.b.def_var(vtag, tag);
                self.b.def_var(vbits, bits);
            }
            Instr::Glyph { dst, text } => {
                let (ptr, len) = self.str_data(text)?;
                let v = self.call_value(self.refs.glyph, &[ptr, len]);
                self.tmps.insert(*dst, v);
            }
            Instr::Render { dst, val } => {
                let (tag, bits) = self.operand(val);
                let v = self.call_value(self.refs.render, &[tag, bits]);
                self.release_operand(val);
                self.tmps.insert(*dst, v);
            }
            Instr::Concat { dst, parts } => {
                let v = self.concat(parts);
                self.tmps.insert(*dst, v);
            }
            Instr::Bin { dst, op, lhs, rhs } => {
                let v = self.binop(*op, lhs, rhs)?;
                self.release_operand(lhs);
                self.release_operand(rhs);
                self.tmps.insert(*dst, v);
            }
            Instr::Un { dst, op, val } => {
                let v = self.unop(*op, val)?;
                self.release_operand(val);
                self.tmps.insert(*dst, v);
            }
            Instr::Call { dst, callee, args } => {
                let fref = *self
                    .user_refs
                    .get(callee)
                    .ok_or_else(|| format!("call to unknown function `{callee}`"))?;
                // Arguments transfer ownership to the callee; do not release them.
                let mut flat = Vec::with_capacity(args.len() * 2);
                for a in args {
                    let (tag, bits) = self.operand(a);
                    flat.push(tag);
                    flat.push(bits);
                }
                let v = self.call_value(fref, &flat);
                self.tmps.insert(*dst, v);
            }
            Instr::Speak { val } => {
                let (tag, bits) = self.operand(val);
                self.b.ins().call(self.refs.speak, &[tag, bits]);
                self.release_operand(val);
            }
            Instr::Listen { dst, prompt } => {
                let (ptag, pbits) = self.operand(prompt);
                let v = self.call_value(self.refs.listen, &[ptag, pbits]);
                self.release_operand(prompt);
                self.tmps.insert(*dst, v);
            }
            Instr::Field { dst, recv, field } => {
                let (rtag, rbits) = self.operand(recv);
                let (nptr, nlen) = self.str_data(field)?;
                let v = self.call_value(self.refs.field, &[rtag, rbits, nptr, nlen]);
                // The receiver temporary is consumed by this read.
                self.release_operand(recv);
                self.tmps.insert(*dst, v);
            }
            Instr::VariantField { dst, recv, index } => {
                let (rtag, rbits) = self.operand(recv);
                let idx = self.b.ins().iconst(I64, *index as i64);
                let v = self.call_value(self.refs.variant_field, &[rtag, rbits, idx]);
                // The receiver is reused across arms; it is released by enact's owner.
                self.tmps.insert(*dst, v);
            }
            Instr::VariantTag { dst, recv } => {
                let (rtag, rbits) = self.operand(recv);
                let call = self.b.ins().call(self.refs.variant_tag, &[rtag, rbits]);
                let tag32 = self.b.inst_results(call)[0];
                let tag64 = self.b.ins().uextend(I64, tag32);
                // Only the integer (bits) is read by the Switch; the tag slot is unused.
                let zero = self.b.ins().iconst(I64, 0);
                self.tmps.insert(*dst, (zero, tag64));
            }
            Instr::ProvenanceGlyph { dst, recv } => {
                let (rtag, rbits) = self.operand(recv);
                let v = self.call_value(self.refs.provenance_glyph, &[rtag, rbits]);
                self.tmps.insert(*dst, v);
            }
            Instr::Decode {
                dst_val,
                dst_conf,
                grammar,
                intent,
                input,
            } => {
                // The input glyph (the prompt) is the engine's `from (...)`; the
                // runtime consumes it (the Mock ignores it; a real engine reads it).
                let (itag, ibits) = self.operand(input);
                let bytes = self.grammar_bytes[*grammar as usize].clone();
                let (gptr, glen) = self.bytes_data(&bytes)?;
                let (iptr, ilen) = self.str_data(intent)?;
                // Stack slot to receive the confidence (an out-parameter).
                let slot = self.b.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    8,
                    3,
                ));
                let conf_ptr = self.b.ins().stack_addr(I64, slot, 0);
                let val = self.call_value(
                    self.refs.divine,
                    &[gptr, glen, iptr, ilen, itag, ibits, conf_ptr],
                );
                let conf_f = self.b.ins().stack_load(F64, slot, 0);
                let conf_val = self.spark_from_f64(conf_f);
                self.tmps.insert(*dst_val, val);
                self.tmps.insert(*dst_conf, conf_val);
            }
            Instr::MakeInferred { dst, val, conf } => {
                let (vtag, vbits) = self.operand(val);
                let conf_f = self.f64_of(conf);
                let call = self
                    .b
                    .ins()
                    .call(self.refs.make_inferred, &[vtag, vbits, conf_f]);
                let res = self.b.inst_results(call);
                self.tmps.insert(*dst, (res[0], res[1]));
            }
            Instr::MakeRecord { dst, fields } => {
                let v = self.build_aggregate(fields, None)?;
                self.tmps.insert(*dst, v);
            }
            Instr::MakeVariant {
                dst,
                name,
                tag,
                fields,
            } => {
                let v = self.build_aggregate(fields, Some((name, *tag)))?;
                self.tmps.insert(*dst, v);
            }
            Instr::MakeList { dst, items } => {
                let v = self.build_list(items);
                self.tmps.insert(*dst, v);
            }
            Instr::Embed {
                dst,
                oracle,
                space,
                input,
            } => {
                let (itag, ibits) = self.operand(input);
                let (optr, olen) = self.str_data(oracle)?;
                let (sptr, slen) = self.str_data(space)?;
                let v = self.call_value(self.refs.embed, &[optr, olen, sptr, slen, itag, ibits]);
                self.release_operand(input);
                self.tmps.insert(*dst, v);
            }
            Instr::Similarity { dst, lhs, rhs } => {
                let (ltag, lbits) = self.operand(lhs);
                let (rtag, rbits) = self.operand(rhs);
                let v = self.call_value(self.refs.similarity, &[ltag, lbits, rtag, rbits]);
                self.release_operand(lhs);
                self.release_operand(rhs);
                self.tmps.insert(*dst, v);
            }
            Instr::Nearest {
                dst,
                query,
                candidates,
                k,
            } => {
                let (qtag, qbits) = self.operand(query);
                let (ctag, cbits) = self.operand(candidates);
                let (ktag, kbits) = self.operand(k);
                let v =
                    self.call_value(self.refs.nearest, &[qtag, qbits, ctag, cbits, ktag, kbits]);
                self.release_operand(query);
                self.release_operand(candidates);
                self.release_operand(k);
                self.tmps.insert(*dst, v);
            }
            Instr::MemRegister {
                name,
                scope,
                retention,
                audit,
            } => {
                let (nptr, nlen) = self.str_data(name)?;
                let (sptr, slen) = self.str_data(scope)?;
                let (has_ret, ret) = match retention {
                    Some(r) => (1i64, *r),
                    None => (0i64, 0.0),
                };
                let has_ret = self.b.ins().iconst(I8, has_ret);
                let ret = self.b.ins().f64const(ret);
                let audit = self.b.ins().iconst(I8, *audit as i64);
                self.b.ins().call(
                    self.refs.mem_register,
                    &[nptr, nlen, sptr, slen, has_ret, ret, audit],
                );
            }
            Instr::MemWrite { name, value } => {
                let (nptr, nlen) = self.str_data(name)?;
                let (vtag, vbits) = self.operand(value);
                // The value's reference transfers into the store; do not release.
                self.b
                    .ins()
                    .call(self.refs.mem_write, &[nptr, nlen, vtag, vbits]);
            }
            Instr::MemRecent {
                dst,
                name,
                method,
                k,
            } => {
                let (nptr, nlen) = self.str_data(name)?;
                let (mptr, mlen) = self.str_data(method)?;
                let (ktag, kbits) = self.operand(k);
                let v =
                    self.call_value(self.refs.mem_recent, &[nptr, nlen, mptr, mlen, ktag, kbits]);
                self.release_operand(k);
                self.tmps.insert(*dst, v);
            }
            Instr::Advance { n } => {
                let (ntag, nbits) = self.operand(n);
                self.b.ins().call(self.refs.advance, &[ntag, nbits]);
                self.release_operand(n);
            }
            Instr::AuditLog { dst } => {
                let v = self.call_value(self.refs.audit_log, &[]);
                self.tmps.insert(*dst, v);
            }
        }
        Ok(())
    }

    /// Build a list value via the runtime builder (elements are positional;
    /// names are dropped by `w_list_finish`). Each element transfers in.
    fn build_list(&mut self, items: &[Operand]) -> (CValue, CValue) {
        let call = self.b.ins().call(self.refs.builder_new, &[]);
        let builder = self.b.inst_results(call)[0];
        let (nptr, nlen) = self.str_data("").expect("empty field name");
        for item in items {
            let (vtag, vbits) = self.operand(item);
            self.b
                .ins()
                .call(self.refs.builder_push, &[builder, nptr, nlen, vtag, vbits]);
        }
        let result = self.b.ins().call(self.refs.list_finish, &[builder]);
        let res = self.b.inst_results(result);
        (res[0], res[1])
    }

    /// Build a record (when `variant` is `None`) or variant via the runtime
    /// builder, transferring each field value into the builder.
    fn build_aggregate(
        &mut self,
        fields: &[(String, Operand)],
        variant: Option<(&str, u32)>,
    ) -> Result<(CValue, CValue), String> {
        let call = self.b.ins().call(self.refs.builder_new, &[]);
        let builder = self.b.inst_results(call)[0];
        for (fname, fop) in fields {
            let (vtag, vbits) = self.operand(fop);
            let (nptr, nlen) = self.str_data(fname)?;
            self.b
                .ins()
                .call(self.refs.builder_push, &[builder, nptr, nlen, vtag, vbits]);
        }
        let result = match variant {
            None => self.b.ins().call(self.refs.record_finish, &[builder]),
            Some((name, tag)) => {
                let (nptr, nlen) = self.str_data(name)?;
                let tagc = self.b.ins().iconst(I32, tag as i64);
                self.b
                    .ins()
                    .call(self.refs.variant_finish, &[builder, nptr, nlen, tagc])
            }
        };
        let res = self.b.inst_results(result);
        Ok((res[0], res[1]))
    }

    fn concat(&mut self, parts: &[Operand]) -> (CValue, CValue) {
        if parts.is_empty() {
            // An empty glyph.
            let (ptr, len) = self.str_data("").expect("empty glyph literal");
            return self.call_value(self.refs.glyph, &[ptr, len]);
        }
        let mut acc = self.operand(&parts[0]);
        for p in &parts[1..] {
            let (ptag, pbits) = self.operand(p);
            let next = self.call_value(self.refs.concat2, &[acc.0, acc.1, ptag, pbits]);
            // Both inputs are consumed by concat (their text was copied).
            self.b.ins().call(self.refs.release, &[acc.0, acc.1]);
            self.b.ins().call(self.refs.release, &[ptag, pbits]);
            acc = next;
        }
        acc
    }

    fn binop(
        &mut self,
        op: BinOp,
        lhs: &Operand,
        rhs: &Operand,
    ) -> Result<(CValue, CValue), String> {
        use BinOp::*;
        match op {
            Add | Sub | Mul | Div => {
                let a = self.f64_of(lhs);
                let b = self.f64_of(rhs);
                let r = match op {
                    Add => self.b.ins().fadd(a, b),
                    Sub => self.b.ins().fsub(a, b),
                    Mul => self.b.ins().fmul(a, b),
                    Div => self.b.ins().fdiv(a, b),
                    _ => unreachable!(),
                };
                Ok(self.spark_from_f64(r))
            }
            Lt | Le | Gt | Ge => {
                let a = self.f64_of(lhs);
                let b = self.f64_of(rhs);
                let cc = match op {
                    Lt => FloatCC::LessThan,
                    Le => FloatCC::LessThanOrEqual,
                    Gt => FloatCC::GreaterThan,
                    Ge => FloatCC::GreaterThanOrEqual,
                    _ => unreachable!(),
                };
                let cmp = self.b.ins().fcmp(cc, a, b);
                Ok(self.bool_value(cmp))
            }
            Eq | Ne => {
                let (lt, lb) = self.operand(lhs);
                let (rt, rb) = self.operand(rhs);
                let call = self.b.ins().call(self.refs.equals, &[lt, lb, rt, rb]);
                let eq = self.b.inst_results(call)[0];
                let bit = if matches!(op, Ne) {
                    let one = self.b.ins().iconst(I8, 1);
                    self.b.ins().bxor(eq, one)
                } else {
                    eq
                };
                Ok(self.bool_value(bit))
            }
            And | Or => Err("logical and/or should have been lowered to control flow".into()),
        }
    }

    fn unop(&mut self, op: UnOp, val: &Operand) -> Result<(CValue, CValue), String> {
        match op {
            UnOp::Neg => {
                let f = self.f64_of(val);
                let neg = self.b.ins().fneg(f);
                Ok(self.spark_from_f64(neg))
            }
            UnOp::Not => {
                let (_, bits) = self.operand(val);
                let one = self.b.ins().iconst(I64, 1);
                let flipped = self.b.ins().bxor(bits, one);
                let tag = self.b.ins().iconst(I64, TAG_BOOL);
                Ok((tag, flipped))
            }
        }
    }

    fn terminator(
        &mut self,
        term: &Terminator,
        cl_blocks: &[cranelift_codegen::ir::Block],
    ) -> Result<(), String> {
        match term {
            Terminator::Jump(b) => {
                self.b.ins().jump(cl_blocks[*b as usize], &[]);
            }
            Terminator::Branch {
                cond,
                then_blk,
                else_blk,
            } => {
                let (_, bits) = self.operand(cond);
                self.b.ins().brif(
                    bits,
                    cl_blocks[*then_blk as usize],
                    &[],
                    cl_blocks[*else_blk as usize],
                    &[],
                );
            }
            Terminator::Return(op) => {
                // Compute the return value (ownership transfers to the caller),
                // then release every local before leaving the function.
                let ret = match op {
                    Some(o) => self.operand(o),
                    None => {
                        let z = self.b.ins().iconst(I64, 0);
                        (z, z)
                    }
                };
                for &(vtag, vbits) in &self.locals {
                    let tag = self.b.use_var(vtag);
                    let bits = self.b.use_var(vbits);
                    self.b.ins().call(self.refs.release, &[tag, bits]);
                }
                self.b.ins().return_(&[ret.0, ret.1]);
            }
            Terminator::Switch { tag, arms, default } => {
                let (_, scrutinee) = self.operand(tag);
                let mut switch = Switch::new();
                for (tag_id, blk) in arms {
                    switch.set_entry(*tag_id as u128, cl_blocks[*blk as usize]);
                }
                switch.emit(self.b, scrutinee, cl_blocks[*default as usize]);
            }
            Terminator::Unreachable => {
                // Statically dead (e.g. an exhaustive enact default). Return unit.
                let z = self.b.ins().iconst(I64, 0);
                self.b.ins().return_(&[z, z]);
            }
        }
        Ok(())
    }

    /// Define a read-only data object for a string (glyph literal, field name,
    /// oracle/model id) and return `(pointer, len)` Cranelift values.
    fn str_data(&mut self, text: &str) -> Result<(CValue, CValue), String> {
        self.bytes_data(text.as_bytes())
    }

    /// Define a read-only data object for arbitrary bytes (e.g. a serialised
    /// grammar) and return `(pointer, len)` Cranelift values.
    fn bytes_data(&mut self, bytes: &[u8]) -> Result<(CValue, CValue), String> {
        let name = format!("data_{}", next_data_id());
        let data_id = self
            .module
            .declare_data(&name, Linkage::Local, false, false)
            .map_err(de)?;
        let mut desc = DataDescription::new();
        desc.define(bytes.to_vec().into_boxed_slice());
        self.module.define_data(data_id, &desc).map_err(de)?;
        let gv = self.module.declare_data_in_func(data_id, self.b.func);
        let ptr_ty = self.module.target_config().pointer_type();
        let ptr = self.b.ins().global_value(ptr_ty, gv);
        let ptr = if ptr_ty != I64 {
            self.b.ins().uextend(I64, ptr)
        } else {
            ptr
        };
        let len = self.b.ins().iconst(I64, bytes.len() as i64);
        Ok((ptr, len))
    }
}

/// A process-global counter for unique data-object names across compilations.
fn next_data_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}
