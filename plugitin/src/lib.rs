pub fn greeting() -> &'static str {
    "Hello world!"
}

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "host")]
pub mod host;

/// WASM can't return tuples yet so this function packs a (pointer, length) pair of u32s
/// into a single u64 which can be returned as a unit. The pointer is stored in the lower
/// 32 bits and the length is stored in the higher 32 bits. See unpack_buffer_desc for the
/// complementary unpacking operation.
pub(crate) fn pack_buffer_desc(ptr: u32, len: u32) -> u64 {
    (ptr as u64) | ((len as u64) << 32)
}

/// Unpacks a (pointer, length) pair of u32s representing a buffer descriptor from a
/// packed u64. The u64 must have been packed by pack_buffer_desc previously.
pub(crate) fn unpack_buffer_desc(packed: u64) -> (u32, u32) {
    let ptr = packed as u32;
    let len = (packed >> 32) as u32;
    (ptr, len)
}