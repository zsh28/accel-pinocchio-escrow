# Pinocchio Escrow (v1 unsafe + v2 wincode)

This project implements a Solana escrow program with two side-by-side instruction families:

- `unsafe` (v1): manual instruction decoding and direct account data loading
- `wincode` (v2): wincode-based instruction/state deserialization

Both flows run the same escrow lifecycle:

1. `make` (maker deposits token A in vault)
2. `take` (taker pays token B to maker and receives token A)
3. `cancel` (maker reclaims token A from vault)

The program is built with Pinocchio and tested with LiteSVM.

## Program layout

- `src/lib.rs`: program entrypoint and instruction routing
- `src/state/escrow.rs`: escrow account layout and loaders
- `src/instructions/unsafe/*`: v1 instruction handlers
- `src/instructions/wincode/*`: v2 instruction handlers
- `src/tests/mod.rs`: LiteSVM functional tests and CU benchmarking

## Instruction discriminators

### v1 (`unsafe`)

- `0`: `Make`
- `1`: `Take`
- `2`: `Cancel`

### v2 (`wincode`)

- `3`: `MakeV2`
- `4`: `TakeV2`
- `5`: `CancelV2`

## Account model

Escrow PDA is derived as:

`["escrow", maker_pubkey, bump]`

Vault account is the ATA for:

- owner: escrow PDA
- mint: mint A

## State layout

Escrow account fields (`src/state/escrow.rs`):

- `maker: [u8; 32]`
- `mint_a: [u8; 32]`
- `mint_b: [u8; 32]`
- `amount_to_receive: [u8; 8]`
- `amount_to_give: [u8; 8]`
- `bump: u8`
- `_padding: [u8; 7]`

`Escrow::LEN` is `core::mem::size_of::<Escrow>()` to keep allocation consistent with actual memory layout.

## v1 (`unsafe`) details

The v1 path intentionally uses low-level parsing patterns:

- Instruction data is read manually from byte slices
- Escrow account data is loaded with alignment checks + casts
- Unsafe account borrows are scoped tightly before CPI calls

Unsafe borrows are used in:

- `src/instructions/unsafe/make.rs`
- `src/instructions/unsafe/take.rs`
- `src/instructions/unsafe/cancel.rs`

These borrows are dropped before CPI calls to avoid runtime `AccountBorrowFailed` errors.

## v2 (`wincode`) details

The v2 path keeps the same escrow behavior but changes serialization:

- `MakeV2` instruction payload uses `wincode::deserialize`
- `take_v2` and `cancel_v2` deserialize escrow data using `wincode`
- Account checks, signer checks, PDA checks, token transfers, and close flows remain equivalent to v1

`MakeV2` payload schema:

- `amount_to_receive: u64`
- `amount_to_give: u64`
- `bump: u8`

## Build and test

### 1) Build host artifacts

```bash
cargo test --no-run
```

### 2) Build SBF program (`.so`) for LiteSVM loading

```bash
cargo build-sbf
```

### 3) Run tests

```bash
cargo test
```

The LiteSVM loader in tests searches for the program binary in:

- `target/sbpf-solana-solana/release/escrow.so`
- `target/sbf-solana-solana/release/escrow.so`
- `target/deploy/escrow.so`

## Benchmarking (inside tests)

The test `benchmark_compute_units_v1_vs_v2` runs make/take/cancel for both versions and prints a CU table.

Run it with output visible:

```bash
cargo test benchmark_compute_units_v1_vs_v2 -- --nocapture
```

Actual output from this repo:

```text
Instruction |      v1 CU |      v2 CU
-----------+------------+-----------
make       |      31974 |      30442
take       |      16643 |      16662
cancel     |      10599 |      10619
```

## Security and correctness checks implemented

- Program ID assertion at entrypoint
- Required signer validation (`maker` or `taker`)
- PDA derivation validation for escrow account
- Token account owner/mint validation before settlement
- Vault close after settlement/cancel
- Escrow lamports reclaimed to maker and escrow lamports set to zero
