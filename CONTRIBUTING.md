# Contributing to rustmetaverse

Thanks for your interest in contributing to rustmetaverse. This project is a
native Rust reimplementation of the Second Life / OpenSimulator virtual-world
protocol.

## Getting started

```sh
git clone https://github.com/cinderblocks/rustmetaverse.git
cd rustmetaverse
cargo build --workspace
cargo test --workspace
```

## Development workflow

1. Fork the repository and create a feature branch.
2. Make your change. Keep the public API stable unless you have discussed the
   change in an issue first.
3. Ensure the following pass before opening a pull request:

   ```sh
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo build --workspace
   cargo test --workspace
   cargo doc --no-deps --workspace
   ```

4. Write commit messages in English, using
   [Conventional Commits](https://www.conventionalcommits.org/) when possible.
5. Open a pull request with a clear description of what changed and why.

## Code style

- Run `cargo fmt` before committing. The `rustfmt.toml` at the repository root
  defines the project's formatting rules.
- Clippy must pass with no warnings.
- All new public types and functions should have doc comments.
- Comments, documentation, and commit messages must be in English.

## Project structure

| Crate | Responsibility |
|-------|---------------|
| `rustmetaverse_types` | Foundational types: UUID, Vector3, Quaternion, math helpers |
| `rustmetaverse_structured_data` | LLSD (Linden Lab Structured Data) XML serialization |
| `rustmetaverse_protocol` | LLUDP wire format: packet header, zero-coding, packet definitions |
| `rustmetaverse` | Client orchestration: login, networking, packet dispatch, session state |

## Reporting issues

Open a GitHub issue with a clear title and description. Include:
- What you expected to happen.
- What actually happened.
- Steps to reproduce, including the grid/region if relevant.
- Rust version and platform.

## License

By contributing, you agree that your contributions will be licensed under the
BSD 3-Clause license that covers the project.
