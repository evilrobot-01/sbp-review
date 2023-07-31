# SBP-Review PoC
A quick proof-of-concept to assist with SBP reviews.

```shell
cargo install --git https://github.com/evilrobot-01/sbp-review
```
Alternatively clone the repo and then install locally:
```shell
cargo install --path .
```
 
## Usage

### Code
Uses `cargo clippy` lints to highlight potential issues in code (e.g. unsafe math, unwraps, function length).
```shell
sbp-review code
```
Note: ctrl-clicking on the mentioned source location within the resulting output should take you directly to the offending code.

### Manifests
Basic manifest inspection using `cargo metadata`. Useful for checking for missing manifest attributes and for validating supported versions of Substrate, Cumulus, Polkadot.
```shell
sbp-review manifest
```
Note: ctrl-clicking on the manifest name within the resulting output should take you directly to the `cargo.toml` file.

### Tests
Simply runs `cargo test` for a workspace.
```shell
sbp-review tests
```

### Benchmarks
Simply runs `cargo test` for a workspace with the `runtime-benchmarks` feature enabled.
```shell
sbp-review benchmarks
```