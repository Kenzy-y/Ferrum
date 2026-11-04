# Ferrum

A native IDE for Rust, written in Rust: the feature set of a modern editor with
lower latency, a smaller memory footprint and near-zero CPU usage when idle.

> "Ferrum" is a working title. The product name is defined once in the
> `app_identity` crate.

The project is at an early stage; see [`ROADMAP.md`](ROADMAP.md) for the plan
and [`AGENTS.md`](AGENTS.md) for the engineering rules.

## Building

Requires the Rust toolchain pinned in `rust-toolchain.toml` (installed
automatically by `rustup`).

```sh
cargo run            # debug build of the IDE
cargo test --workspace
```

On Linux, GPUI needs the system development packages for X11/Wayland, Vulkan
and fonts; see `.github/workflows/ci.yml` for the exact list.

Logging is controlled by the `RUST_LOG` environment variable, for example
`RUST_LOG=debug cargo run`.

## Third-party assets

The binary embeds these assets; their license files are next to them:

- [Inter](https://github.com/rsms/inter) 4.1 — SIL Open Font License 1.1,
  `src/design/theme/fonts/inter/OFL.txt`.
- [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono) 2.304 — SIL Open
  Font License 1.1, `src/design/theme/fonts/jetbrains-mono/OFL.txt`.
- [Lucide](https://lucide.dev) icons 1.60.0 — ISC (some icons MIT, from
  Feather), `src/design/ui/icons/lucide/LICENSE`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
