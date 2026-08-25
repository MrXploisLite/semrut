# SemRut

**Assembly-level control with Rust-like safety.**

SemRut (smrc) is a systems programming language that combines the low-level control of assembly with the memory safety guarantees of Rust. It compiles to LLVM IR, producing optimized native binaries for any target architecture.

## Why SemRut?

- **Hardware control** — inline assembly, SIMD types, explicit memory modes
- **Memory safety** — ownership/borrow checking by default, `unsafe` opt-out
- **Zero-cost abstractions** — generics, traits, comptime evaluation
- **LLVM backend** — battle-tested optimization, multi-arch support

## Quick Start

### Prerequisites

- Rust toolchain (`rustup`, stable) for building smrc
- LLVM 18 exactly: `llc-18` and `llvm-config` from the same version. smrc
  generates IR through inkwell's `llvm18-1` bindings; linking against a newer
  libLLVM produces IR that `llc-18` cannot parse (see `.cargo/config.toml`
  for how this repo pins `LLVM_SYS_181_PREFIX` on Arch/CachyOS).

### Build the Compiler

```bash
git clone https://github.com/MrXploisLite/semrut.git
cd semrut
cargo build --release
```

The compiler binary is at `target/release/smrc`.

### Run the Test Suite

```bash
cargo test --release
```

The e2e suite compiles snippets with smrc, runs the produced binaries, and
asserts exit codes plus expected rejections from the borrow checker.

### Windows Setup

On Windows, install LLVM 18 via winget:
```powershell
winget install LLVM.LLVM --version 18.1.8
```
Then download `clang+llvm-18.1.8-x86_64-pc-windows-msvc.tar.xz` from the [LLVM releases page](https://github.com/llvm/llvm-project/releases/tag/llvmorg-18.1.8) and extract `llc.exe`, `llvm-config.exe` to your PATH.

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
    fn new(x: i64, y: i64) -> Point {
        return Point { x: x, y: y };
    }

    fn distance(self: Point) -> i64 {
        let x: i64 = self.x;
        let y: i64 = self.y;
        return x + y;
    }
}

fn main() -> i64 {
    let p: Point = Point { x: 3, y: 4 };
    let d: i64 = p.distance();
    return d;  // 7
}
```

### Enums & Pattern Matching

```rust
enum Option {
    Some(i64),
    None,
}

fn unwrap(opt: Option) -> i64 {
    let result: i64 = match opt {
        Option::Some(v) => v,
        Option::None => 0,
    };
    return result;
}
```

### Generics

```rust
fn identity<T>(x: T) -> T {
    return x;
}

fn main() -> i64 {
    let n: i64 = identity(42);
    return n;
}
```

### Traits

```rust
trait Printable {
    fn print(self: Point);
}

struct Point {
    x: i64,
    y: i64,
}

impl Printable for Point {
    fn print(self: Point) {
        let x: i64 = self.x;
        print_int(x);
    }
}

fn main() -> i64 {
    let p: Point = Point { x: 10, y: 20 };
    Printable::print(p);
    return 0;
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
│   ├── sema/        # Type checker + name resolution + traits
│   ├── ownership/   # Borrow checker
│   ├── mir/         # Mid-level IR
│   ├── llvm/        # LLVM codegen (inkwell 0.8, LLVM 18)
│   └── main.rs      # CLI entry point
├── tests/           # .smr sample programs + Rust e2e suite (compiler_e2e.rs)
├── examples/        # Example programs (valid and intentionally-invalid)
└── .cargo/config.toml  # Pins llvm-sys to the LLVM 18 toolchain
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
  -O <0-3>              Optimization level (default: 0, validated by the CLI)
  -V, --version         Print version
```

## Implemented Features

- [x] Lexer (handwritten scanner with keyword recognition)
- [x] Parser (recursive descent, full expression parsing)
- [x] Type checker (name resolution, type inference, coercion rules)
- [x] Ownership checker (borrow tracking, scope analysis, move semantics)
- [x] MIR builder (basic blocks, SSA-like, explicit control flow)
- [x] LLVM codegen (inkwell 0.8, LLVM 18, opaque pointers)
- [x] Standard library (print, print_int, alloc, free, memcpy, memset)
- [x] Control flow (if/else, while, loop, for..in, break, continue, return, implicit return)
- [x] Method calls with receiver (`receiver.method()`) and `self` shorthand
- [x] Struct literals with field type validation (`Type { field: value, ... }`)
- [x] Struct field access (LLVM GEP via `build_struct_gep`)
- [x] Enums with pattern matching (`match` expressions, enum destructuring)
- [x] Generics (type parameters `<T, U>`, type inference from call args)
- [x] Traits (interfaces, `impl Trait for Type`, method mangling)
- [x] Inline assembly (`asm { out/in/constraints }`)
- [x] Undefined values (`let x: T = undefined`)
- [x] References and dereferencing (`&mut`, `*`)
- [x] Source location tracking in error messages
- [x] End-to-end test suite (compile → run → assert exit code / rejection)
- [x] CLI validation (`--version`, `-O` range check)

## Roadmap

- [ ] Multi-error reporting (report every broken function in one pass; the
      per-function collection exists in sema, output still stops at the first)
- [ ] Trait bounds on generics (`<T: Printable>`)
- [ ] Trait method resolution (dispatch to correct impl)
- [ ] Comptime evaluation
- [ ] SIMD vector types (`vec128<f32>`, `vec256<i64>`)
- [ ] Standard library expansion (String, Vec, HashMap)
- [ ] Cross-compilation targets
- [ ] Safe/pin/raw memory mode enforcement
- [ ] Module system and imports
- [ ] Error handling (`Result`, `?` operator)

## License

GPL v3 — see [LICENSE](LICENSE) for details. SemRut is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

All derivative works must also be licensed under GPL v3. This ensures every improvement stays open and available to the community.
