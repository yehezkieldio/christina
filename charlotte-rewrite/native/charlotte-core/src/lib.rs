//! FFI boundary for Charlotte's native core, loaded from `packages/native-core`
//! through `bun:ffi`. Every exported function takes plain arguments and
//! returns plain data, per `02-native-core-and-ffi.md`'s boundary contract:
//! primitives cross directly, structured results cross as a heap-allocated
//! JSON C string that the caller must release through
//! [`charlotte_core_free_string`].
//!
//! This file is deliberately thin: it is the only place in the crate that
//! touches raw pointers or `extern "C"`. Every other module (`git`,
//! `binary`, `chunking`, `tokenizer`) is plain, safe Rust with no FFI
//! awareness, so its own tests need no pointer juggling.

mod binary;
mod chunking;
mod git;
mod tokenizer;

use std::ffi::{CStr, CString, c_char};

use serde::Serialize;

/// Phase 0's proof that `bun:ffi` can call into this crate at all
/// (`11-build-phases.md`). Kept as a minimal, dependency-free smoke test
/// alongside the real exports below. `wrapping_add` rather than `+`: this
/// function has no invariant to protect (it is a connectivity smoke test,
/// not a real computation), so wrapping is simpler than a panic path.
#[allow(unsafe_code, reason = "FFI export requires #[unsafe(no_mangle)] on an extern \"C\" fn; every unsafe site in this file is reviewed individually rather than covered by a crate-wide allow")]
#[unsafe(no_mangle)]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}

/// Releases a string returned by any function in this crate. Every
/// heap-allocated `*mut c_char` this crate hands to `bun:ffi` must come
/// back through this function exactly once; calling it twice on the same
/// pointer, or on a pointer this crate did not allocate, is undefined
/// behavior — that contract lives on the TypeScript side in
/// `packages/native-core`, which wraps every call site.
///
/// # Safety
///
/// `ptr` must be null, or a pointer this crate previously returned from a
/// JSON-producing export, not yet passed to this function before.
#[allow(unsafe_code, reason = "FFI export requires #[unsafe(no_mangle)] on an extern \"C\" fn")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn charlotte_core_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    #[allow(unsafe_code, reason = "reconstructing the CString this crate allocated is how its memory gets freed")]
    // SAFETY: `ptr` is non-null here, and the function's own safety contract
    // requires it to be a pointer this crate previously returned via
    // `CString::into_raw`, not yet freed.
    drop(unsafe { CString::from_raw(ptr) });
}

fn borrow_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    #[allow(unsafe_code, reason = "every caller of borrow_str already documents the pointer-validity contract on its own # Safety section")]
    // SAFETY: `ptr` is non-null here; the caller's `# Safety` section
    // requires it to be a valid pointer to a null-terminated C string for
    // the duration of the call.
    let c_str = unsafe { CStr::from_ptr(ptr) };
    c_str.to_str().ok()
}

#[allow(clippy::expect_used, reason = "both expects here guard a genuine invariant violation (a serialization bug or an impossible embedded NUL), not a recoverable runtime failure")]
fn to_json_ptr<T: Serialize>(value: &T) -> *mut c_char {
    // serde_json never emits a raw NUL byte (control characters are
    // escaped as `\u0000`), so this can only fail on a serialization bug.
    let json = serde_json::to_string(value).expect("value must serialize to JSON");
    CString::new(json).expect("JSON output must not contain a raw NUL byte").into_raw()
}

#[derive(Serialize)]
struct ErrorResponse<'a> {
    error: &'a str,
}

fn to_error_ptr(message: &str) -> *mut c_char {
    to_json_ptr(&ErrorResponse { error: message })
}

/// `read_staged_diff(repo_path: string) -> { diff: string, files: string[] }`
///
/// # Safety
///
/// `repo_path` must be null, or a valid pointer to a null-terminated C
/// string that stays valid for the duration of this call.
#[allow(unsafe_code, reason = "FFI export requires #[unsafe(no_mangle)] on an extern \"C\" fn")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn read_staged_diff(repo_path: *const c_char) -> *mut c_char {
    let Some(repo_path) = borrow_str(repo_path) else {
        return to_error_ptr("repo_path must be a valid UTF-8 string");
    };
    match git::read_staged_diff(repo_path) {
        Ok(staged) => to_json_ptr(&staged),
        Err(err) => to_error_ptr(&err.to_string()),
    }
}

