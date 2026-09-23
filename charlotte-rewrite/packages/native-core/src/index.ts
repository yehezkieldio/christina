import { CString, dlopen, FFIType, type Pointer, ptr } from "bun:ffi";
import { nativeCoreLibraryPath } from "./library-path";

const { symbols } = dlopen(nativeCoreLibraryPath, {
  add: {
    args: [FFIType.i32, FFIType.i32],
    returns: FFIType.i32,
  },
  charlotte_core_free_string: {
    args: [FFIType.ptr],
    returns: FFIType.void,
  },
  read_staged_diff: {
    args: [FFIType.cstring],
    returns: FFIType.ptr,
  },
  read_commit_history: {
    args: [FFIType.cstring, FFIType.u32],
    returns: FFIType.ptr,
  },
  is_binary_content: {
    args: [FFIType.ptr, FFIType.u64, FFIType.cstring],
    returns: FFIType.bool,
  },
  count_tokens: {
    args: [FFIType.cstring],
    returns: FFIType.u32,
  },
  chunk_diff: {
    args: [FFIType.cstring, FFIType.u32, FFIType.u32],
    returns: FFIType.ptr,
  },
});

export function add(a: number, b: number): number {
  return symbols.add(a, b);
}

export class NativeCoreError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "NativeCoreError";
  }
}

type NativeResult<T> = T | { readonly error: string };

/** `bun:ffi` types a `ptr`-returning symbol as `Pointer | bigint | null`: a
 * 64-bit address that overflows a safe JS number comes back as `bigint`
 * instead of silently losing precision. Every real address from this
 * crate's allocator fits in a JS number in practice, so normalizing to
 * `Pointer` here is safe. */
function assertPointer(raw: Pointer | bigint | null, errorMessage: string): Pointer {
  if (raw === null) {
    throw new NativeCoreError(errorMessage);
  }
  return (typeof raw === "bigint" ? Number(raw) : raw) as Pointer;
}

/** Every JSON-returning native call follows this shape: clone the string
 * out of native memory, free the native allocation exactly once, then
 * parse. `charlotte_core_free_string` must run even when parsing throws. */
function readJson<T>(rawPtr: Pointer): T {
  try {
    const text = new CString(rawPtr);
    const parsed = JSON.parse(text as unknown as string) as NativeResult<T>;
    if (parsed !== null && typeof parsed === "object" && "error" in parsed) {
      throw new NativeCoreError(parsed.error);
    }
    return parsed;
  } finally {
    symbols.charlotte_core_free_string(rawPtr);
  }
}

export interface StagedDiff {
  readonly diff: string;
  readonly files: readonly string[];
}

export function readStagedDiff(repoPath: string): StagedDiff {
  const rawPtr = assertPointer(symbols.read_staged_diff(repoPath), "read_staged_diff returned a null pointer");
  return readJson<StagedDiff>(rawPtr);
}

export interface CommitSummary {
  readonly sha: string;
  readonly subject: string;
}

export function readCommitHistory(repoPath: string, depth: number): CommitSummary[] {
  const rawPtr = assertPointer(symbols.read_commit_history(repoPath, depth), "read_commit_history returned a null pointer");
  return readJson<CommitSummary[]>(rawPtr);
}

export function isBinaryContent(bytes: Uint8Array, path: string): boolean {
  const bufferPtr = bytes.length === 0 ? null : ptr(bytes);
  return symbols.is_binary_content(bufferPtr, BigInt(bytes.length), path);
}

export function countTokens(text: string): number {
  return symbols.count_tokens(text);
}

export interface Chunk {
  readonly content: string;
  readonly filePaths: readonly string[];
}

export function chunkDiff(diff: string, tokenLimit: number, lockfileTokenLimit: number): Chunk[] {
  const rawPtr = assertPointer(symbols.chunk_diff(diff, tokenLimit, lockfileTokenLimit), "chunk_diff returned a null pointer");
  return readJson<Chunk[]>(rawPtr);
}
