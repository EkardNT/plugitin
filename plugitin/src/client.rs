//! Code used by plugins.
//!
//! # Features
//! This module is only available if the **client** feature is enabled.

use std::alloc::Layout;
use std::marker::PhantomData;
use std::mem::size_of;

use crate::{pack_buffer_desc, unpack_buffer_desc};

use bincode::{deserialize_from, serialize_into, serialized_size};
use serde::{Deserialize, Serialize};

/// Declares a client plugin. Takes a single argument, the name of the plugin type.
///
/// # Features
/// Only available if the **client** feature is enabled.
///
/// # Examples
///
/// ```
/// plugin!(MyPlugin);
///
/// struct MyPlugin;
///
/// impl Plugin for MyPlugin {
///     fn new() -> Self {
///         MyPlugin {}
///     }
/// }
#[macro_export]
macro_rules! plugin {
    ($name:ty) => {
        #[no_mangle]
        fn plugitin_init() -> u32 {
            $crate::client::plugitin_init_impl::<$name>()
        }

        #[no_mangle]
        fn plugitin_destroy(info: u32) {
            $crate::client::plugitin_destroy_impl::<$name>(info)
        }

        #[no_mangle]
        fn plugitin_alloc(info: u32, size: u32, align: u32) -> u32 {
            $crate::client::plugitin_alloc_impl::<$name>(info, size, align)
        }

        #[no_mangle]
        fn plugitin_dealloc(info: u32, ptr: u32, size: u32, align: u32) {
            $crate::client::plugitin_dealloc_impl::<$name>(info, ptr, size, align)
        }

        #[no_mangle]
        fn plugitin_client_call(info: u32, input_packed: u64) -> u64 {
            $crate::client::plugitin_client_call_impl::<$name>(info, input_packed)
        }
    }
}

// Entry point to the plugin. Returns an opaque data pointer which will be passed
// unchanged as an argument to all further plugin calls.
#[doc(hidden)]
pub fn plugitin_init_impl<P: Plugin>() -> u32 {
    // It is impossible to know up front the maximum serialized size that input/outputs
    // will take, due to the possibility of types arbitrarily amplifying their serialized
    // sizes (see https://github.com/servo/bincode/issues/291). Therefore we need to
    // support buffer resizing. I set the initial size of the buffers to 0 so that
    // resizing logic is always invoked, giving less space for bugs to hide in resizing
    // code that might otherwise be infrequently called.
    Box::into_raw(Box::new(PluginInfo {
        plugin: P::new(),
        client_call_output_buffer: vec![0u8; 0].into_boxed_slice(),
        host_call_input_buffer: vec![0u8; 0].into_boxed_slice(),
    })) as u32
}

// Called to tear down the plugin. Input is the exact same opaque data pointer
// previously retrieved from plugin_init.
#[doc(hidden)]
pub fn plugitin_destroy_impl<P: Plugin>(info: u32) {
    unsafe { Box::from_raw(info as *mut PluginInfo<P>); }
}

// Called to allocate memory so that the host can pass data to the plugin.
#[doc(hidden)]
pub fn plugitin_alloc_impl<P: Plugin>(info: u32, size: u32, align: u32) -> u32 {
    let info_ref = info_ref::<P>(info);
    let layout = std::alloc::Layout::from_size_align(size as usize, align as usize)
        .expect("Invalid layout parameters");
    info_ref.plugin.alloc(layout) as u32
}

// Called to deallocate memory that was previously allocated by plugitin_alloc.
#[doc(hidden)]
pub fn plugitin_dealloc_impl<P: Plugin>(info: u32, ptr: u32, size: u32, align: u32) {
    let info_ref = info_ref::<P>(info);
    let layout = std::alloc::Layout::from_size_align(size as usize, align as usize)
        .expect("Invalid layout parameters");
    let ptr = ptr as *mut u8;
    info_ref.plugin.dealloc(ptr, layout);
}

