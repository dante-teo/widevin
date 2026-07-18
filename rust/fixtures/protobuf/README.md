# Cross-language protobuf fixtures

- `typescript-models-response.base64` was encoded with the committed
  `@bufbuild/protobuf` TypeScript schema. Rust tests decode it with Prost.
- `rust-models-request.base64` was encoded with the committed minimal Prost
  schema. TypeScript tests decode it with `@bufbuild/protobuf`.

Base64 keeps the fixtures reviewable while preserving the exact binary wire
payload after decoding.
