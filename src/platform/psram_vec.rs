//! PSRAM 分配的 Vec 包装器，用于大块音频缓冲。
//! PSRAM-backed Vec wrapper for large audio buffers.

use super::heap::{alloc_spiram_buffer, free_spiram_buffer};

/// PSRAM 分配的 i16 缓冲区。失败时回退到普通 Vec。
pub struct PsramVecI16 {
    ptr: Option<*mut i16>,
    len: usize,
    fallback: Option<Vec<i16>>,
}

impl PsramVecI16 {
    pub fn new(capacity: usize) -> Self {
        let byte_size = capacity * std::mem::size_of::<i16>();
        if let Some(ptr) = alloc_spiram_buffer(byte_size) {
            unsafe {
                std::ptr::write_bytes(ptr, 0, byte_size);
            }
            Self {
                ptr: Some(ptr as *mut i16),
                len: capacity,
                fallback: None,
            }
        } else {
            Self {
                ptr: None,
                len: 0,
                fallback: Some(vec![0i16; capacity]),
            }
        }
    }

    pub fn as_mut_slice(&mut self) -> &mut [i16] {
        if let Some(ptr) = self.ptr {
            unsafe { std::slice::from_raw_parts_mut(ptr, self.len) }
        } else {
            self.fallback.as_mut().unwrap().as_mut_slice()
        }
    }

    pub fn len(&self) -> usize {
        if self.ptr.is_some() {
            self.len
        } else {
            self.fallback.as_ref().unwrap().len()
        }
    }
}

impl Drop for PsramVecI16 {
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr {
            unsafe { free_spiram_buffer(ptr as *mut u8) };
        }
    }
}

unsafe impl Send for PsramVecI16 {}
