// Copyright 2016-2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A crate for measuring the heap usage of data structures in a way that
//! integrates with Firefox's memory reporting, particularly the use of
//! mozjemalloc and DMD. In particular, it has the following features.
//! - It isn't bound to a particular heap allocator.
//! - It provides traits for both "shallow" and "deep" measurement, which gives
//!   flexibility in the cases where the traits can't be used.
//! - It allows for measuring blocks even when only an interior pointer can be
//!   obtained for heap allocations, e.g. `HashSet` and `HashMap`. (This relies
//!   on the heap allocator having suitable support, which mozjemalloc has.)
//! - It allows handling of types like `Rc` and `Arc` by providing traits that
//!   are different to the ones for non-graph structures.
//!
//! Suggested uses are as follows.
//! - When possible, use the `MallocSizeOf` trait. (Deriving support is
//!   provided by the `malloc_size_of_derive` crate.)
//! - If you need an additional synchronization argument, provide a function
//!   that is like the standard trait method, but with the extra argument.
//! - If you need multiple measurements for a type, provide a function named
//!   `add_size_of` that takes a mutable reference to a struct that contains
//!   the multiple measurement fields.
//! - When deep measurement (via `MallocSizeOf`) cannot be implemented for a
//!   type, shallow measurement (via `MallocShallowSizeOf`) in combination with
//!   iteration can be a useful substitute.
//! - `Rc` and `Arc` are always tricky, which is why `MallocSizeOf` is not (and
//!   should not be) implemented for them.
//! - If an `Rc` or `Arc` is known to be a "primary" reference and can always
//!   be measured, it should be measured via the `MallocUnconditionalSizeOf`
//!   trait.
//! - If an `Rc` or `Arc` should be measured only if it hasn't been seen
//!   before, it should be measured via the `MallocConditionalSizeOf` trait.
//! - Using universal function call syntax is a good idea when measuring boxed
//!   fields in structs, because it makes it clear that the Box is being
//!   measured as well as the thing it points to. E.g.
//!   `<Box<_> as MallocSizeOf>::size_of(field, ops)`.

//! This is an extended version of the Servo internal malloc_size crate.
//! We should occasionally track the upstream changes/fixes and reintroduce them here, whenever applicable.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
mod rstd {
	pub use std::*;
}
#[cfg(not(feature = "std"))]
mod rstd {
	pub use core::*;
	pub mod collections {
		pub use alloc::collections::*;
		pub use vec_deque::VecDeque;
	}
}

#[cfg(feature = "std")]
use std::sync::Arc;

#[cfg(not(feature = "std"))]
pub use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use core::ffi::c_void;
#[cfg(feature = "std")]
use rstd::hash::Hash;
use rstd::mem::size_of;
use rstd::ops::Range;
use rstd::ops::{Deref, DerefMut};
#[cfg(feature = "std")]
use std::hash::BuildHasher;
#[cfg(feature = "std")]
use std::os::raw::c_void;

/// A C function that takes a pointer to a heap allocation and returns its size.
pub type VoidPtrToSizeFn = unsafe extern "C" fn(ptr: *const c_void) -> usize;

/// A closure implementing a stateful predicate on pointers.
pub type VoidPtrToBoolFnMut = dyn FnMut(*const c_void) -> bool;

/// Operations used when measuring heap usage of data structures.
pub struct MallocSizeOfOps {
	/// A function that returns the size of a heap allocation.
	size_of_op: VoidPtrToSizeFn,

	/// Like `size_of_op`, but can take an interior pointer. Optional because
	/// not all allocators support this operation. If it's not provided, some
	/// memory measurements will actually be computed estimates rather than
	/// real and accurate measurements.
	enclosing_size_of_op: Option<VoidPtrToSizeFn>,

	/// Check if a pointer has been seen before, and remember it for next time.
	/// Useful when measuring `Rc`s and `Arc`s. Optional, because many places
	/// don't need it.
	have_seen_ptr_op: Option<Box<VoidPtrToBoolFnMut>>,
}

impl MallocSizeOfOps {
	pub fn new(
		size_of: VoidPtrToSizeFn,
		malloc_enclosing_size_of: Option<VoidPtrToSizeFn>,
		have_seen_ptr: Option<Box<VoidPtrToBoolFnMut>>,
	) -> Self {
		MallocSizeOfOps {
			size_of_op: size_of,
			enclosing_size_of_op: malloc_enclosing_size_of,
			have_seen_ptr_op: have_seen_ptr,
		}
	}

