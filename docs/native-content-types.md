# Native Content Types: Image, Audio, and Link

Three new types — `Image`, `Audio`, and `Link` — are being added to the SA prelude. They give the language native vocabulary for the content kinds that both agent protocols and LLM APIs already understand, closing the gap between SA's type system and the wire formats it must produce and consume.

## The Gap These Types Fill

SA's current prelude covers `String`, `Int`, `Boolean`, `Unit`, `List<T>`, and `Option<T>`. These are sufficient for data-processing agents, but modern LLM APIs and agent communication protocols define a richer set of content primitives. Both ACP ([agent-client-protocol-schema](https://docs.rs/agent-client-protocol-schema/0.11.4/agent_client_protocol_schema/content/enum.ContentBlock.html)) and MCP ([rmcp](https://docs.rs/rmcp/0.15.0/rmcp/model/content/enum.RawContent.html)) enumerate content blocks including text, image, audio, and resource links. Gemini and OpenAI's input APIs accept inline image and audio data natively. Without types to represent these, SA programs receiving multimodal tool results have nowhere to put them, and the `!` operator — which already routes values into LLM conversation context — has no way to handle non-text content correctly at the type level.

The `!` operator is already multimodal in its intent. Making it correctly multimodal in practice requires these three types.

## The Three Types

`Image` and `Audio` each hold a MIME type and raw binary data. From an SA author's perspective they are opaque: there is no language-level facility to inspect or construct their content. They arrive as return values from `extern fn` calls or as results from MCP and ACP tools, and they are passed onward to the LLM via `!`. The binary data is an implementation detail; `Image` and `Audio` exist as distinct named types because they carry semantic meaning. A value typed `Image` tells the integration layer to encode it as a Gemini `inline_data` part or an OpenAI `image_url` part, not as a text string.

`Link` holds a URI and an optional name. It maps to ACP's `ResourceLink` and MCP's `RawResource`. A `Link` is a reference: the receiver resolves it, it is not inline content. This distinction matters for integration. A `Link` passed to an LLM renders as a URI string in a text part; an `Image` renders as encoded binary inline data. Conflating the two would require the integration layer to inspect content at runtime to determine handling — exactly the kind of ambiguity a type system should eliminate.

## What Was Considered and Rejected

A `Bytes` primitive was considered and rejected. Opaque binary data with no associated semantics has no meaningful use in a language built around language model integration. An `Image` knows it is an image; bare bytes know nothing. The MIME type and the semantic type name together are what allow correct dispatch through integration layers. Having `Bytes` in the prelude would invite its misuse wherever `Image` or `Audio` is correct.

