# Node Graph Binary Editor — Reference Specification

> **Status:** Draft / Design phase  
> **Scope:** File format, type system, and node catalogue for a node-graph editor that compiles directly to binary machine code.

---

## Design Philosophy

- The node graph **is** the canonical source. There is no text serialization that the graph is derived from.
- The file format must be readable by a bootstrapped hex monitor — no query engine, no schema language, no parser beyond simple arithmetic on fixed-width records.
- The UI is a configuration surface. The file format stores only fully resolved, monomorphized types.
- Complexity is pushed up the stack, not down. The loader is dumb; the editor is smart.

---

## Wire Types (Edge Types)

Edges are typed. The type is determined by the source output port and must match the destination input port. Type mismatches are rejected at edit time, not compile time.

| Name   | Width  | Description                        |
|--------|--------|------------------------------------|
| `Bit`  | 1 bit  | Single boolean / logic value       |
| `Byte` | 8 bit  | Unsigned octet                     |
| `Word` | 64 bit | Machine word (unsigned)            |

**Types are distinct, not hierarchical.** A `Byte` is not 8 `Bit`s in the type system — explicit conversion nodes are required. This matches how hardware thinks about these widths.

**Default port type is `Byte`.** When a node is first placed, all ports default to `Byte`. The user may change the type via a port type selector before wiring.

---

## Monomorphization

The UI and the file format have different representations:

- **UI (`NodeKind`):** Nodes are untyped. An `And` node is just `And` — no width suffix. Wire type is inferred from what is connected at runtime.
- **File format:** All nodes are fully monomorphized. `And` at `Byte` width is stored as discriminant `AND_BYTE`. The loader performs a direct lookup — no type inference.

```
// UI node kind:
And   // no type — determined by connected wires

// File format discriminant:
type: AND_BYTE   // fully resolved u16
```

The editor is responsible for resolving the UI node + wire types to a concrete discriminant when writing the file. The loader is dumb.

---

## File Format

### Overview

The file is a flat binary composed of four consecutive sections:

```
[ Header ] [ Topology ] [ Wires ] [ Payload ]
```

All integers are little-endian.

### Header (fixed, 32 bytes)

| Offset | Size | Field                  | Description                        |
|--------|------|------------------------|------------------------------------|
| 0      | 4    | `magic`                | `0x53545250` ("STRP")              |
| 4      | 2    | `version`              | Format version, currently `0x0001` |
| 6      | 2    | `reserved`             | Must be zero                       |
| 8      | 4    | `node_count`           | Number of topology records         |
| 12     | 4    | `wire_count`           | Number of wire records             |
| 16     | 4    | `payload_section_offset` | Byte offset to payload section   |
| 20     | 4    | `root_node_id`         | ID of the top-level Sink node      |
| 24     | 8    | `reserved`             | Must be zero                       |

### Topology Section

Immediately follows the header. One record per node, fixed width (24 bytes).

| Offset | Size | Field            | Description                                |
|--------|------|------------------|--------------------------------------------|
| 0      | 4    | `id`             | Unique node ID (1-indexed; 0 = null)       |
| 4      | 4    | `parent_id`      | Enclosing scope node ID; 0 = top level     |
| 6      | 2    | `type`           | Node type discriminant (see catalogue)     |
| 8      | 4    | `pos_x`          | Canvas X position (f32)                   |
| 12     | 4    | `pos_y`          | Canvas Y position (f32)                   |
| 16     | 4    | `payload_offset` | Byte offset into payload section; 0 = none |
| 20     | 4    | `reserved`       | Must be zero                               |

Node `i` is located at `sizeof(Header) + i * 24`. O(1) lookup, no indirection.

Scope hierarchy is implicit: to find all children of node `N`, scan for records where `parent_id == N`. No nested sections.

### Wire Section

Immediately follows topology. One record per wire, fixed width (8 bytes).

| Offset | Size | Field       | Description                  |
|--------|------|-------------|------------------------------|
| 0      | 4    | `from_node` | Source node ID               |
| 1      | 1    | `from_port` | Output port index on source  |
| 4      | 4    | `to_node`   | Destination node ID          |
| 5      | 1    | `to_port`   | Input port index on dest     |

Wire types are not stored — they are implied by the port definitions of the connected node types. A validator may check consistency; the loader does not.

### Payload Section

Variable-length blob. Each node's payload is at `payload_offset` from the start of this section. Layout is defined per node type (see catalogue). Nodes with no payload have `payload_offset = 0` and that offset is not read.