	/// Check if an allocation is empty. This relies on knowledge of how Rust
	/// handles empty allocations, which may change in the future.
	fn is_empty<T: ?Sized>(ptr: *const T) -> bool {
		// The correct condition is this:
		//   `ptr as usize <= ::std::mem::align_of::<T>()`
		// But we can't call align_of() on a ?Sized T. So we approximate it
		// with the following. 256 is large enough that it should always be
		// larger than the required alignment, but small enough that it is
		// always in the first page of memory and therefore not a legitimate
		// address.
		return ptr as *const usize as usize <= 256;
	}

	/// Call `size_of_op` on `ptr`, first checking that the allocation isn't
	/// empty, because some types (such as `Vec`) utilize empty allocations.
	pub unsafe fn malloc_size_of<T: ?Sized>(&self, ptr: *const T) -> usize {
		if MallocSizeOfOps::is_empty(ptr) {
			0
		} else {
			(self.size_of_op)(ptr as *const c_void)
		}
	}

	/// Is an `enclosing_size_of_op` available?
	pub fn has_malloc_enclosing_size_of(&self) -> bool {
		self.enclosing_size_of_op.is_some()
	}

	/// Call `enclosing_size_of_op`, which must be available, on `ptr`, which
	/// must not be empty.
	pub unsafe fn malloc_enclosing_size_of<T>(&self, ptr: *const T) -> usize {
		assert!(!MallocSizeOfOps::is_empty(ptr));
		(self.enclosing_size_of_op.unwrap())(ptr as *const c_void)
	}

	/// Call `have_seen_ptr_op` on `ptr`.
	pub fn have_seen_ptr<T>(&mut self, ptr: *const T) -> bool {
		let have_seen_ptr_op = self.have_seen_ptr_op.as_mut().expect("missing have_seen_ptr_op");
		have_seen_ptr_op(ptr as *const c_void)
	}
}

/// Trait for measuring the "deep" heap usage of a data structure. This is the
/// most commonly-used of the traits.
pub trait MallocSizeOf {
	/// Measure the heap usage of all descendant heap-allocated structures, but
	/// not the space taken up by the value itself.
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize;
}

/// Trait for measuring the "shallow" heap usage of a container.
pub trait MallocShallowSizeOf {
	/// Measure the heap usage of immediate heap-allocated descendant
	/// structures, but not the space taken up by the value itself. Anything
	/// beyond the immediate descendants must be measured separately, using
	/// iteration.
	fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize;
}

/// Like `MallocSizeOf`, but with a different name so it cannot be used
/// accidentally with derive(MallocSizeOf). For use with types like `Rc` and
/// `Arc` when appropriate (e.g. when measuring a "primary" reference).
pub trait MallocUnconditionalSizeOf {
	/// Measure the heap usage of all heap-allocated descendant structures, but
	/// not the space taken up by the value itself.
	fn unconditional_size_of(&self, ops: &mut MallocSizeOfOps) -> usize;
}

/// `MallocUnconditionalSizeOf` combined with `MallocShallowSizeOf`.
pub trait MallocUnconditionalShallowSizeOf {
	/// `unconditional_size_of` combined with `shallow_size_of`.
	fn unconditional_shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize;
}

/// Like `MallocSizeOf`, but only measures if the value hasn't already been
/// measured. For use with types like `Rc` and `Arc` when appropriate (e.g.
/// when there is no "primary" reference).
pub trait MallocConditionalSizeOf {
	/// Measure the heap usage of all heap-allocated descendant structures, but
	/// not the space taken up by the value itself, and only if that heap usage
	/// hasn't already been measured.
	fn conditional_size_of(&self, ops: &mut MallocSizeOfOps) -> usize;
}

/// `MallocConditionalSizeOf` combined with `MallocShallowSizeOf`.
pub trait MallocConditionalShallowSizeOf {
	/// `conditional_size_of` combined with `shallow_size_of`.
	fn conditional_shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize;
}

#[cfg(not(any(all(target_os = "macos", not(feature = "jemalloc-global"),), feature = "estimate-heapsize")))]
pub mod inner_allocator_use {

	use super::*;

