# Temporary upstream security compatibility patch

Source: https://crates.io/crates/utoipa-swagger-ui/9.0.2
Upstream: https://github.com/juhaku/utoipa
Upstream commit: `85952757ee50632be4b84e6580f16530397808d1`
Published crate SHA-256: `d047458f1b5b65237c2f6dc6db136945667f40a7668627b3490b9513a3d43a55`

The public API, Swagger assets and optional features are unchanged.
The normalized crates.io manifest and upstream source/licenses are retained.
Changes: require patched `zip >=9.0.0` (RUSTSEC-2026-0367/0368/0369),
adapt the fallible ZIP filename accessor, and mask extracted permissions to ordinary `0o777` bits.
The upstream 10.0.1 release still requires zip 8.6 and also changes the OpenAPI
major version; retaining 9.0.2 avoids an unrelated API migration.
Remove this patch when a compatible upstream release accepts patched zip.

Repository formatter and whitespace normalization are also applied.
