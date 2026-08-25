# SemRut Roadmap: 0.0.6 → 1.0.0 (revisi riset 2026)

> Replan Agustus 2026 berdasarkan evaluasi ekosistem bahasa pemrograman &
> toolchain tahun 2026 terhadap sumber-sumber publik (daftar lengkap di bagian
> bawah dokumen ini).
>
> **Target absolut proyek (tidak bisa dinegosiasikan):**
>
> 1. **Performa**: setara C/C++, lalu melebihinya.
> 2. **Keamanan**: setara Rust, lalu melebihinya.
> 3. **General-purpose penuh**: OS, web, game, aplikasi, tooling, embedded.

## Perubahan penting dari roadmap lama (kenapa direplan)

1. **Polonius Alpha sudah di nightly Rust** (stabil akhir 2026). Pelajaran:
   borrow-checker generasi baru itu realistis, tapi butuh formulasi dataflow yang
   benar dari awal. SemRut desain ownership pass sekali jadi, bukan NLL-dulu-lalu-diganti.
2. **WASI 0.3 ship Feb 2026; WASI 1.0 baru akhir 2026/awal 2027.** Target web SemRut
   pakai wasip2 yang stabil dulu — jangan bind ke API yang belum beku.
3. **Zig masih belum 1.0 di 2026 dan itu melukai adopsi; Odin & Mojo baru merilis 1.0.**
   Pelajaran: janji stabilitas lebih awal = keunggulan kompetitif. Edition system
   dipikirkan sejak v0.x, bukan menjelang 1.0.
4. **Query-based incremental compilation bukan gratis makan siang**
   (matklad, Feb 2026): mulai dari module-level caching sederhana, bukan Salsa-like engine.
5. **Formal verification sudah keluar dari niche** (Kani/Verus produksi 2026).
   "Safety melebihi Rust" jadi milestone konkret, bukan slogan: harness-based model
   checking + annotation syntax dicadangkan sejak sekarang.
6. **Embedded: AVR 8-bit bukan starting point yang baik.** Mulai dari ARM Cortex-M
   (RP2040 / ESP32-C3 / Arduino Uno R4) dengan pola PAC→HAL→BSP ala embedded-hal v1.0.
7. **Redox OS membuktikan kernel Rust-level safety bisa sampai jauh** (cargo+rustc
   jalan di dalam Redox per Jan 2026). Jalur OS SemRut mengikuti pola terbukti:
   UEFI → no_std stub → serial console → shell.

---

## v0.0.7 — Fondasi Bahasa yang Utuh

- [ ] Generic structs & enums (monomorphize tipe, bukan cuma fn)
- [ ] Static trait dispatch via bound: `x.show()` ketika `T: Show`
      (mangle `Trait::Type` yang sudah dibangun di v0.0.6 → panggil method impl)
- [ ] Default trait implementations
- [ ] Operator overloading via trait (`Add`, `Eq`, dst.)
- [ ] `str` heap-allocated beneran + string literal
- [ ] Array/slice lengkap; bounds-check on debug, off di `-O` release
- [ ] Error messages dengan lokasi source (file:line:col + caret)
      *Catatan riset: ini prasyarat LSP; span harus masuk AST sejak sekarang.*

## v0.0.8 — Memory Model & Ownership Level Rust

- [ ] Lifetime tracking eksplisit (`&'a T`) — desain dataflow ala Polonius
      sejak awal (facts + rules), hindari rework NLL→Polonius
- [ ] Drop semantics / RAII (destructor otomatis di akhir scope)
- [ ] Move semantics penuh untuk struct
- [ ] `Option<T>` / `Result<T, E>` standar + `?` operator
- [ ] Panic handler + abort strategy yang jelas
- [ ] Fuzzing sema/ownership pass (compiler tidak boleh panic/crash pada input acak)
- [ ] **Cadangan sintaks** untuk pre/post-condition annotations
      (dipakai formal verification di v0.3.0 — dicadangkan sekarang agar tidak breaking)

## v0.1.0 — Stdlib & Tooling Minimum Viable

- [ ] Package manager `smr` (init/build/test/publish; registry lokal dulu,
      format index mirip crates.io sparse-index yang terbukti)
