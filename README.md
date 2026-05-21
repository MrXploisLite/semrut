# SemRut

**Assembly-level control with Rust-like safety.**

SemRut (smrc) is a systems programming language that combines the low-level control of assembly with the memory safety guarantees of Rust. It compiles to LLVM IR, producing optimized native binaries for any target architecture.

## Why SemRut?

- **Hardware control** — inline assembly, SIMD types, explicit memory modes
- **Memory safety** — ownership/borrow checking by default, `unsafe` opt-out
- **Zero-cost abstractions** — comptime evaluation, generics, traits
- **LLVM backend** — battle-tested optimization, multi-arch support

## Quick Start

### Prerequisites

- Rust toolchain (for building smrc)
- LLVM 18

### Build the Compiler

```bash
git clone https://github.com/MrXploisLite/semrut.git
cd semrut
cargo build --release
```

The compiler binary is at `target/release/smrc`.

### Compile Your First Program

```rust
// hello.smr
fn main() -> i64 {
    print("Hello from SemRut!\n");
    return 0;
}
```

```bash
./target/release/smrc hello.smr -o hello
./hello
# Hello from SemRut!
```

## Language Features

### Memory Modes

| Mode | Description |
|------|-------------|
| `safe` (default) | Ownership + borrow checking |
| `pin` | Arena allocation, no move |
| `raw` | Full manual control (unsafe) |

### Ownership & Borrowing

```rust
fn main() -> i64 {
    let x: i64 = 42;
    let y: i64 = x;  // copy (i64 is Copy)
    return y;        // 42
}
```

Double mutable borrows, use-after-move, and other violations are caught at compile time.

### Inline Assembly

```rust
fn add(a: i64, b: i64) -> i64 {
    asm {
        out("rax") result
        in("rdi") a
        in("rsi") b
        "add rdi, rsi"
        "mov rax, rdi"
    }
    return result;
}
```

### Structs & Methods

```rust
struct Point {
    x: i64,
    y: i64,
}

impl Point {
    fn new(x: i64, y: i64) -> i64 {
        return x + y;
    }
}

fn main() -> i64 {
    let result: i64 = Point::new(3, 4);
    return result;  // 7
}
```

### Pattern Matching

```rust
fn unwrap(opt: i64) -> i64 {
    let result: i64 = match opt {
        0 => 0,
        1 => 100,
        _ => opt,
    };
    return result;
}
```

### Enums

```rust
enum Option {
    Some(i64),
    None,
}
```

### Standard Library

| Function | Signature | Description |
|----------|-----------|-------------|
| `print` | `(str) -> ()` | Print string |
| `print_int` | `(i64) -> ()` | Print integer |
| `alloc` | `(i64) -> *T` | Allocate memory |
| `free` | `(*T) -> ()` | Free memory |
| `memcpy` | `(*T, *T, i64) -> ()` | Copy memory |
| `memset` | `(*T, i64, i64) -> ()` | Set memory |

## Compiler Pipeline

```
Source (.smr)
    ↓
Lexer (handwritten scanner)
    ↓
Parser (recursive descent)
    ↓
Type Checker (name resolution, inference, coercion)
    ↓
Ownership Checker (borrow tracking, scope analysis)
    ↓
MIR Builder (basic blocks, explicit control flow)
    ↓
LLVM Codegen (inkwell → LLVM IR)
    ↓
Native Binary (optimized)
```

## Project Structure

```
semrut/
├── src/
│   ├── lexer/       # Handwritten tokenizer
│   ├── parser/      # Recursive descent parser + AST
│   ├── sema/        # Type checker + name resolution
│   ├── ownership/   # Borrow checker
│   ├── mir/         # Mid-level IR
│   ├── llvm/        # LLVM codegen
│   └── main.rs      # CLI entry point
├── tests/           # Test programs
└── examples/        # Example programs
```

## CLI Usage

```bash
smrc <input.smr> [OPTIONS]

Options:
  -o, --output <path>   Output binary path
  --dump-tokens         Print tokens and exit
  --dump-ast            Print AST and exit
  --dump-types          Print checked types and exit
  --dump-mir            Print MIR and exit
  --dump-llvm           Print LLVM IR and exit
  -O <0-3>              Optimization level (default: 0)
```

## Roadmap

- [ ] Generics (type parameters)
- [ ] Traits (interfaces)
- [ ] Pattern matching with enum destructuring
- [ ] SIMD vector types (`vec128<f32>`, `vec256<i64>`)
- [ ] Comptime evaluation
- [ ] Standard library expansion
- [ ] Cross-compilation targets

## License

GPL v3 — see [LICENSE](LICENSE) for details. SemRut is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

All derivative works must also be licensed under GPL v3. This ensures every improvement stays open and available to the community.