	#[cfg(not(feature = "std"))]
	use alloc::string::String;

	impl<T: ?Sized> MallocShallowSizeOf for Box<T> {
		fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
			unsafe { ops.malloc_size_of(&**self) }
		}
	}

	impl<T> MallocShallowSizeOf for Vec<T> {
		fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
			unsafe { ops.malloc_size_of(self.as_ptr()) }
		}
	}

	// currently this seems only fine with jemalloc
	#[cfg(feature = "std")]
	#[cfg(all(feature = "jemalloc-global", not(target_os = "windows")))]
	impl<T> MallocUnconditionalShallowSizeOf for Arc<T> {
		fn unconditional_shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
			unsafe { ops.malloc_size_of(arc_ptr(self)) }
		}
	}

	#[cfg(feature = "std")]
	#[cfg(not(all(feature = "jemalloc-global", not(target_os = "windows"))))]
	impl<T> MallocUnconditionalShallowSizeOf for Arc<T> {
		fn unconditional_shallow_size_of(&self, _ops: &mut MallocSizeOfOps) -> usize {
			size_of::<T>()
		}
	}

	impl MallocSizeOf for String {
		fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
			unsafe { ops.malloc_size_of(self.as_ptr()) }
		}
	}
}

impl<'a, T: ?Sized> MallocSizeOf for &'a T {
	fn size_of(&self, _ops: &mut MallocSizeOfOps) -> usize {
		// Zero makes sense for a non-owning reference.
		0
	}
}

impl<T: MallocSizeOf + ?Sized> MallocSizeOf for Box<T> {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		self.shallow_size_of(ops) + (**self).size_of(ops)
	}
}

#[impl_trait_for_tuples::impl_for_tuples(12)]
impl MallocSizeOf for Tuple {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		let mut result = 0;
		for_tuples!( #( result += Tuple.size_of(ops); )* );
		result
	}
}

impl<T: MallocSizeOf> MallocSizeOf for Option<T> {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		if let Some(val) = self.as_ref() {
			val.size_of(ops)
		} else {
			0
		}
	}
}

impl<T: MallocSizeOf, E: MallocSizeOf> MallocSizeOf for Result<T, E> {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		match *self {
			Ok(ref x) => x.size_of(ops),
			Err(ref e) => e.size_of(ops),
		}
	}
}

impl<T: MallocSizeOf + Copy> MallocSizeOf for rstd::cell::Cell<T> {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		self.get().size_of(ops)
	}
}

impl<T: MallocSizeOf> MallocSizeOf for rstd::cell::RefCell<T> {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		self.borrow().size_of(ops)
	}
}

#[cfg(feature = "std")]
impl<'a, B: ?Sized + ToOwned> MallocSizeOf for std::borrow::Cow<'a, B>
where
	B::Owned: MallocSizeOf,
{
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		match *self {
			std::borrow::Cow::Borrowed(_) => 0,
			std::borrow::Cow::Owned(ref b) => b.size_of(ops),
		}
	}
}

impl<T: MallocSizeOf> MallocSizeOf for [T] {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		let mut n = 0;
		for elem in self.iter() {
			n += elem.size_of(ops);
		}
		n
	}
}

impl<T: MallocSizeOf> MallocSizeOf for Vec<T> {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		let mut n = self.shallow_size_of(ops);
		for elem in self.iter() {
			n += elem.size_of(ops);
		}
		n
	}
}

impl<T> MallocShallowSizeOf for rstd::collections::VecDeque<T> {
	fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		if ops.has_malloc_enclosing_size_of() {
			if let Some(front) = self.front() {
				// The front element is an interior pointer.
				unsafe { ops.malloc_enclosing_size_of(&*front) }
			} else {
				// This assumes that no memory is allocated when the VecDeque is empty.
				0
			}
		} else {
			// An estimate.
			self.capacity() * size_of::<T>()
		}
	}
}

impl<T: MallocSizeOf> MallocSizeOf for rstd::collections::VecDeque<T> {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		let mut n = self.shallow_size_of(ops);
		for elem in self.iter() {
			n += elem.size_of(ops);
		}
		n
	}
}

