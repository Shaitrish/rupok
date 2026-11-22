# Licensing

Rupok uses two licenses: a copyleft one for the app and a permissive one for
everything other software needs to interoperate with it.

| Part | License | File |
|---|---|---|
| Everything not listed below: the app, the engine crates (`rupok-ink`, `rupok-core`, `rupok-render`, `rupok-wasm`), the CLI and MCP server (`rupok-cli`), `bench/`, `apps/`, docs, examples | **AGPL-3.0-or-later** | [`LICENSE`](LICENSE) |
| `spec/`: the `.rpk` format and Smart Object specs | **Apache-2.0** | [`spec/LICENSE`](spec/LICENSE) |
| `crates/rupok-format`: the reference reader, writer and validator | **Apache-2.0** | [`crates/rupok-format/LICENSE`](crates/rupok-format/LICENSE) |
| `packages/object-sdk`: the in-object SDK (`@rupok/object-sdk`) | **Apache-2.0** | [`packages/object-sdk/LICENSE`](packages/object-sdk/LICENSE) |
| `skills/`: instructions and skills for AI agents | **Apache-2.0** | [`skills/LICENSE`](skills/LICENSE) |

A file's own license header, or a `LICENSE` file in its directory, takes
precedence over this table.

## Why the split

- **The app is AGPL** so improvements to Rupok stay open, including when
  someone runs a modified Rupok as a network service (for example a hosted
  board or a broadcast server, under AGPL section 13).
- **The format, SDK and skills are Apache-2.0** so anyone can build
  compatible software without copyleft obligations: a converter, a viewer, an
  LMS plugin, an AI agent that writes lessons, or a Smart Object that embeds
  the SDK. Open formats matter more than any one app. Teachers should never be
  locked in.

## Your content is yours

The AGPL covers Rupok's code, not what you make with it. Your lessons
(`.rpk` files), your ink and the Smart Objects you or your AI agent write are
your own work, under whatever terms you choose. The SDK that the host injects
into Smart Objects is Apache-2.0, so it adds no copyleft obligations to your
objects.

## Contributions

Contributions are accepted under the license of the part they change
("inbound = outbound"), with a Developer Certificate of Origin sign-off on
every commit. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Third-party material

Bundled fonts, icons and libraries keep their own licenses; see
[`NOTICE`](NOTICE). Rust and JavaScript dependencies are listed in
`Cargo.lock` and `apps/board/bun.lock`.