/// `read_commit_history(repo_path: string, depth: number) -> { sha, subject }[]`
///
/// # Safety
///
/// `repo_path` must be null, or a valid pointer to a null-terminated C
/// string that stays valid for the duration of this call.
#[allow(unsafe_code, reason = "FFI export requires #[unsafe(no_mangle)] on an extern \"C\" fn")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn read_commit_history(repo_path: *const c_char, depth: u32) -> *mut c_char {
    let Some(repo_path) = borrow_str(repo_path) else {
        return to_error_ptr("repo_path must be a valid UTF-8 string");
    };
    match git::read_commit_history(repo_path, depth) {
        Ok(commits) => to_json_ptr(&commits),
        Err(err) => to_error_ptr(&err.to_string()),
    }
}

/// `is_binary_content(bytes: Uint8Array, path: string) -> boolean`. The
/// TypeScript wrapper in `packages/native-core` passes the buffer as a
/// pointer plus an explicit length, since `bun:ffi` does not carry a typed
/// array's length across the boundary on its own.
///
/// # Safety
///
/// `path` must be null, or a valid pointer to a null-terminated C string.
/// `bytes_ptr` must be null, or a valid pointer to at least `bytes_len`
/// readable bytes; both must stay valid for the duration of this call.
#[allow(unsafe_code, reason = "FFI export requires #[unsafe(no_mangle)] on an extern \"C\" fn")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn is_binary_content(bytes_ptr: *const u8, bytes_len: u64, path: *const c_char) -> bool {
    let Some(path) = borrow_str(path) else {
        return false;
    };
    if bytes_ptr.is_null() || bytes_len == 0 {
        return binary::is_binary_content(&[], path);
    }
    // A `bytes_len` too large for this platform's `usize` cannot be a valid
    // buffer length (nothing this large fits in addressable memory), so
    // treat it the same as a null buffer rather than truncating it.
    let Ok(bytes_len) = usize::try_from(bytes_len) else {
        return binary::is_binary_content(&[], path);
    };
    #[allow(unsafe_code, reason = "the function's own # Safety section requires bytes_ptr to point to at least bytes_len valid, readable bytes")]
    // SAFETY: `bytes_ptr` is non-null here, and the function's safety
    // contract requires it to point to at least `bytes_len` readable bytes
    // for the duration of this call.
    let bytes = unsafe { std::slice::from_raw_parts(bytes_ptr, bytes_len) };
    binary::is_binary_content(bytes, path)
}

/// `count_tokens(text: string) -> number`
///
/// # Safety
///
/// `text` must be null, or a valid pointer to a null-terminated C string
/// that stays valid for the duration of this call.
#[allow(unsafe_code, reason = "FFI export requires #[unsafe(no_mangle)] on an extern \"C\" fn")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn count_tokens(text: *const c_char) -> u32 {
    let Some(text) = borrow_str(text) else {
        return 0;
    };
    tokenizer::count_tokens(text)
}

/// `chunk_diff(diff: string, token_limit: number, lockfile_token_limit: number) -> Chunk[]`
///
/// # Safety
///
/// `diff` must be null, or a valid pointer to a null-terminated C string
/// that stays valid for the duration of this call.
#[allow(unsafe_code, reason = "FFI export requires #[unsafe(no_mangle)] on an extern \"C\" fn")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chunk_diff(diff: *const c_char, token_limit: u32, lockfile_token_limit: u32) -> *mut c_char {
    let Some(diff) = borrow_str(diff) else {
        return to_error_ptr("diff must be a valid UTF-8 string");
    };
    let chunks = chunking::chunk_diff(diff, token_limit, lockfile_token_limit);
    to_json_ptr(&chunks)
}
