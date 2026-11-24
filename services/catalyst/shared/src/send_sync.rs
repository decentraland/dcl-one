//! Proving a state struct `Send + Sync` at its definition.
//!
//! A future that must be `Send` proves the auto traits of the state it
//! captures through that state's fields. [`send_sync_once!`] asserts each
//! field type `Send + Sync` at the definition and implements both traits on
//! the struct, so those proofs stop at the struct.

/// Defines a non-generic struct whose `Send` and `Sync` are proven from its
/// field types at the definition. A field that is not both `Send` and `Sync`
/// fails to compile, so a `Send`-only field (an mpsc `Receiver`) is refused.
/// Field attributes are repeated on each field's assertion, so only doc
/// comments and `#[cfg]` are supported there; the struct takes doc comments only.
///
/// ```
/// catalyst_shared::send_sync_once! {
///     struct Shared { name: String, count: std::sync::Arc<std::sync::Mutex<u64>> }
/// }
/// ```
///
/// A field that is not `Send + Sync` is rejected:
///
/// ```compile_fail,E0277
/// catalyst_shared::send_sync_once! {
///     struct Local { count: std::rc::Rc<u64> }
/// }
/// ```
///
/// So is a generic struct, and any outer attribute but a doc comment:
///
/// ```compile_fail
/// catalyst_shared::send_sync_once! {
///     struct Generic<T> { value: T }
/// }
/// ```
///
/// ```compile_fail
/// catalyst_shared::send_sync_once! {
///     #[derive(Default)]
///     struct Derived { name: String }
/// }
/// ```
#[macro_export]
macro_rules! send_sync_once {
    (
        $(#[doc = $doc:expr])*
        $vis:vis struct $name:ident {
            $( $(#[$field_meta:meta])* $field_vis:vis $field:ident : $ty:ty ),* $(,)?
        }
    ) => {
        $(#[doc = $doc])*
        $vis struct $name {
            $( $(#[$field_meta])* $field_vis $field: $ty ),*
        }

        const _: () = {
            #[allow(dead_code, unused_doc_comments)]
            fn fields() {
                fn send_sync<T: ?Sized + Send + Sync>() {}
                $( $(#[$field_meta])* send_sync::<$ty>(); )*
            }
        };

        // SAFETY: every field type is asserted `Send + Sync` above, at least
        // what the auto impls require (they need `Send` fields for `Send` and
        // `Sync` fields for `Sync`). Only doc comments may precede the struct,
        // so no attribute can gate it apart from these impls or rewrite it.
        unsafe impl Send for $name {}
        // SAFETY: as above.
        unsafe impl Sync for $name {}
    };
}

#[cfg(test)]
mod tests {
    crate::send_sync_once! {
        /// Doc comments and field visibility pass through.
        pub struct Wide {
            /// A field doc.
            pub name: String,
            #[cfg(any())]
            absent: std::rc::Rc<u8>,
            count: std::sync::Arc<std::sync::Mutex<u64>>,
        }
    }

    fn send_sync<T: Send + Sync>() {}

    #[test]
    fn the_struct_is_send_and_sync() {
        send_sync::<Wide>();
        let wide = Wide {
            name: String::new(),
            count: Default::default(),
        };
        assert_eq!(*wide.count.lock().unwrap(), 0);
        assert!(wide.name.is_empty());
    }
}
