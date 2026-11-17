//! Proving a state struct `Send + Sync` at its definition.
//!
//! A future that must be `Send` proves the auto traits of the state it
//! captures through that state's fields. [`send_sync_once!`] asserts each
//! field type `Send + Sync` at the definition and implements both traits on
//! the struct, so those proofs stop at the struct.

/// Defines a non-generic struct whose `Send` and `Sync` are proven from its
/// field types at the definition. A field that is not `Send + Sync` fails to
/// compile. Field attributes are repeated on each field's assertion, so only
/// doc comments and `#[cfg]` are supported.
///
/// ```
/// catalyst_shared::send_sync_once! {
///     struct Shared { name: String, count: std::sync::Arc<std::sync::Mutex<u64>> }
/// }
/// ```
///
/// A field that is not `Send + Sync` is rejected:
///
/// ```compile_fail
/// catalyst_shared::send_sync_once! {
///     struct Local { count: std::rc::Rc<u64> }
/// }
/// ```
#[macro_export]
macro_rules! send_sync_once {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $( $(#[$field_meta:meta])* $field_vis:vis $field:ident : $ty:ty ),* $(,)?
        }
    ) => {
        $(#[$meta])*
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

        // SAFETY: every field type is asserted `Send + Sync` above, which is
        // what the auto impls would require.
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