#[cfg(feature = "std")]
impl<T, S> MallocShallowSizeOf for std::collections::HashSet<T, S>
where
	T: Eq + Hash,
	S: BuildHasher,
{
	fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		if ops.has_malloc_enclosing_size_of() {
			// The first value from the iterator gives us an interior pointer.
			// `ops.malloc_enclosing_size_of()` then gives us the storage size.
			// This assumes that the `HashSet`'s contents (values and hashes)
			// are all stored in a single contiguous heap allocation.
			self.iter().next().map_or(0, |t| unsafe { ops.malloc_enclosing_size_of(t) })
		} else {
			// An estimate.
			self.capacity() * (size_of::<T>() + size_of::<usize>())
		}
	}
}

#[cfg(feature = "std")]
impl<T, S> MallocSizeOf for std::collections::HashSet<T, S>
where
	T: Eq + Hash + MallocSizeOf,
	S: BuildHasher,
{
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		let mut n = self.shallow_size_of(ops);
		for t in self.iter() {
			n += t.size_of(ops);
		}
		n
	}
}

impl<I: MallocSizeOf> MallocSizeOf for rstd::cmp::Reverse<I> {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		self.0.size_of(ops)
	}
}

#[cfg(feature = "std")]
impl<K, V, S> MallocShallowSizeOf for std::collections::HashMap<K, V, S> {
	fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		// See the implementation for std::collections::HashSet for details.
		if ops.has_malloc_enclosing_size_of() {
			self.values().next().map_or(0, |v| unsafe { ops.malloc_enclosing_size_of(v) })
		} else {
			self.capacity() * (size_of::<V>() + size_of::<K>() + size_of::<usize>())
		}
	}
}

#[cfg(feature = "std")]
impl<K, V, S> MallocSizeOf for std::collections::HashMap<K, V, S>
where
	K: MallocSizeOf,
	V: MallocSizeOf,
{
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		let mut n = self.shallow_size_of(ops);
		for (k, v) in self.iter() {
			n += k.size_of(ops);
			n += v.size_of(ops);
		}
		n
	}
}

impl<K, V> MallocShallowSizeOf for rstd::collections::BTreeMap<K, V> {
	fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		if ops.has_malloc_enclosing_size_of() {
			self.values().next().map_or(0, |v| unsafe { ops.malloc_enclosing_size_of(v) })
		} else {
			self.len() * (size_of::<V>() + size_of::<K>() + size_of::<usize>())
		}
	}
}

impl<K, V> MallocSizeOf for rstd::collections::BTreeMap<K, V>
where
	K: MallocSizeOf,
	V: MallocSizeOf,
{
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		let mut n = self.shallow_size_of(ops);
		for (k, v) in self.iter() {
			n += k.size_of(ops);
			n += v.size_of(ops);
		}
		n
	}
}

// PhantomData is always 0.
impl<T> MallocSizeOf for rstd::marker::PhantomData<T> {
	fn size_of(&self, _ops: &mut MallocSizeOfOps) -> usize {
		0
	}
}

// XXX: we don't want MallocSizeOf to be defined for Rc and Arc. If negative
// trait bounds are ever allowed, this code should be uncommented.
// (We do have a compile-fail test for this:
// rc_arc_must_not_derive_malloc_size_of.rs)
//impl<T> !MallocSizeOf for Arc<T> { }
//impl<T> !MallocShallowSizeOf for Arc<T> { }

#[cfg(feature = "std")]
fn arc_ptr<T>(s: &Arc<T>) -> *const T {
	&(**s) as *const T
}

#[cfg(feature = "std")]
impl<T: MallocSizeOf> MallocUnconditionalSizeOf for Arc<T> {
	fn unconditional_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		self.unconditional_shallow_size_of(ops) + (**self).size_of(ops)
	}
}

#[cfg(feature = "std")]
impl<T> MallocConditionalShallowSizeOf for Arc<T> {
	fn conditional_shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		if ops.have_seen_ptr(arc_ptr(self)) {
			0
		} else {
			self.unconditional_shallow_size_of(ops)
		}
	}
}

#[cfg(feature = "std")]
impl<T: MallocSizeOf> MallocConditionalSizeOf for Arc<T> {
	fn conditional_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		if ops.have_seen_ptr(arc_ptr(self)) {
			0
		} else {
			self.unconditional_size_of(ops)
		}
	}
}

