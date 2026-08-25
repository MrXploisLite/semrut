# SemRut Roadmap: 0.0.6 → 1.0.0

> Target absolut proyek (tidak bisa dinegosiasikan):
>
> 1. **Performa**: setara C/C++, lalu melebihinya (zero-cost abstraction, no GC pause,
>    optimisasi LLVM maksimal + pass SemRut sendiri).
> 2. **Keamanan**: setara Rust, lalu melebihinya (ownership + borrow checking sudah ada;
>    lanjut ke data-race freedom, formal verification subset).
> 3. **General-purpose penuh**: OS, web, game, aplikasi desktop, tooling, embedded/Arduino.

Setiap versi = milestone yang bisa di-release, di-test, dan di-push. Tidak ada versi
yang "setengah jalan": kalau fitur belum stabil, dia masuk versi berikutnya.

---

## v0.0.7 — Fondasi Bahasa yang Utuh

Fokus: semua konstruksi dasar bekerja tanpa celah.

- [ ] Generic structs & enums (`struct Box<T> { ... }`) — monomorphize tipe, bukan cuma fn
- [ ] Trait method call via bound (`x.show()` ketika `T: Show`, dispatch statis)
- [ ] Default trait implementations (method dengan body di trait declaration)
- [ ] Operator overloading via trait (`Add`, `Eq`, dst.)
- [ ] String type beneran (`str` heap-allocated + literal, bukan cuma C string)
- [ ] Array & slice lengkap: iterasi, indexing bounds-checked (debug) / unchecked (release)
- [ ] Error messages dengan lokasi source (file:line:col + caret ^)

## v0.0.8 — Memory Model & Ownership Level Rust

Fokus: safety setara Rust, fondasi menuju lebih baik.

- [ ] Lifetime tracking eksplisit (`&'a T`) — dianalisis di ownership pass
- [ ] Drop semantics: destructor dipanggil otomatis di akhir scope (RAII)
- [ ] Move semantics lengkap untuk struct (bukan cuma primitive copy)
- [ ] `Option<T>` / `Result<T, E>` sebagai enum standar dengan `?` operator
- [ ] Panic handler + unwinding atau abort strategy yang jelas
- [ ] Fuzzing sema/ownership pass (cargo-fuzz): compiler tidak boleh crash pada input random

## v0.1.0 — Stdlib & Tooling Minimum Viable

Fokus: bahasa bisa dipakai bikin program nyata.

- [ ] Package manager `smr pkg` (init/build/test/publish, registry lokal dulu)
- [ ] Build system terintegrasi (incremental compile, cache)
- [ ] stdlib: collections (Vec, HashMap), io (file), math, time, fmt
- [ ] `smrc test` built-in (test functions dengan attribute `#[test]`)
- [ ] Doc comments (`///`) + `smrc doc` generator HTML
- [ ] LSP server dasar (autocomplete, go-to-definition, diagnostics) — editor support

## v0.2.0 — Performa: Mengejar & Menyalip C

Fokus: benchmark-driven, bukan feeling-driven.

- [ ] Benchmark suite resmi (Benchmarks Game set + real-world: JSON parse, matrix mul, fib)
- [ ] Optimizer passes SemRut sendiri di level MIR (inlining, DCE, constant folding)
- [ ] LLVM optimization pipeline tuning per `-O` level (0-3, size)
- [ ] PGO + LTO support (`smrc --pgo`)
- [ ] SIMD intrinsics eksplisit (`@vector` types)
- [ ] Zero-cost abstractions diverifikasi: generic/trait/iterator harus compile jadi kode
      identik dengan hand-written C (cek di IR)
- [ ] Target: ≥ 95% kecepatan C di seluruh benchmark suite sebelum lolos

## v0.3.0 — Keamanan Melebihi Rust

Fokus: fitur keamanan yang Rust belum punya (atau baru rancangan).

- [ ] Data-race freedom di compile time (ownership + thread model ala Rust `Send/Sync`
      tapi lebih ergonomis)
- [ ] Integer overflow: panik di debug, wrap/checked eksplisit di release (default aman)
- [ ] Null tidak ada di bahasa — `Option` wajib; pointer raw hanya dalam `unsafe` block
- [ ] `unsafe` block dengan audit trail (compiler catat semua unsafe usage)
- [ ] Formal verification subset: proof assistant opsional untuk fungsi kritis
      (mulai dari pre/post condition annotations)