// Allows the host to call the client.
#[doc(hidden)]
pub fn plugitin_client_call_impl<P: Plugin>(info: u32, input_packed: u64) -> u64 {
    let info_ref = info_ref::<P>(info);

    // Read input.
    let (input_ptr, input_len) = unpack_buffer_desc(input_packed);
    let input_slice: &[u8] = unsafe {
        std::slice::from_raw_parts(input_ptr as *const u8, input_len as usize)
    };
    let call_input: P::ClientCallInput = deserialize_from(input_slice)
        .expect("Failed to deserialize client call input");

    // Call plugin logic.
    let mut host = Host::new(info, &mut info_ref.host_call_input_buffer);
    let call_output = info_ref.plugin.call(&call_input, &mut host);

    // Determine whether we need to expand the output buffer.
    let output_len = serialized_size(&call_output)
        .expect("Failed to compute serialized size for client call output");

    if output_len as usize > info_ref.client_call_output_buffer.len() {
        let new_buffer = vec![0u8; output_len as usize].into_boxed_slice();
        // Free the old buffer and replace it with the new.
        let _ = std::mem::replace(&mut info_ref.client_call_output_buffer, new_buffer);
    }

    let output_slice: &mut [u8] = &mut info_ref.client_call_output_buffer;
    serialize_into(output_slice, &call_output)
        .expect("Failed to serialize client call output");

    let output_ptr = info_ref.client_call_output_buffer.as_mut_ptr() as u32;
    pack_buffer_desc(output_ptr, output_len as u32)
}

struct PluginInfo<T> {
    plugin: T,
    // The client is responsible for writing to these buffers, so it owns them so that it
    // can enlarge them when necessary. The host will own the other two buffers that it
    // is responsible for writing to.
    client_call_output_buffer: Box<[u8]>,
    host_call_input_buffer: Box<[u8]>,
}

fn info_ref<'info, P>(info: u32) -> &'info mut PluginInfo<P> {
    unsafe { (info as *mut PluginInfo<P>).as_mut() }
        .expect("Host provided a plugin pointer that is null")
}

extern "C" {
    // Allows the host to call the client. input_ptr and input_len describe a memory
    // location in the plugin's linear memory that the plugin wrote the serialized input
    // data to. output_ptr is where the plugin expects the host to write the
    // serialized output to. The return value is how many bytes the host wrote to
    // output_ptr. The size of allocated memory at output_ptr is guaranteed to equal
    // the size returned by plugitin_buffer_max.
    fn plugitin_host_call(plugin: u32, input_buffer: u64) -> u64;
}

/// Main trait which plugins must implement.
pub trait Plugin {
    type ClientCallInput  : for<'de> Deserialize<'de>;
    type ClientCallOutput : Serialize;
    type HostCallInput    : Serialize;
    type HostCallOutput   : for<'de> Deserialize<'de>;

    /// Initialize a new plugin.
    fn new() -> Self;

    /// Allocates memory. Necessary so that the host can obtain memory to write to. The
    /// default implementation passes through to the standard Rust allocator. If you
    /// override the default implementation, make sure to also override `dealloc`.
    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        unsafe { std::alloc::alloc_zeroed(layout) }
    }

    /// Deallocates memory. The default implementation passes through to the standard Rust
    /// allocator. If you override the default implementation, make sure to also override
    /// `alloc`.
    fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        unsafe { std::alloc::dealloc(ptr, layout) }
    }

    /// Invoked when the host calls the client.
    fn call(
        &mut self,
        input: &Self::ClientCallInput,
        host: &mut Host<Self::HostCallInput, Self::HostCallOutput>)
        -> Self::ClientCallOutput;
}

pub struct Host<'info, In, Out> {
    info: u32,
    host_call_input_buffer: &'info mut Box<[u8]>,
    _types: PhantomData<(In, Out)>
}

impl<'info, In, Out> Host<'info, In, Out> where In : Serialize, for<'de> Out : Deserialize<'de> {
    fn new(info: u32, host_call_input_buffer: &'info mut Box<[u8]>) -> Self {
        Self {
            info,
            host_call_input_buffer,
            _types: PhantomData
        }
    }

    pub fn call(&mut self, input: In) -> Out {
        // Determine whether we need to expand the input buffer.
        let input_len : usize = serialized_size(&input)
            .expect("Failed to compute serialized size for host call input") as usize;

        if input_len > self.host_call_input_buffer.len() {
            let new_buffer = vec![0u8; input_len].into_boxed_slice();
            // Free the old buffer and replace it with the new.
            let _ = std::mem::replace(self.host_call_input_buffer, new_buffer);
        }

        // Serialize into host's input.
        let input_slice: &mut [u8] = &mut self.host_call_input_buffer;
        serialize_into(input_slice, &input)
            .expect("Failed to serialize host call input");

        let input_ptr = self.host_call_input_buffer.as_mut_ptr() as u32;
        let input_packed = pack_buffer_desc(input_ptr, input_len as u32);

        // Invoke the host.
        let output_packed = unsafe { plugitin_host_call(self.info, input_packed) };
        let (output_ptr, output_len) = unpack_buffer_desc(output_packed);

        // Deserialize from host's output.
        let output_slice: &[u8] = unsafe {
            std::slice::from_raw_parts(output_ptr as *mut u8, output_len as usize)
        };
        deserialize_from(output_slice).expect("Failed to deserialize host call output")
    }
}