/// If a mutex is stored directly as a member of a data type that is being measured,
/// it is the unique owner of its contents and deserves to be measured.
///
/// If a mutex is stored inside of an Arc value as a member of a data type that is being measured,
/// the Arc will not be automatically measured so there is no risk of overcounting the mutex's
/// contents.
///
/// The same reasoning applies to RwLock.
#[cfg(feature = "std")]
impl<T: MallocSizeOf> MallocSizeOf for std::sync::Mutex<T> {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		self.lock().unwrap().size_of(ops)
	}
}

#[cfg(feature = "std")]
impl<T: MallocSizeOf> MallocSizeOf for parking_lot::Mutex<T> {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		self.lock().size_of(ops)
	}
}

#[cfg(feature = "std")]
impl<T: MallocSizeOf> MallocSizeOf for std::sync::RwLock<T> {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		self.read().unwrap().size_of(ops)
	}
}

#[cfg(feature = "std")]
impl<T: MallocSizeOf> MallocSizeOf for parking_lot::RwLock<T> {
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		self.read().size_of(ops)
	}
}

#[macro_export]
macro_rules! malloc_size_of_is_0(
    ($($ty:ty),+) => (
        $(
            impl $crate::MallocSizeOf for $ty {
                #[inline(always)]
                fn size_of(&self, _: &mut $crate::MallocSizeOfOps) -> usize {
                    0
                }
            }
        )+
    );
    ($($ty:ident<$($gen:ident),+>),+) => (
        $(
        impl<$($gen: $crate::MallocSizeOf),+> $crate::MallocSizeOf for $ty<$($gen),+> {
            #[inline(always)]
            fn size_of(&self, _: &mut $crate::MallocSizeOfOps) -> usize {
                0
            }
        }
        )+
    );
);

malloc_size_of_is_0!(bool, char, str);
malloc_size_of_is_0!(u8, u16, u32, u64, u128, usize);
malloc_size_of_is_0!(i8, i16, i32, i64, i128, isize);
malloc_size_of_is_0!(f32, f64);

malloc_size_of_is_0!(rstd::sync::atomic::AtomicBool);
malloc_size_of_is_0!(rstd::sync::atomic::AtomicIsize);
malloc_size_of_is_0!(rstd::sync::atomic::AtomicUsize);

malloc_size_of_is_0!(Range<u8>, Range<u16>, Range<u32>, Range<u64>, Range<usize>);
malloc_size_of_is_0!(Range<i8>, Range<i16>, Range<i32>, Range<i64>, Range<isize>);
malloc_size_of_is_0!(Range<f32>, Range<f64>);

/// Measurable that defers to inner value and used to verify MallocSizeOf implementation in a
/// struct.
#[derive(Clone)]
pub struct Measurable<T: MallocSizeOf>(pub T);

impl<T: MallocSizeOf> Deref for Measurable<T> {
	type Target = T;

	fn deref(&self) -> &T {
		&self.0
	}
}

impl<T: MallocSizeOf> DerefMut for Measurable<T> {
	fn deref_mut(&mut self) -> &mut T {
		&mut self.0
	}
}

#[cfg(feature = "hashbrown")]
impl<K, V, S> MallocShallowSizeOf for hashbrown::HashMap<K, V, S> {
	fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		// See the implementation for std::collections::HashSet for details.
		if ops.has_malloc_enclosing_size_of() {
			self.values().next().map_or(0, |v| unsafe { ops.malloc_enclosing_size_of(v) })
		} else {
			self.capacity() * (size_of::<V>() + size_of::<K>() + size_of::<usize>())
		}
	}
}

#[cfg(feature = "hashbrown")]
impl<K, V, S> MallocSizeOf for hashbrown::HashMap<K, V, S>
where
	K: MallocSizeOf,
	V: MallocSizeOf,
{
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		let mut n = self.shallow_size_of(ops);
		for (k, v) in self.iter() {
			n += k.size_of(ops);
			n += v.size_of(ops);
		}
		n
	}
}

#[cfg(feature = "lru")]
impl<K, V, S> MallocSizeOf for lru::LruCache<K, V, S>
where
	K: MallocSizeOf + rstd::cmp::Eq + rstd::hash::Hash,
	V: MallocSizeOf,
	S: rstd::hash::BuildHasher,
{
	fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
		let mut n = 0;
		for (k, v) in self.iter() {
			n += k.size_of(ops);
			n += v.size_of(ops);
		}
		n
	}
}