## v0.4.0 — Multi-target: Embedded & Bare Metal

Fokus: Arduino/bare metal = bukti bahasa bisa "apa aja".

- [ ] `--target avr|arm-cortex-m|x86_64-baremetal` via LLVM targets
- [ ] `#![no_std]` mode: runtime minimal, allocation optional
- [ ] HAL crates untuk AVR (Arduino Uno/Mega) & ARM Cortex-M (STM32, RP2040)
- [ ] Blink LED demo end-to-end: `.smr` → hex file → flash → jalan di hardware asli
- [ ] Size profiling: binary footprint kompetitif dengan C

## v0.5.0 — Web & Aplikasi

Fokus: dua target deployment besar.

- [ ] WebAssembly first-class (`--target wasm32`, ABI clean, JS interop)
- [ ] DOM API bindings + framework ringan (komponen, reactive signals)
- [ ] Desktop app story: binding GUI toolkit (satu resmi, mis. custom immediate-mode)
- [ ] HTTP client + server stdlib (async runtime berbasis event loop sendiri)

## v0.6.0 — Game & Real-time

Fokus: performa real-time + ekosistem game.

- [ ] Game loop + windowing stdlib (abstraksi atas platform native)
- [ ] Graphics backend: Vulkan/Metal/DX12 abstraction + WebGL/WGPU untuk web
- [ ] Audio, input, asset pipeline dasar
- [ ] ECS framework resmi (data-oriented, cache-friendly)
- [ ] Demo game 2D lengkap sebagai proof + template project

## v0.7.0 — Self-hosting Compiler

Fokus: ujian pamungkas general-purpose-ness.

- [ ] Rewrite lexer + parser SemRut dalam SemRut
- [ ] Rewrite sema + ownership dalam SemRut
- [ ] Bootstrap: smrc-v1 (Rust) mengkompilasi smrc-v2 (SemRut), hasil identik
- [ ] Setelah self-hosted: pengembangan compiler pakai SemRut sendiri

## v0.8.0 — Menuju OS

Fokus: kernel development capability.

- [ ] Freestanding target x86_64 (UEFI bootloader → kernel stub)
- [ ] Kernel stdlib: paging, GDT/IDT, interrupts, timers, UART/serial console
- [ ] `#![kernel]` mode dengan safety guarantees di level kernel (no UB antar module)
- [ ] Demo OS: boot ke shell interaktif sederhana, ditulis 100% SemRut

## v0.9.0 — Stabilization & Ecosystem

Fokus: kematangan sebelum 1.0.

- [ ] Language spec resmi (grammar formal + semantics)
- [ ] Edition system (compatibility promises seperti Rust editions)
- [ ] FFI lengkap (C ABI both ways: panggil C dari SemRut, expose SemRut ke C)
- [ ] Cross-platform CI: Linux, macOS, Windows, WASM, AVR, ARM
- [ ] Security audit: fuzzing menyeluruh, MIRI-style interpreter untuk deteksi UB
- [ ] Komunitas: contribution guide, RFC process, changelog discipline

## v1.0.0 — Release Stabil

Syarat mutlak sebelum 1.0:

1. Benchmark ≥ C di mayoritas suite (dan tidak kalah >5% di satupun)
2. Safety: 0 known soundness holes; data-race free; memory-safe tanpa GC
3. Self-hosted compiler yang stabilize dirinya sendiri
4. Minimal 4 domain terbukti: CLI tools, web (WASM), embedded (Arduino), mini-OS
5. SemVer guarantee + edition stability untuk ecosystem
6. Dokumentasi lengkap: book, stdlib docs, spec, tutorial

---

## Prinsip Kerja (berlaku setiap versi)

1. **Test dulu, fitur kemudian** — setiap fitur punya e2e test positif & negatif.
2. **Tidak ada regresi** — full suite hijau sebelum commit; examples sweep wajib.
3. **Benchmark setiap perubahan optimizer** — performa itu ukuran, bukan opini.
4. **Keamanan tidak dikompromikan demi fitur** — borrow checker adalah suci.
5. **Dokumentasi ikut koding** — README/spec update di commit yang sama.
