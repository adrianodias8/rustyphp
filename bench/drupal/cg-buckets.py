#!/usr/bin/env python3
"""Group both engines' per-request instruction counts into comparable pieces.

  cg-buckets.py FERRO.tsv PHP.tsv [--cg FERRO0.out FERRO20.out PHP0.out PHP20.out M]

The .tsv files come from cg-handlers.py --tsv (H rows: interpreter handler
regions, self cost with inlined code; F rows: every other function, exclusive).
Each row goes to the first bucket whose rule matches its name; the rules are
the table below, one list per engine, so the mapping is reviewable.
With --cg, D1/LL misses and mispredicts per bucket come from the cachegrind
runs (function level: the interpreter function is one bucket there, its
handlers cannot be split without addresses).
"""
import argparse, collections, os, re

# (bucket, ferro patterns, php patterns); H rows are prefixed "H:" before matching.
RULES = [
    ("dispatch (fetch next op, frame/ip bookkeeping)",
     [r"^H:fn run_loop \[run\.rs\]", r"^H:run_loop \(prologue\)"],
     []),
    ("operand loads/stores (no Zend equivalent: CV/CONST operands are embedded)",
     [r"^H:Op::(LoadVar|LoadSlot|PushConst|StoreSlot|LoadVarPushConst|Dup|Pop|Swap|This|PushArgPlace|PushRef)$",
      r"reg_load_slot", r"reg_store_slot"],
     [r"^H:ZEND_(FREE|QM_ASSIGN)"]),
    ("user calls + return (frame setup, args, params, leave)",
     [r"^H:Op::(MethodCall|MethodCallArgs|Call|CallArgs|CallUser|StaticCall|StaticCallArgs|NewObj|New|Ret|RetNull|RetVoid|CoerceParams|CallValue|CallDyn)\w*$",
      r"methodcall_fast", r"resolve_method_runtime", r"Frame>::with_buffers", r"recycle_frame",
      r"drop_glue::<php_runtime::vm::Frame>", r"enter_callee", r"bind_params", r"coerce_param_hints",
      r"coerce_or_check_hint", r"Vm>::method_call$", r"dispatch_instance_call", r"HintKind as core::clone",
      r"value_satisfies_class", r"coerce_to_hint", r"FrameExt", r"invoke_value", r"push_frame", r"call_user",
      r"FramePool", r"Vec<php_runtime::vm::Frame>>::truncate", r"drop_glue::<php_runtime::hir::HintKind>",
      r"Vm>::call_function", r"static_call"],
     [r"^H:ZEND_(INIT_METHOD_CALL|INIT_STATIC_METHOD_CALL|INIT_FCALL|INIT_DYNAMIC_CALL|INIT_USER_CALL|DO_UCALL|DO_FCALL_SPEC|SEND_|RECV|RETURN|CHECK_UNDEF_ARGS|VERIFY_RETURN|NEW_|FETCH_THIS|CALLABLE_CONVERT|EXT_)",
      r"^H:zend_leave_helper", r"zend_verify_recv_arg_type_helper", r"zend_std_get_method", r"zend_init_func_execute_data",
      r"init_func_run_time_cache", r"zend_init_dynamic_call", r"zend_is_callable", r"zend_call_function", r"zend_verify_",
      r"zend_check_type", r"object_init_ex", r"zend_objects_new", r"zend_vm_stack_extend", r"i_init_func_execute_data",
      r"zend_get_executed", r"zend_fcall_info"]),
    ("builtin calls (resolution, arg passing; bodies excluded)",
     [r"^H:Op::(CallNsFallback|CallHostBuiltin|CallBuiltin\w*)$", r"call_ns_fallback_site", r"ns_site_target",
      r"value_builtin", r"run_value_builtin", r"host_builtin_canonical", r"builtin::Builtin", r"decay_arg",
      r"builtin_string_coerces"],
     [r"^H:ZEND_(INIT_NS_FCALL_BY_NAME|DO_ICALL|DO_FCALL_BY_NAME|FRAMELESS_ICALL|JMP_FRAMELESS|SEND_VAL|STRLEN|COUNT|TYPE_CHECK|DEFINED|IN_ARRAY|FUNC_GET_ARGS|GET_CLASS|ARRAY_KEY_EXISTS)",
      r"zend_parse_arg", r"zend_wrong_parameter"]),
    ("property access (fetch/assign/isset on objects)",
     [r"^H:Op::(ThisProp\w*|PropSet\w*|PropGet\w*|PropIsset|PropUnset\w*|FieldAssign|FieldGet\w*|PropIncDec\w*|StaticProp\w*)$",
      r"PropInfo", r"PropsLayout", r"resolve_prop_access", r"lazy_prop_access", r"magic_applies",
      r"prop_get_fallback", r"prop_set_entry", r"prop_get_entry", r"Props>::set", r"Props as core::clone",
      r"deref_object", r"prop_concat_in_place", r"object::Props", r"write_property_at", r"oop::read_property",
      r"coerce_typed_prop_write", r"asym_write_error", r"uninit_typed_read_at", r"as_arrayaccess", r"overload_receiver",
      r"InitProps"],
     [r"^H:ZEND_(FETCH_OBJ_(R|W|RW|FUNC_ARG|UNSET)|ASSIGN_OBJ|PRE_INC_OBJ|POST_INC_OBJ|PRE_DEC_OBJ|POST_DEC_OBJ|ASSIGN_STATIC_PROP|FETCH_STATIC_PROP|UNSET_OBJ)",
      r"zend_std_(read|write|has|get_property|unset)_property", r"zend_std_get_property_ptr_ptr",
      r"rebuild_object_properties", r"is_property_visibility_changed", r"zend_get_property_offset",
      r"zend_get_property_info", r"zend_fetch_property_address", r"zend_std_get_properties", r"zend_read_property",
      r"zend_update_property", r"zend_fetch_static_property"]),
    ("isset/empty on paths ($this->a[$k], $a[$k][...])",
     [r"^H:Op::(FieldIsset|IssetPath|EmptyPath|IssetDim\w*|EmptyDim\w*|Isset\w*|Empty\w*)$", r"field_isset_op",
      r"is_walk_resume", r"silent_walk", r"arrays::field_get"],
     [r"^H:ZEND_(ISSET_ISEMPTY|FETCH_OBJ_IS|FETCH_DIM_IS)", r"zend_fetch_dimension_address_read_IS",
      r"zend_isset_dim_slow", r"zend_isempty_dim_slow", r"zend_is_true", r"zend_std_has_dimension"]),
    ("arrays (dim fetch/assign, iteration, insert, copy, destroy)",
     [r"^H:Op::(FetchDim\w*|AssignPath|AssignDim\w*|ArrayInit|ArrayPush\w*|ArrayLit\w*|IterNext|IterInit|IterEnd|IterKey\w*|Unset\w*|AppendPath\w*|ArrayKey\w*)$",
      r"PhpArray>::(insert|append|remove)", r"KeyIndex>::insert", r"Key>::from_bytes", r"PhpArray as core::ops::drop",
      r"drop_glue::<php_types::array", r"Rc<php_types::array::PhpArray>>::drop_slow", r"coerce_key", r"field_write_walk",
      r"arrays::", r"PhpArray>::", r"array::Repr", r"base_cell", r"pop_field_keys", r"path_walk", r"path_op",
      r"make_cell", r"field_lazy_root", r"field_aa_leaf", r"base_field_cell", r"KeyIndex>::rebuild", r"PhpArray as core::clone",
      r"Rc<php_types::array::PhpArray>>::make_mut", r"Key>::from_zstr"],
     [r"^H:ZEND_(FETCH_DIM|ASSIGN_DIM|FE_|ADD_ARRAY_ELEMENT|INIT_ARRAY|UNSET_DIM|FETCH_LIST|ADD_ARRAY_UNPACK|SEND_UNPACK)",
      r"zend_hash_(add|update|next_index|del|real_init|rehash|extend|index_add|index_update|copy|clean|packed|to_packed|iterator)",
      r"zend_array_(destroy|dup|count)", r"_zend_new_array", r"zend_fetch_dimension_address", r"zend_new_array",
      r"zend_hash_str_(add|update|del)", r"zend_hash_destroy", r"slow_index_convert", r"zend_assign_to_dim"]),
    ("hash lookup (arrays, symbol/class/function/constant tables)",
     [r"KeyIndex>::lookup", r"hashbrown::map::HashMap<.*>>::(get|contains_key|insert|remove)", r"sip::Hasher",
      r"FxBuildHasher as core::hash::BuildHasher", r"RandomState", r"ci_hash", r"hash_one", r"reserve_rehash",
      r"findElementWithHash"],
     [r"zend_hash_(find|lookup|func|index_find|index_lookup|str_find|find_known_hash|find_bucket)", r"zend_string_hash_func",
      r"zend_accel_hash_find", r"zend_inline_hash_func", r"zend_hash_str_find"]),
    ("memory allocator",
     [r"^_?mi_", r"malloc", r"^free$", r"_int_free", r"finish_grow", r"RawVecInner", r"box_new_uninit", r"realloc",
      r"unlink_chunk", r"memset", r"grow_one", r"dbMallocRaw"],
     [r"_e(malloc|free|realloc)", r"zend_mm_", r"malloc", r"^free$", r"_int_free", r"unlink_chunk", r"memset", r"realloc"]),
    ("value copy/drop, refcounting, object ids",
     [r"drop_glue::<php_types::zval::Zval>", r"Zval as core::clone", r"zstr_drop_slow", r"drop_slow", r"deref_clone",
      r"Vm>::next_id", r"BTreeMap<u32", r"push_mut", r"zstr::PhpStr>::new", r"object::drop_bounded",
      r"release_quarantined_ids", r"pop_pending_dtor", r"track_object", r"Object as core::ops::drop",
      r"drop_glue::<php_types::object::Object>", r"drop_glue::<\[php_types::zval::Zval\]>",
      r"drop_glue::<alloc::vec::Vec<core::option::Option<alloc::rc::Rc"],
     [r"rc_dtor_func", r"zend_objects_store_(del|put)", r"zend_object_std_dtor", r"zval_ptr_dtor", r"i_zval_ptr_dtor",
      r"zend_objects_destroy", r"zend_object_dtor"]),
    ("cycle GC bookkeeping",
     [r"gc_note", r"^H:Op::Sweep$", r"Vm>::gc_", r"sweep"],
     [r"gc_possible_root", r"gc_remove_from_buffer", r"zend_gc_", r"gc_check_possible_root"]),
    ("include, autoload, class linking per request",
     [r"^H:Op::(Include\w*|DeclareDeferred|DeclareClass\w*|DeclareFunc\w*|ClassConst|ConstFetch|DefineConst|InstanceOf|NewClassRef\w*)$",
      r"run_linked", r"unit_slot_pos", r"resolve_class_autoload", r"user_wrapper_url", r"run_module_with_hir",
      r"unit_cache_get", r"revalidated_unit_key", r"defercache", r"UnitKey", r"resolve_dynamic_class", r"resolve_constant",
      r"instance_of", r"lru::", r"try_autoload", r"resolve_name_autoload", r"unit_fp", r"unit_class_remap",
      r"unit_key_for", r"is_user_wrapper_path_op", r"is_user_stream_op", r"std::sys::fs::metadata", r"CString",
      r"ensure_static", r"run_deferred", r"vm_new", r"Module, usize"],
     [r"^H:ZEND_(INCLUDE_OR_EVAL|DECLARE_|FETCH_CLASS|FETCH_CONSTANT|INSTANCEOF|BIND_|FETCH_CLASS_CONSTANT)",
      r"persistent_compile_file", r"zend_accel_load_script", r"accel_init_interned_string", r"zend_do_link_class",
      r"zend_include_or_eval", r"php_stat", r"zif_file_exists", r"php_stream_locate_url_wrapper", r"zend_lookup_class",
      r"zend_is_valid_class_name", r"zend_fetch_class", r"zend_get_class_constant", r"zend_accel_", r"zend_instanceof_function",
      r"instanceof_function", r"zend_get_constant", r"zend_bind_class", r"do_bind", r"zend_try_early_bind",
      r"zend_string_tolower", r"spl_perform_autoload", r"zend_update_class_constants", r"accel_make_persistent_key",
      r"compile_filename", r"zend_file_handle_dtor", r"destroy_op_array", r"zend_ast_evaluate"]),
    ("unserialize (cache rows)",
     [r"Vm>::us_", r"unserialized", r"us_parse"],
     [r"var_unserialize", r"parse_iv", r"var_push", r"object_common", r"php_var_unserializer", r"var_destroy",
      r"process_nested"]),
    ("SQLite",
     [r"sqlite3", r"^yy_", r"pthread_mutex", r"keywordCode", r"resolveExprStep", r"tokenExpr"],
     [r"sqlite3", r"^yy_", r"pthread_mutex", r"^0x0000"]),
    ("strings + mem primitives (compare, copy, search, concat, convert)",
     [r"^bcmp", r"memcpy", r"memchr", r"memmem", r"strlen", r"Utf8Chunks", r"concat", r"to_zstr", r"numstr",
      r"^H:Op::(Concat\w*|Stringify\w*|Interp\w*)$", r"memmove", r"convert::"],
     [r"^bcmp", r"memcpy", r"memchr", r"strlen", r"zend_memnstr", r"^H:ZEND_(CONCAT|FAST_CONCAT|ROPE_|CAST)",
      r"concat_function", r"smart_str", r"zend_string_concat", r"memmove", r"zend_strtol", r"_zend_handle_numeric",
      r"is_numeric", r"zend_long_to_str", r"zend_dval_to_lval"]),
    ("builtin bodies (string, regex, hash, output ...)",
     [r"php_builtins::", r"frame_local", r"fancy_regex", r"regex", r"write_output", r"flush_diags", r"Timespec", r"sha2", r"sha1"],
     [r"^zif_", r"^php_", r"^zflf_", r"_zend_substr", r"SHA256", r"pcre", r"^ZEND_FRAMELESS", r"^zend_fetch_debug_backtrace",
      r"php_escape_html", r"php_strtr", r"php_char_to_str", r"sapi_", r"php_output"]),
    ("compare/arith/branch ops",
     [r"^H:Op::(CmpJmp\w*|JumpIf\w*|Jump|Unary|Binary\w*|Cmp\w*|Not|BoolNot|IncDec\w*|Match\w*|Coalesce\w*)$",
      r"binary_value", r"cmp_jmp", r"apply_binop", r"binop", r"cmp::", r"to_bool", r"loose_eq"],
     [r"^H:ZEND_(IS_|JMP|BOOL|ADD|SUB|MUL|DIV|MOD|PRE_INC|POST_INC|PRE_DEC|POST_DEC|COALESCE|JMP_SET|CASE|MATCH|BW_|SL|SR|SPACESHIP|ASSIGN_OP)",
      r"zend_is_identical", r"compare_function", r"zend_compare", r"zend_is_equal", r"string_compare", r"zendi_smart"]),
    ("other interpreter ops (assign, misc)",
     [r"^H:Op::"],
     [r"^H:ZEND_"]),
    ("interpreter, unattributed (inlined code without a run.rs / zend_vm_execute.h frame)",
     [r"^H:"],
     [r"^H:"]),
]