- [ ] Build system terintegrasi; **module-level incremental caching**
      (hash per file — BUKAN query-engine penuh; lihat catatan matklad)
- [ ] stdlib: collections (Vec, HashMap), io, math, time, fmt
- [ ] `smrc test` built-in (`#[test]`)
- [ ] Doc comments `///` + generator HTML
- [ ] LSP server dasar: backend = compiler sema sendiri;
      tree-sitter grammar sebagai pelengkap highlighting (pola standar 2026)
- [ ] Async I/O: trait reactor-abstrak SEJAK AWAL supaya backend epoll/io_uring
      bisa ditukar; thread-per-core sebagai mode runtime opsional

## v0.2.0 — Performa: Mengejar & Menyalip C

- [ ] Benchmark suite resmi dengan metodologi anti-ngawur (lihat Riset 04):
      micro (Benchmarks Game set) + real-world (JSON, matmul, regex, HTTP req/s);
      pinned CPU, median+stddev multi-run, regression gate di CI
- [ ] Optimizer passes sendiri di level MIR — evolusi MIR ke SSA-form
      (SSA adalah standar de-facto middle-end; lihat Riset 01)
- [ ] LLVM pipeline tuning per `-O` level; PGO + ThinLTO support
- [ ] Upgrade path LLVM tahunan (sekarang pin llvm18; LLVM 23 sudah rilis —
      inkwell feature flag ganti per versi, jadwalkan migrasi tiap ~2 versi LLVM)
- [ ] SIMD intrinsics eksplisit (`@vector`)
- [ ] Verifikasi zero-cost: IR hasil generic/trait/iterator harus identik
      dengan hand-written C pada kasus uji
- [ ] Gate rilis: ≥95% kecepatan C-equivalent di seluruh suite sebelum klaim apa pun

## v0.3.0 — Keamanan Melebihi Rust

- [ ] Data-race freedom compile-time (model ownership antar-thread,
      semacam Send/Sync tapi ergonomis)
- [ ] Integer overflow: panik di debug; wrap/checked eksplisit di release
- [ ] Null tidak ada di bahasa; raw pointer hanya dalam `unsafe` block
      + audit trail semua penggunaan unsafe
- [ ] Formal verification subset (realistis di 2026 — Kani/Verus sudah produksi):
      - harness-based model checking untuk stdlib inti
      - pre/post-condition annotations (sintaks dicadangkan di v0.0.8)
      - target pertama: buktikan properti safety collection inti (Vec, HashMap)

## v0.4.0 — Embedded & Bare Metal

- [ ] `--target arm-cortex-m|riscv|avr|x86_64-baremetal`
- [ ] `#![no_std]` mode: runtime minimal, alloc optional
- [ ] HAL layer ala embedded-hal v1.0 (trait-based, portable lintas chip);
      **mulai dari RP2040 / ESP32-C3 / Arduino Uno R4 (Cortex-M)** —
      AVR Uno klasik menyusul (8-bit, 32KB, terbatas)
- [ ] probe-rs based flashing/debugging workflow
- [ ] Acceptance test: blink LED end-to-end di hardware asli +
      binary size kompetitif dengan C equivalent

## v0.5.0 — Web & Aplikasi

- [ ] Target wasm32: core Wasm + JS glue dulu (wasm-bindgen-style),
      Component Model menyusul setelah CM 1.0 beku (roadmap Bytecode Alliance)
- [ ] WASI: target wasip2 (stabil); wasip3 pas sudah GA — JANGAN bind API belum beku
- [ ] DOM bindings + framework ringan reactive-signals
- [ ] Desktop GUI: satu toolkit resmi (immediate-mode custom)
- [ ] HTTP client/server stdlib di atas async runtime v0.1.0

## v0.6.0 — Game & Real-time

- [ ] Windowing + game loop stdlib (abstraksi platform native)
- [ ] Graphics via wgpu-style abstraction (Vulkan/Metal/DX12/WebGPU)
- [ ] Audio, input, asset pipeline dasar
- [ ] ECS framework resmi (data-oriented ala Bevy 0.18 yang terbukti,
      TAPI API-stabil sejak hari pertama — pelajaran dari pre-1.0 Bevy pain)
