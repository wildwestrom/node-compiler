# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A node-graph editor that compiles directly to binary machine code. The node graph **is** the canonical source — there is no text serialization. The UI is a configuration surface; the file format stores fully resolved, monomorphized types.

The binary is named `node-compiler` (Cargo package name). Currently early-stage: `src/main.rs` has the `NodeKind` and `Value` enums sketched out, with egui-snarl wired in as the node graph UI framework.

## Commands

```bash
cargo build          # build
cargo run            # run
cargo check          # type-check without linking
cargo test           # run all tests
cargo test <name>    # run a single test by name
cargo fmt            # format
cargo clippy         # lint
nix develop          # enter dev shell (NixOS environment)
rusty-man <crate>    # after running `cargo doc` use it to get documentation
```

This project uses jujutsu so for any git commands, use whatever the `jj` equivalent is.

## Architecture

### Key dependencies

- **egui** — immediate-mode GUI framework
- **egui-snarl** — node graph widget for egui; provides `SnarlViewer` trait to implement for custom node rendering

### Core types (`src/main.rs`)

- `Value` — runtime value enum (Bytes, Int, Float, Bool, Text, List, Map, Null)
- `NodeKind` — node type enum covering primitives (Literal, FileSink, FileSource), byte manipulation (Concat, Slice, EncodeU32, DecodeU32), control flow (Map, Fold, Branch), introspection (GraphToList, ListToGraph), and composition (Subgraph)

### Binary file format (STRP)

Specified in `node-graph-spec.md`. The format is a flat binary with four sections: `[Header][Topology][Wires][Payload]`, all little-endian. Key design points:

- Fixed-width topology records (24 bytes/node) allow O(1) lookup
- Wire types are not stored — implied by port definitions of connected node types
- Nodes are fully monomorphized: `AND_BIT`, `AND_BYTE`, `AND_WORD` are distinct discriminants (no type inference at load time)
- Wire types: `Bit` (1-bit), `Byte` (8-bit), `Word` (64-bit) — **not hierarchical**, explicit conversion nodes required between types
- Default port type is `Byte`
- Node discriminant ranges: `0x0001–0x00FF` structural, `0x0100` literals, `0x0200` bitwise, `0x0300` arithmetic, `0x0400` byte manipulation, `0x0500` scoping, `0x0600` binary generation
- The graph must be a DAG (cycles are a hard error); exactly one top-level `SINK` node required
