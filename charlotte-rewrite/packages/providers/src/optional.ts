/**
 * `exactOptionalPropertyTypes` treats `{ key: undefined }` as distinct from
 * omitting `key` entirely, but most external option types (the AI SDK's
 * provider settings, this package's own `GenerateStructuredOptions`) only
 * declare `key?: V`, not `key?: V | undefined`. Spreading `optional(key,
 * value)` into an object literal supplies the key only when `value` is
 * actually present, so an absent optional field never becomes an explicit
 * `undefined` that the stricter target type rejects.
 */
// `Partial<Record<K, V>>` fails to accept `{}` when `K` is an unresolved
// generic (a known TS quirk); the mapped-type form below is what actually
// typechecks under `exactOptionalPropertyTypes`.
// oxlint-disable typescript/consistent-indexed-object-style
export const optional = <K extends string, V>(key: K, value: V | undefined): { readonly [P in K]?: V } =>
  value === undefined ? {} : ({ [key]: value } as { readonly [P in K]: V });
// oxlint-enable typescript/consistent-indexed-object-style