- [ ] Demo game 2D lengkap + template project

## v0.7.0 — Self-hosting Compiler

Strategi incremental ala Go/8cc (bukan big-bang):

- [ ] Lexer + parser SemRut ditulis dalam SemRut (paralel dengan compiler Rust
      sebagai referensi oracle: output kedua implementasi harus identik)
- [ ] Sema + ownership dalam SemRut, phase-by-phase parity check
- [ ] Bootstrap loop: smrc-v1 (Rust) mengkompilasi smrc-v2 (SemRut);
      smrc-v2 mengkompilasi dirinya sendiri; hasil byte-identik (diverse
      double-compilation juga sebagai mitigasi "Trusting Trust")
- [ ] Setelah self-hosted: pengembangan compiler pakai SemRut sendiri

## v0.8.0 — Menuju OS

Jalur terbukti ala phil-opp/Redox:

- [ ] Freestanding x86_64: UEFI bootloader → kernel stub no_std
- [ ] Kernel core: GDT/IDT, paging, interrupts, timers, UART serial console
- [ ] `#![kernel]` mode dengan safety guarantees antar-module
- [ ] Demo OS: boot → shell interaktif sederhana 100% SemRut
- [ ] Referensi kesehatan ekosistem: rust-osdev aktif, Redox menjalankan
      cargo/rustc in-OS (2026) — bukti jalannya safety di kernel space

## v0.9.0 — Stabilization & Ecosystem

- [ ] Language spec resmi (grammar formal + semantics)
- [ ] **Edition system** — didesain SEBELUM 1.0 (pelajaran Zig tanpa 1.0 &
      breaking-change fatigue Bevy): compat promises sejak edisi pertama
- [ ] FFI dua arah (C ABI)
- [ ] Cross-platform CI: Linux, macOS, Windows, WASM, Cortex-M, RISC-V, AVR
- [ ] Security audit: fuzzing menyeluruh + interpreter ala Miri untuk deteksi UB
- [ ] Komunitas: contribution guide, RFC process, changelog discipline

## v1.0.0 — Release Stabil

Syarat mutlak (semua harus hijau):

1. Benchmark ≥ C di mayoritas suite; tidak kalah >5% di satupun (metodologi terdokumentasi)
2. Safety: 0 known soundness holes; memory-safe tanpa GC; data-race free
3. Self-hosted compiler yang mem-bootstrap dirinya secara reproducible
4. Minimal 4 domain terbukti: CLI tools, web (WASM), embedded (hardware asli), mini-OS
5. SemVer guarantee + edition stability
6. Dokumentasi lengkap: book, stdlib docs, spec, tutorial

---

## Prinsip Kerja (berlaku setiap versi)

1. **Test dulu, fitur kemudian** — e2e test positif & negatif untuk tiap fitur.
2. **Tidak ada regresi** — full suite + examples sweep hijau sebelum commit.
3. **Benchmark setiap perubahan optimizer** — performa itu ukuran, bukan opini.
4. **Keamanan tidak dikompromikan demi fitur** — borrow checker adalah suci.
5. **Dokumentasi ikut koding** — README/spec update di commit yang sama.
6. **Riset ulang tiap milestone besar** — ekosistem 2026 terbukti cepat berubah
   (Mojo open-source, WASI 0.3, Polonius nightly — semua kejadian tahun ini).

## Sumber publik utama (diakses Agustus 2026)

- InfoWorld / i-programmer.info: Polonius Alpha borrow checker di Rust nightly (Agustus 2026)
- platform.uno & Bytecode Alliance: State of WebAssembly, WASI 0.3, Component Model 1.0 roadmap
- linuxiac.com: Mojo 1.0 dan pembukaan source compiler-nya (Agustus 2026)
- r/ProgrammingLanguages: pengumuman Odin 1.0 (2026)
- matklad.github.io: "Against Query Based Compilers" (Februari 2026)
- rust-osdev.com & redox-os.org: ekosistem OS development ber-Rust (2026)
- blog.rust-embedded.org: embedded-hal v1.0 dan ekosistem embedded
- arXiv 2607.01504: Kani model checker untuk Rust (Juli 2026)
- Phoronix: rilis LLVM 23.1 dan GCC 16 (2026)