---

## Node Type Catalogue

### Discriminant Ranges

| Range         | Category              |
|---------------|-----------------------|
| `0x0000`      | Reserved / null       |
| `0x0001–0x00FF` | Sink and structural |
| `0x0100–0x01FF` | Literals            |
| `0x0200–0x02FF` | Bitwise operations  |
| `0x0300–0x03FF` | Arithmetic          |
| `0x0400–0x04FF` | Byte manipulation   |
| `0x0500–0x05FF` | Scoping / structure |
| `0x0600–0x06FF` | Binary generation (reserved; built-in functions TBD) |

---

### Sink and Structural (`0x0001–0x00FF`)

#### `SINK` — `0x0001`
The single output of the graph. Consumes a stream of `Byte` values and produces the final binary artifact.

| Ports | Type   | Direction |
|-------|--------|-----------|
| `in`  | `Byte` | Input     |

No payload.

---

### Literals (`0x0100–0x01FF`)

#### `LIT_BIT` — `0x0100`
| Ports | Type  | Direction |
|-------|-------|-----------|
| `out` | `Bit` | Output    |

Payload: `1 byte` — value is `0x00` (false) or `0x01` (true).

#### `LIT_BYTE` — `0x0101`
| Ports | Type   | Direction |
|-------|--------|-----------|
| `out` | `Byte` | Output    |

Payload: `1 byte` — the literal value.

#### `LIT_WORD` — `0x0102`
| Ports | Type   | Direction |
|-------|--------|-----------|
| `out` | `Word` | Output    |

Payload: `8 bytes` — the literal value, little-endian.

---

### Bitwise Operations (`0x0200–0x02FF`)

Each logical operation is monomorphized per wire type. Ports `a` and `b` are inputs; `out` is output. All three share the same type.

| Node         | Discriminant | Inputs | Output |
|--------------|-------------|--------|--------|
| `AND_BIT`    | `0x0200`    | a, b   | out    |
| `AND_BYTE`   | `0x0201`    | a, b   | out    |
| `AND_WORD`   | `0x0202`    | a, b   | out    |
| `OR_BIT`     | `0x0203`    | a, b   | out    |
| `OR_BYTE`    | `0x0204`    | a, b   | out    |
| `OR_WORD`    | `0x0205`    | a, b   | out    |
| `XOR_BIT`    | `0x0206`    | a, b   | out    |
| `XOR_BYTE`   | `0x0207`    | a, b   | out    |
| `XOR_WORD`   | `0x0208`    | a, b   | out    |
| `NOT_BIT`    | `0x0209`    | a      | out    |
| `NOT_BYTE`   | `0x020A`    | a      | out    |
| `NOT_WORD`   | `0x020B`    | a      | out    |
| `NAND_BIT`   | `0x020C`    | a, b   | out    |
| `NAND_BYTE`  | `0x020D`    | a, b   | out    |
| `NAND_WORD`  | `0x020E`    | a, b   | out    |
| `NOR_BIT`    | `0x020F`    | a, b   | out    |
| `NOR_BYTE`   | `0x0210`    | a, b   | out    |
| `NOR_WORD`   | `0x0211`    | a, b   | out    |
| `SHL_BYTE`   | `0x0212`    | a, amount: Byte | out: Byte |
| `SHL_WORD`   | `0x0213`    | a, amount: Byte | out: Word |
| `SHR_BYTE`   | `0x0214`    | a, amount: Byte | out: Byte |
| `SHR_WORD`   | `0x0215`    | a, amount: Byte | out: Word |

No payload for any bitwise node.

---

### Arithmetic (`0x0300–0x03FF`)

All arithmetic nodes operate on `Word`. Wrap-around semantics (no trapping).

| Node       | Discriminant | Inputs | Output      |
|------------|-------------|--------|-------------|
| `ADD_WORD` | `0x0300`    | a, b   | out: Word   |
| `SUB_WORD` | `0x0301`    | a, b   | out: Word   |
| `MUL_WORD` | `0x0302`    | a, b   | out: Word   |
| `DIV_WORD` | `0x0303`    | a, b   | out: Word   |
| `MOD_WORD` | `0x0304`    | a, b   | out: Word   |

No payload.

---

### Byte Manipulation (`0x0400–0x04FF`)

#### `CONCAT` — `0x0400`
Concatenates a variable number of `Byte` inputs into a `Byte` stream.

