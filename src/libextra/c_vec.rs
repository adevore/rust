// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!
 * Library to interface with chunks of memory allocated in C.
 *
 * It is often desirable to safely interface with memory allocated from C,
 * encapsulating the unsafety into allocation and destruction time.  Indeed,
 * allocating memory externally is currently the only way to give Rust shared
 * mut state with C programs that keep their own references; vectors are
 * unsuitable because they could be reallocated or moved at any time, and
 * importing C memory into a vector takes a one-time snapshot of the memory.
 *
 * This module simplifies the usage of such external blocks of memory.  Memory
 * is encapsulated into an opaque object after creation; the lifecycle of the
 * memory can be optionally managed by Rust, if an appropriate destructor
 * closure is provided.  Safety is ensured by bounds-checking accesses, which
 * are marshalled through get and set functions.
 *
 * There are three unsafe functions: the two constructors, and the
 * unwrap method. The constructors are unsafe for the
 * obvious reason (they act on a pointer that cannot be checked inside the
 * method), but `unwrap()` is somewhat more subtle in its unsafety.
 * It returns the contained pointer, but at the same time destroys the CVec
 * without running its destructor. This can be used to pass memory back to
 * C, but care must be taken that the ownership of underlying resources are
 * handled correctly, i.e. that allocated memory is eventually freed
 * if necessary.
 */

use std::ptr;

/**
 * The type representing a foreign chunk of memory
 */
pub struct CVec<T> {
    priv base: *mut T,
    priv len: uint,
    priv rsrc: DtorRes,
}

struct DtorRes {
    dtor: Option<proc()>,
}

#[unsafe_destructor]
impl Drop for DtorRes {
    fn drop(&mut self) {
        let dtor = self.dtor.take();
        match dtor {
            None => (),
            Some(f) => f()
        }
    }
}

impl DtorRes {
    fn new(dtor: Option<proc()>) -> DtorRes {
        DtorRes {
            dtor: dtor,
        }
    }
}

impl <T> CVec<T> {
    /**
     * Create a `CVec` from a raw pointer to a buffer with a given length.
     *
     * Fails if the given pointer is null.
     *
     * # Arguments
     *
     * * base - A raw pointer to a buffer
     * * len - The number of elements in the buffer
     */
    pub unsafe fn new(base: *mut T, len: uint) -> CVec<T> {
        assert!(base != ptr::mut_null());
        CVec {
            base: base,
            len: len,
            rsrc: DtorRes::new(None)
        }
    }

    /**
     * Create a `CVec` from a foreign buffer, with a given length,
     * and a function to run upon destruction.
     *
     * Fails if the given pointer is null.
     *
     * # Arguments
     *
     * * base - A foreign pointer to a buffer
     * * len - The number of elements in the buffer
     * * dtor - A proc to run when the value is destructed, useful
     *          for freeing the buffer, etc.
     */
    pub unsafe fn new_with_dtor(base: *mut T, len: uint, dtor: proc()) -> CVec<T> {
        assert!(base != ptr::mut_null());
        CVec {
            base: base,
            len: len,
            rsrc: DtorRes::new(Some(dtor))
        }
    }

    /**
     * Retrieves an element at a given index
     *
     * Fails if `ofs` is greater or equal to the length of the vector
     */
    pub fn get<'a>(&'a self, ofs: uint) -> &'a T {
        assert!(ofs < self.len);
        unsafe {
            &*self.base.offset(ofs as int)
        }
    }

    /**
     * Retrieves a mutable element at a given index
     *
     * Fails if `ofs` is greater or equal to the length of the vector
     */
    pub fn get_mut<'a>(&'a mut self, ofs: uint) -> &'a mut T {
        assert!(ofs < self.len);
        unsafe {
            &mut *self.base.offset(ofs as int)
        }
    }

    /**
     * Unwrap the pointer without running the destructor
     *
     * This method retrieves the underlying pointer, and in the process
     * destroys the CVec but without running the destructor. A use case
     * would be transferring ownership of the buffer to a C function, as
     * in this case you would not want to run the destructor.
     *
     * Note that if you want to access the underlying pointer without
     * cancelling the destructor, you can simply call `transmute` on the return
     * value of `get(0)`.
     */
    pub unsafe fn unwrap(mut self) -> *mut T {
        self.rsrc.dtor = None;
        self.base
    }
}

impl <T> Container for CVec<T> {
    /// Returns the length of the vector
    fn len(&self) -> uint { self.len }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::libc::*;
    use std::libc;
    use std::ptr;
    use std::rt::global_heap::malloc_raw;

    fn malloc(n: uint) -> CVec<u8> {
        unsafe {
            let mem = malloc_raw(n);

            CVec::new_with_dtor(mem as *mut u8, n,
                proc() { libc::free(mem as *mut c_void); })
        }
    }

    #[test]
    fn test_basic() {
        let mut cv = malloc(16);

        *cv.get_mut(3) = 8;
        *cv.get_mut(4) = 9;
        assert_eq!(*cv.get(3), 8);
        assert_eq!(*cv.get(4), 9);
        assert_eq!(cv.len(), 16);
    }

    #[test]
    #[should_fail]
    fn test_fail_at_null() {
        unsafe {
            CVec::new(ptr::mut_null::<u8>(), 9);
        }
    }

    #[test]
    #[should_fail]
    fn test_overrun_get() {
        let cv = malloc(16);

        cv.get(17);
    }

    #[test]
    #[should_fail]
    fn test_overrun_set() {
        let mut cv = malloc(16);

        *cv.get_mut(17) =  0;
    }

    #[test]
    fn test_unwrap() {
        unsafe {
            let cv = CVec::new_with_dtor(1 as *mut int, 0,
                proc() { fail!("Don't run this destructor!") });
            let p = cv.unwrap();
            assert_eq!(p, 1 as *mut int);
        }
    }

}
