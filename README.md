# Slide

Slide is a personal Windows/macOS presentation app maintained in
[JeongEEE/deckcraft](https://github.com/JeongEEE/deckcraft), based on
[DeckCraft by the ArtCraft team](https://github.com/storytold/deckcraft).

Rust workspace package names, engine commands and `DECKCRAFT_*` environment variables remain
compatible with upstream. The installed app, preferences and update feed are independent of
DeckCraft.

- `main`: unmodified upstream history; synchronize from `upstream/main`.
- `my-custom`: development and personal changes.
- [Upstream architecture](docs/architecture.md) and [known limitations](docs/gaps.md).

## Build locally

Install Rust 1.95 or later and the platform's native build tools, then run:

```sh
cargo build --release --locked -p deckcraft -p deckcraft-cli
cargo run --release -p deckcraft -- --sample
cargo xtask ci
```

## License and attribution

Code is available under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your choice.
Copyright (c) 2026 ArtCraft Team and the DeckCraft contributors. See [NOTICE](NOTICE)
and [ATTRIBUTION.md](ATTRIBUTION.md) for third-party notices and the original app icon.
ArtCraft logos and wordmarks have been removed from this modified distribution, as the
[brand terms](docs/brand/LICENSE-brand.txt) require of forks.
Slide is an independent personal fork and is not an official ArtCraft product.