| Ports    | Type   | Direction |
|----------|--------|-----------|
| `in[N]`  | `Byte` | Input (N inputs) |
| `out`    | `Byte` | Output stream |

Payload: `1 byte` — input count N.

#### `SLICE` — `0x0401`
Extracts a single `Byte` from a `Word` at a given bit offset.

| Ports    | Type   | Direction |
|----------|--------|-----------|
| `in`     | `Word` | Input     |
| `offset` | `Byte` | Input     |
| `out`    | `Byte` | Output    |

No payload.

#### `PACK` — `0x0402`
Packs 8 `Bit` inputs into a `Byte`.

| Ports        | Type   | Direction |
|--------------|--------|-----------|
| `bit[0..7]`  | `Bit`  | Input     |
| `out`        | `Byte` | Output    |

No payload.

#### `UNPACK` — `0x0403`
Unpacks a `Byte` into 8 `Bit` outputs.

| Ports        | Type   | Direction |
|--------------|--------|-----------|
| `in`         | `Byte` | Input     |
| `bit[0..7]`  | `Bit`  | Output    |

No payload.

---

### Scoping / Structure (`0x0500–0x05FF`)

Scope nodes are containers. Their `parent_id` chain defines the scope tree. Children of a scope node have their `parent_id` set to the scope node's `id`.

#### `FUNCTION` — `0x0500`
A named subgraph with typed input and output ports. Appears as a single node in the parent graph; contains a child graph.

Payload:
| Offset | Size | Field        | Description                    |
|--------|------|--------------|--------------------------------|
| 0      | 1    | `name_len`   | Length of name in bytes        |
| 1      | N    | `name`       | UTF-8 name                     |
| 1+N    | 1    | `in_count`   | Number of input ports          |
| 2+N    | 1    | `out_count`  | Number of output ports         |
| 3+N    | M    | `port_types` | One byte per port: wire type tag |

#### `MODULE` — `0x0501`
A namespace container for functions. Has no ports itself; provides scope.

Payload: same name structure as `FUNCTION`, no port counts.

#### `RECORD` — `0x0502`
A named fixed-layout collection of typed fields. Produces a `Byte` stream representing the serialized record.

Payload:
| Offset | Size | Field        | Description             |
|--------|------|--------------|-------------------------|
| 0      | 1    | `name_len`   | Length of name          |
| 1      | N    | `name`       | UTF-8 name              |
| 1+N    | 1    | `field_count` | Number of fields       |
| 2+N    | M    | `field_types` | One byte per field: wire type tag |

---

### Binary Generation (`0x0600–0x06FF`)

> **Design decision:** Binary generation operations (`INSTRUCTION`, `LABEL`, `BRANCH_TARGET`, `ELF_HEADER`, `PE_HEADER`) are **not** first-class node kinds in the UI. They are exposed as built-in `FUNCTION` nodes. The discriminant range `0x0600–0x06FF` is reserved; its contents are TBD pending the built-in function design.

---

## Wire Type Tags

Used in payload fields (port type arrays, field type arrays):

| Value  | Type   |
|--------|--------|
| `0x00` | `Bit`  |
| `0x01` | `Byte` |
| `0x02` | `Word` |

---

## Validation Rules

A conforming file must satisfy all of the following. A loader may reject non-conforming files without further processing.

1. Magic bytes match `0x53545250` ("STRP").
2. `node_count` and `wire_count` are consistent with file size.
3. All `parent_id` references resolve to existing node IDs.
4. All wire endpoint node IDs and port indices resolve to valid nodes and their defined port counts.
5. Wire source and destination port types match.
6. Exactly one `SINK` node exists at the top level (`parent_id = 0`).
7. The graph is acyclic (DAG). Cycles are a hard error.
8. No node has an unresolved type discriminant.

---

## Open Questions

- [ ] Target architecture field — per-graph or per-module? Where in the header?
- [ ] Multi-output instruction encoding (e.g. instructions that write to both `rd` and flags)
- [ ] How to represent register operands — as `Byte` literals, or a distinct `Reg` type?
- [ ] Endianness field in header, or always little-endian?
- [ ] Maximum name length for labels/functions
- [ ] Versioning strategy — reject unknown versions outright, or have a compatibility range?
- [ ] Built-in functions: how are they distinguished from user-defined `FUNCTION` nodes in the file format? Special discriminant range, a flag in the payload, or a reserved name prefix?