`EmbeddedResource` — defined by both ACP and MCP as a protocol convenience type for sending fetched file content inline — was also considered and rejected. It wraps either text content (a URI plus a string) or blob content (a URI plus base64-encoded binary). The URI is provenance metadata, not a reference to be resolved. At the SA level, the text variant is a `String` and the blob variant is an `Image` or `Audio`. There is no SA concept that maps cleanly to the union: it is a protocol-level optimisation that avoids extra round-trips, not a semantic type. See [rmcp ResourceContents](https://docs.rs/rmcp/0.15.0/rmcp/model/enum.ResourceContents.html) for the definition.

URL or URI as an alternative representation for `Image` and `Audio` data was also considered. The decision was against it. `Image` and `Audio` hold inline binary data, not references to data. A link to an image is a `Link`, not an `Image`. Allowing either form within a single type would require every integration target to handle two cases and would obscure the difference between retrievable references and self-contained content.

## Rust Representation

The three types follow the `RuntimeValue` pattern established in `structured-agent/src/structured-agent-runtime/src/runtime_value/primitives.rs`. Each is a Rust struct implementing the `RuntimeValue` trait, defined in `structured-agent/src/structured-agent-runtime/src/runtime_value/mod.rs`, which requires `type_name()`, `format_for_llm()`, `as_any()`, `eq()`, and `to_arrow()`. The implementations live in a new file, `structured-agent/src/structured-agent-runtime/src/runtime_value/media.rs`.

For Arrow serialization, `Image` and `Audio` are represented as `StructArray` with fields `mime_type: Utf8` and `data: Binary`. `Link` is a `StructArray` with `uri: Utf8` and `name: Utf8` (nullable, corresponding to `Option<String>`). The `arrow_col_to_expression` function in `structured-agent/src/structured-agent-runtime/src/runtime_value/mod.rs` and `type_to_arrow_datatype` in `structured-agent/src/structured-agent-runtime/src/expression.rs` are extended to recognise these schemas.

`Type::image()`, `Type::audio()`, and `Type::link()` constructor helpers follow the pattern of `Type::string()` and `Type::boolean()` in `structured-agent/src/structured-agent-runtime/src/types.rs`. Corresponding `ExpressionValue::image()`, `::audio()`, and `::link()` constructors and `as_image()`, `as_audio()`, `as_link()` accessors follow the pattern in `structured-agent/src/structured-agent-runtime/src/expression.rs`.

## Integration Mappings

The three types map to wire formats differently across each integration target.

| SA Type | ACP | MCP | Gemini | OpenAI |
|---------|-----|-----|--------|--------|
| `Image` | `ContentBlock::Image` | `RawContent::Image` | `Part.inline_data` (base64 + mime) | `image_url` content part (base64 data URL) |
| `Audio` | `ContentBlock::Audio` | `RawContent::Audio` | `Part.inline_data` (base64 + mime) | `input_audio` content part |
| `Link`  | `ContentBlock::ResourceLink` | `RawContent::ResourceLink` | `Part.file_data` (`file_uri` + mime) | text content part (URI string) |

For Gemini (`structured-agent/src/structured-agent-gemini/src/types.rs`), `Image` and `Audio` become inline `Part` objects carrying `inline_data` with base64-encoded content and the MIME type string. `Link` becomes a `file_data` part carrying the URI as `file_uri` alongside a MIME type.

For OpenAI, `Image` becomes an `image_url` content part using a base64 data URL of the form `data:<mime>;base64,<data>`. `Audio` becomes an audio input part. `Link` renders as text.

For ACP, `Image` maps to `ContentBlock::Image`, `Audio` to `ContentBlock::Audio`, and `Link` to `ContentBlock::ResourceLink`. Inbound conversion — from ACP content blocks arriving at the server into `ExpressionValue` — handles the reverse direction. The mapping lives in `structured-agent/src/structured-agent/src/acp/server.rs`. MCP follows the same pattern using rmcp's `RawContent` enum variants.

## Planned Work

The work divides into six independent pieces. All four integration pieces depend on the runtime layer being in place, but the Gemini, OpenAI, and ACP/MCP pieces are independent of each other.

The first piece is the `RuntimeValue` implementations: `ImageValue`, `AudioValue`, and `LinkValue` in the new `media.rs` file. Tests cover `type_name()` output, equality across same value, differing MIME type, and differing data, `format_for_llm()` output for each type, and the optional `name` field on `LinkValue`.

The second piece adds the `Type` helpers and `ExpressionValue` constructors and accessors. Tests cover constructor return values, `is_image`, `is_audio`, and `is_link` predicates, round-trip through `as_image`, `as_audio`, and `as_link`, and failure cases for type mismatches.

The third piece covers the Arrow layer: `to_arrow()` implementations, schema correctness for each struct type, nullability of `name` on `Link`, and round-trip through `arrow_col_to_expression`.

The fourth, fifth, and sixth pieces are the Gemini, OpenAI, and ACP/MCP integration mappings respectively. Each is independent of the others. Integration tests cover serialization of each type to the wire format; for ACP and MCP, tests additionally cover the inbound conversion direction.

## References

- ACP content block types: https://docs.rs/agent-client-protocol-schema/0.11.4/agent_client_protocol_schema/content/enum.ContentBlock.html
- MCP `RawContent`: https://docs.rs/rmcp/0.15.0/rmcp/model/content/enum.RawContent.html
- MCP `ResourceContents`: https://docs.rs/rmcp/0.15.0/rmcp/model/enum.ResourceContents.html
- `RuntimeValue` trait: `structured-agent/src/structured-agent-runtime/src/runtime_value/mod.rs`
- Existing primitive implementations: `structured-agent/src/structured-agent-runtime/src/runtime_value/primitives.rs`
- Type helpers: `structured-agent/src/structured-agent-runtime/src/types.rs`
- `ExpressionValue` constructors and accessors: `structured-agent/src/structured-agent-runtime/src/expression.rs`
- ACP server integration: `structured-agent/src/structured-agent/src/acp/server.rs`
- Gemini wire types: `structured-agent/src/structured-agent-gemini/src/types.rs`