def classify(name, kind, engine):
    key = ("H:" if kind == "H" else "") + name
    col = 1 if engine == "ferro" else 2
    for r in RULES:
        if any(re.search(p, key) for p in r[col]):
            return r[0]
    return "other"


def load_tsv(path, engine):
    tot, b, members = 0, collections.Counter(), collections.defaultdict(collections.Counter)
    for l in open(path):
        kind, name, ir, ex = l.rstrip("\n").split("\t")
        if kind == "T":
            tot = int(ir); continue
        k = classify(name, kind, engine)
        b[k] += int(ir)
        members[k][(kind, name)] += int(ir)
    return tot, b, members


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ferro"); ap.add_argument("php")
    ap.add_argument("--cg", nargs=5)
    ap.add_argument("--members", type=int, default=0)
    a = ap.parse_args()
    tf, bf, mf = load_tsv(a.ferro, "ferro")
    tp, bp, mp = load_tsv(a.php, "php")
    order = [r[0] for r in RULES] + ["other"]
    print("| piece | ferro Ir | php Ir | ferro / php | excess (ferro − php) | % of the gap |")
    print("|---|---:|---:|---:|---:|---:|")
    gap = tf - tp
    for k in sorted(order, key=lambda k: -(bf[k] - bp[k])):
        if bf[k] or bp[k]:
            r = f"{bf[k] / bp[k]:.1f}×" if bp[k] else "—"
            print(f"| {k} | {bf[k]:,} | {bp[k]:,} | {r} | {bf[k] - bp[k]:,} | {100 * (bf[k] - bp[k]) / gap:.1f} |")
    print(f"| **total** | **{tf:,}** | **{tp:,}** | **{tf / tp:.2f}×** | **{gap:,}** | 100 |")
    if a.members:
        for k in order:
            print(f"\n### {k}\nferro: " + ", ".join(f"{n[1][:60]} {v:,}" for n, v in mf[k].most_common(a.members)))
            print("php: " + ", ".join(f"{n[1][:60]} {v:,}" for n, v in mp[k].most_common(a.members)))
    if a.cg:
        here = os.path.dirname(os.path.abspath(__file__))
        src = open(os.path.join(here, "cg-funcs.py")).read().replace("\nmain()\n", "\n")
        cgf = {}
        exec(compile(src, "cg-funcs.py", "exec"), cgf)
        m = int(a.cg[4])
        out = {}
        for eng, (p0, p1) in (("ferro", a.cg[0:2]), ("php", a.cg[2:4])):
            ev, r0 = cgf["load"](p0)
            _, r1 = cgf["load"](p1)
            acc = collections.defaultdict(lambda: [0] * len(ev))
            for key in set(r0) | set(r1):
                fn = cgf["short"](key[1])
                interp = fn.endswith("run_loop") or fn == "execute_ex" or fn.endswith("_HANDLER")
                k = "interpreter (all handlers + dispatch)" if interp else classify(fn, "F", eng)
                d = [x - y for x, y in zip(r1.get(key) or [0] * len(ev), r0.get(key) or [0] * len(ev))]
                for i, x in enumerate(d):
                    acc[k][i] += x / m
            out[eng] = (ev, acc)
        ev = out["ferro"][0]
        cols = ["D1mr", "D1mw", "DLmr", "DLmw", "Bcm", "Bim"]
        print("\n| piece | " + " | ".join(f"ferro {c} | php {c}" for c in cols) + " |")
        print("|---|" + "---:|" * (2 * len(cols)))
        keys = set(out["ferro"][1]) | set(out["php"][1])
        f1 = lambda e, k, c: out[e][1][k][out[e][0].index(c)] if k in out[e][1] else 0
        for k in sorted(keys, key=lambda k: -(f1("ferro", k, "D1mr") + f1("ferro", k, "D1mw"))):
            print(f"| {k} | " + " | ".join(f"{f1('ferro', k, c):,.0f} | {f1('php', k, c):,.0f}" for c in cols) + " |")


main()
