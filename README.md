# SBP-Review PoC
A quick proof-of-concept to assist with SBP reviews.

```shell
cargo install --git https://github.com/evilrobot-01/sbp-review
```
 
## Usage
### Code
Uses `cargo clippy` lints to highlight potential issues in code (e.g. unsafe math, unwraps, function length).
```shell
sbp-review code
```
Note: ctrl-clicking on the mentioned source location within the resulting output should take you directly to the offending code.
### Manifest
Basic manifest inspection using `cargo metadata`. Useful for checking for missing manifest attributes and for validating supported versions of Substrate, Cumulus, Polkadot.
```shell
sbp-review manifest
```
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