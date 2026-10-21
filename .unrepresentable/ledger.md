# .unrepresentable/ledger.md

## Expect ledger

| File | Item | Lint | Reason |
| --- | --- | --- | --- |
| — | — | — | — |

## Lint omissions

| Lint | Contradicts | Reason |
| --- | --- | --- |
| `clippy::question_mark_used` | `clippy::question_mark` | question_mark rewrites early returns into `?`, which question_mark_used forbids; with unwrap, expect and fallbacks banned, `?` is the only way left to propagate an error. |
| `clippy::implicit_return` | `clippy::needless_return` | implicit_return requires an explicit `return` on every tail expression, which needless_return forbids. |
| `clippy::single_call_fn` | `clippy::too_many_lines` | splitting long functions to satisfy too_many_lines produces helpers with a single caller. |
| `clippy::pub_with_shorthand` | `clippy::pub_without_shorthand` | `pub(in crate)` and `pub(crate)` are mutually exclusive spellings; the workspace uses `pub(crate)`. |
| `clippy::pattern_type_mismatch` | `clippy::ref_patterns` | matching through references needs either `ref` bindings (banned by ref_patterns) or explicit `&` patterns that cannot bind non-Copy fields. |
| `clippy::separated_literal_suffix` | `clippy::unseparated_literal_suffix` | the two require opposite spellings of a literal suffix; the workspace writes `1_u8`. |
| `clippy::self_named_module_files` | `clippy::mod_module_files` | `mod.rs` and `name.rs` + `name/` are mutually exclusive layouts; the workspace uses `name.rs` + `name/`. |
| `clippy::renamed_function_params` | `clippy::min_ident_chars` | trait impls must reuse the trait's parameter names, and std traits such as `Display::fmt` name theirs `f`, which min_ident_chars forbids. |
| `clippy::big_endian_bytes` | - | JPEG markers and ISO boxes are big-endian by specification; the byte order is the format's, not a choice. |
| `clippy::little_endian_bytes` | - | AVI/RIFF chunks are little-endian by specification, and exact truncation of wide arithmetic reads the low bytes; the byte order is the format's, not a choice. |
| `clippy::exhaustive_enums` | - | `#[non_exhaustive]` forces a wildcard arm in every other crate that matches the type, which unrepresentable's catch-all-arm rule forbids. |
| `clippy::exhaustive_structs` | - | `#[non_exhaustive]` forbids building the struct literally in other crates, so every plain value type would need a constructor taking its fields as primitives, which primitive-signature forbids. |
| `clippy::pub_use` | - | each crate root is held to module declarations and re-exports by ci/check-module-size.sh; re-exporting is the root's only job. |
| `clippy::missing_trait_methods` | - | std traits such as `Iterator` and `Display` have many provided methods; restating each one adds no safety. |
| `clippy::field_scoped_visibility_modifiers` | - | a private field would need an accessor returning its primitive, which primitive-signature forbids; a crate-scoped field is how a newtype exposes its value inside its crate. |
| `clippy::missing_inline_in_public_items` | - | cross-crate inlining comes from `lto = "fat"` and `codegen-units = 1` in the release profile; an `#[inline]` on every public item would restate that by hand. |
| `clippy::arbitrary_source_item_ordering` | - | items are ordered by reading order (type, then its constructors, then its uses), and reordering the frozen-vector files would change their recorded digests. |
| `default_overrides_default_fields` | unstable | unstable on the pinned stable toolchain: setting it is an `unknown lint` error. |
| `deprecated_llvm_intrinsic` | unstable | unstable on the pinned stable toolchain: setting it is an `unknown lint` error. |
| `implicit_provenance_casts` | unstable | unstable on the pinned stable toolchain: setting it is an `unknown lint` error. |
| `multiple_supertrait_upcastable` | unstable | unstable on the pinned stable toolchain: setting it is an `unknown lint` error. |
| `must_not_suspend` | unstable | unstable on the pinned stable toolchain: setting it is an `unknown lint` error. |
| `non_exhaustive_omitted_patterns` | unstable | unstable on the pinned stable toolchain: setting it is an `unknown lint` error. |
| `resolving_to_items_shadowing_supertrait_items` | unstable | unstable on the pinned stable toolchain: setting it is an `unknown lint` error. |
| `shadowing_supertrait_items` | unstable | unstable on the pinned stable toolchain: setting it is an `unknown lint` error. |
| `tail_call_track_caller` | unstable | unstable on the pinned stable toolchain: setting it is an `unknown lint` error. |
| `test_unstable_lint` | unstable | unstable on the pinned stable toolchain: setting it is an `unknown lint` error. |
| `unqualified_local_imports` | unstable | unstable on the pinned stable toolchain: setting it is an `unknown lint` error. |
| `clippy::module_name_repetitions` | - | each crate re-exports its types flat from the crate root, so the module name is never part of the path a caller writes; `FrameCount` would otherwise become `frame::Count` only inside its own crate. |
