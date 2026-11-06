//! `NSFileCoordinator` access: every Authentic read
//! before a write and every publication runs inside a coordinated accessor, so cooperating
//! writers (Finder, iCloud, other coordinating apps) are serialized with us. A coordination
//! failure means the closure did not run and nothing may be written.
use block2::RcBlock;
use objc2_foundation::{
    NSError, NSFileCoordinator, NSFileCoordinatorReadingOptions, NSFileCoordinatorWritingOptions,
    NSString, NSURL,
};
use std::{cell::RefCell, path::Path, ptr::NonNull};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoordError;

fn file_url(path: &Path) -> Result<objc2::rc::Retained<NSURL>, CoordError> {
    let text = path.to_str().ok_or(CoordError)?;
    Ok(NSURL::fileURLWithPath(&NSString::from_str(text)))
}

/// Runs `f` inside a coordinated read of `path`.
pub fn read<T>(path: &Path, f: impl FnOnce() -> T) -> Result<T, CoordError> {
    access(path, f, |coordinator, url, error, block| {
        coordinator.coordinateReadingItemAtURL_options_error_byAccessor(
            url,
            NSFileCoordinatorReadingOptions::empty(),
            error,
            block,
        )
    })
}

/// Runs `f` inside a coordinated write that replaces or creates `path`.
pub fn write<T>(path: &Path, f: impl FnOnce() -> T) -> Result<T, CoordError> {
    access(path, f, |coordinator, url, error, block| {
        coordinator.coordinateWritingItemAtURL_options_error_byAccessor(
            url,
            NSFileCoordinatorWritingOptions::ForReplacing,
            error,
            block,
        )
    })
}

/// Runs deletion inside a dedicated accessor. Replacement coordination is not a
/// deletion contract; every member must be revalidated within this closure.
pub fn delete<T>(path: &Path, f: impl FnOnce() -> T) -> Result<T, CoordError> {
    access(path, f, |coordinator, url, error, block| {
        coordinator.coordinateWritingItemAtURL_options_error_byAccessor(
            url,
            NSFileCoordinatorWritingOptions::ForDeleting,
            error,
            block,
        )
    })
}

type Block<'a> = block2::DynBlock<dyn Fn(NonNull<NSURL>) + 'a>;
type Error = Option<objc2::rc::Retained<NSError>>;

fn access<T>(
    path: &Path,
    f: impl FnOnce() -> T,
    call: impl FnOnce(&NSFileCoordinator, &NSURL, Option<&mut Error>, &Block),
) -> Result<T, CoordError> {
    let url = file_url(path)?;
    let f = RefCell::new(Some(f));
    let slot = RefCell::new(None);
    let block = RcBlock::new(|_: NonNull<NSURL>| {
        // The accessor is a `Fn` block, but Foundation invokes it at most once.
        if let Some(f) = f.borrow_mut().take() {
            *slot.borrow_mut() = Some(f());
        }
    });
    let coordinator = NSFileCoordinator::new();
    let mut error: Error = None;
    call(&coordinator, &url, Some(&mut error), &block);
    drop(block);
    slot.into_inner().ok_or(CoordError)
}

#[cfg(test)]
mod tests;
