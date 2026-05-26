# product-cli (absorbed)

This crate is the absorption point for the product-cli engineering-artifact
tool, landed into the decision-cli Cargo workspace per [ADR-067] and
[FT-105]. The full upstream sources at
[`github.com/Hafeok/product-cli`][upstream] merge into `crates/product-cli/`
via `git subtree add --prefix=crates/product-cli` at the absorption commit;
this directory currently hosts the **minimal absorption-shape skeleton** —
the public Rust API that `crates/decision-cli/` and `crates/product-shim/`
depend on — pending the subtree merge.

The structural invariants this crate must hold (asserted by FT-105's TCs)
are:

- **SDP boundary (TC-179).** `crates/product-cli/Cargo.toml` does not
  depend on `decision-cli` or `oxi-events`, and the source tree contains
  no `use decision_cli::*` or `use oxi_events::*` statements. The
  dependency direction is `decision-cli → product-cli → (nothing in
  this workspace)`.
- **Workspace member (TC-178).** `cargo build --workspace` and
  `cargo test --workspace` complete cleanly with this crate as a member.
- **CLI surface (TC-176).** `dec product <verb>` and the `product`
  binary route through the same `build_command` / `dispatch` pair, so
  observable stdout is byte-identical for the parity set.
- **MCP surface (TC-177).** `tool_descriptors()` enumerates the
  product-cli MCP tools whose names the combined `dec mcp` server folds
  into its registry alongside the `dec_*` tools.

[ADR-067]: ../../.product/adrs/
[FT-105]: ../../.product/features/
[upstream]: https://github.com/Hafeok/product-cli
