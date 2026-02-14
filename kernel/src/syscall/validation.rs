use alloc::string::String;
use alloc::vec::Vec;
use core::mem::{align_of, size_of};

use kernel_abi::{Errno, EFAULT, EINVAL};
use kernel_syscall::UserspacePtr;

/// Copy a struct from userspace to kernel.
/// Validates: non-null, canonical address, alignment, within userspace range.
pub fn copy_from_userspace<T: Copy>(ptr: usize) -> Result<T, Errno> {
    if ptr == 0 {
        return Err(EFAULT);
    }

    // SAFETY: We validate that ptr is in the userspace address range (canonical lower half)
    // via try_from_usize, which rejects kernel addresses. This prevents userspace from
    // tricking the kernel into reading kernel memory.
    let user_ptr = unsafe { UserspacePtr::<T>::try_from_usize(ptr)? };
    user_ptr.validate_range(size_of::<T>())?;

    // Validate alignment
    if !ptr.is_multiple_of(align_of::<T>()) {
        return Err(EINVAL);
    }

    // SAFETY: Address has been validated to be:
    // 1. Non-null (checked above)
    // 2. In userspace address range (validated by try_from_usize)
    // 3. Properly aligned for type T (checked above)
    // 4. Within valid address bounds (validated by validate_range)
    // The read is a Copy type, so we produce an owned value.
    Ok(unsafe { *(ptr as *const T) })
}

/// Read a slice from userspace. Returns owned Vec.
pub fn read_userspace_slice(ptr: usize, len: usize) -> Result<Vec<u8>, Errno> {
    if ptr == 0 || len == 0 {
        return Err(EFAULT);
    }

    // SAFETY: We validate that ptr is in the userspace address range (canonical lower half)
    // via try_from_usize, which rejects kernel addresses.
    let user_ptr = unsafe { UserspacePtr::<u8>::try_from_usize(ptr)? };
    user_ptr.validate_range(len)?;

    // SAFETY: Address has been validated to be:
    // 1. Non-null (checked above)
    // 2. In userspace address range (validated by try_from_usize)
    // 3. Within valid bounds for len bytes (validated by validate_range)
    // u8 has no alignment requirements. We immediately copy to an owned Vec.
    let slice = unsafe { core::slice::from_raw_parts(ptr as *const u8, len) };
    Ok(slice.to_vec())
}

/// Read a null-terminated string from userspace.
/// Returns a String.
/// Validates that the string is within userspace bounds and does not exceed max_len.
pub fn read_userspace_string(ptr: usize, max_len: usize) -> Result<String, Errno> {
    if ptr == 0 {
        return Err(EFAULT);
    }

    let mut bytes = Vec::new();
    let mut current_addr = ptr;

    for _ in 0..max_len {
        // Validate address
        // SAFETY: We are checking the address before reading.
        // We use try_from_usize to ensure it's in userspace range.
        let uptr = unsafe { UserspacePtr::<u8>::try_from_usize(current_addr)? };

        // Read one byte
        // SAFETY: Address is validated (canonical userspace).
        let b = unsafe { *uptr.as_ptr() };

        if b == 0 {
            return String::from_utf8(bytes).map_err(|_| EINVAL);
        }

        bytes.push(b);
        current_addr += 1;
    }

    Err(EINVAL) // String too long or no null terminator found
}

/// Read a null-terminated array of pointers to strings (argv/envp).
pub fn read_userspace_string_array(
    ptr: usize,
    max_count: usize,
    max_string_len: usize,
) -> Result<Vec<String>, Errno> {
    if ptr == 0 {
        return Ok(Vec::new());
    }

    let mut strings = Vec::new();
    let mut current_addr = ptr;

    for _ in 0..max_count {
        // Validate pointer to the pointer
        // SAFETY: Checking userspace bounds for the pointer array entry.
        let uptr = unsafe { UserspacePtr::<usize>::try_from_usize(current_addr)? };
        uptr.validate_range(size_of::<usize>())?;

        // Read the pointer
        // SAFETY: Validated above.
        let str_ptr = unsafe { *uptr.as_ptr() };

        if str_ptr == 0 {
            return Ok(strings);
        }

        let s = read_userspace_string(str_ptr, max_string_len)?;
        strings.push(s);

        current_addr += size_of::<usize>();
    }

    Err(EINVAL) // Too many arguments or no null terminator found for array
}

/// Copy data to userspace buffer.
pub fn copy_to_userspace(ptr: usize, data: &[u8]) -> Result<(), Errno> {
    if ptr == 0 {
        return Err(EFAULT);
    }

    // SAFETY: We validate that ptr is in the userspace address range (canonical lower half)
    // via try_from_usize, which rejects kernel addresses.
    let user_ptr = unsafe { UserspacePtr::<u8>::try_from_usize(ptr)? };
    user_ptr.validate_range(data.len())?;

    // SAFETY: Address has been validated to be:
    // 1. Non-null (checked above)
    // 2. In userspace address range (validated by try_from_usize)
    // 3. Within valid bounds for data.len() bytes (validated by validate_range)
    // u8 has no alignment requirements. copy_nonoverlapping requires non-overlapping
    // src/dst, which is guaranteed since data is kernel memory and ptr is userspace.
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len()) }
    Ok(())
}
