use std::sync::Arc;

use wayland_commons::user_data::UserData;
use wayland_commons::{Interface, MessageGroup};

use wayland_sys::server::*;

use crate::imp::ResourceInner;
use crate::{Client, Filter};

/// An handle to a wayland resource
///
/// This represents a wayland object instantiated in a client
/// session. Several handles to the same object can exist at a given
/// time, and cloning them won't create a new protocol object, only
/// clone the handle. The lifetime of the protocol object is **not**
/// tied to the lifetime of these handles, but rather to sending or
/// receiving destroying messages.
///
/// These handles are notably used to send events to the associated client,
/// via the `send` method, although you're encouraged to use methods on the
/// corresponding Rust objects instead. To convert a `Resource<I>` into the
/// `I` Rust object, use the `.into()` method.
pub struct Resource<I: Interface> {
    _i: ::std::marker::PhantomData<&'static I>,
    inner: ResourceInner,
}

impl<I: Interface> PartialEq for Resource<I> {
    fn eq(&self, other: &Resource<I>) -> bool {
        self.equals(other)
    }
}

impl<I: Interface> Eq for Resource<I> {}

impl<I: Interface> Resource<I> {
    #[allow(dead_code)]
    pub(crate) fn wrap(inner: ResourceInner) -> Resource<I> {
        Resource {
            _i: ::std::marker::PhantomData,
            inner,
        }
    }

    /// Send an event through this object
    ///
    /// The event will be send to the client associated to this
    /// object.
    pub fn send(&self, msg: I::Event) {
        #[cfg(feature = "native_lib")]
        {
            if !self.is_external() && !self.is_alive() {
                return;
            }
        }
        #[cfg(not(feature = "native_lib"))]
        {
            if !self.is_alive() {
                return;
            }
        }
        if msg.since() > self.version() {
            let opcode = msg.opcode() as usize;
            panic!(
                "Cannot send event {} which requires version >= {} on resource {}@{} which is version {}.",
                I::Event::MESSAGES[opcode].name,
                msg.since(),
                I::NAME,
                self.id(),
                self.version()
            );
        }
        self.inner.send::<I>(msg)
    }

    /// Check if the object associated with this resource is still alive
    ///
    /// Will return `false` if either:
    ///
    /// - The object has been destroyed
    /// - The object is not managed by this library (see the `from_c_ptr` method)
    pub fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }

    /// Retrieve the interface version of this wayland object instance
    ///
    /// Returns 0 on dead objects
    pub fn version(&self) -> u32 {
        self.inner.version()
    }

    /// Check if the other resource refers to the same underlying wayland object
    pub fn equals(&self, other: &Resource<I>) -> bool {
        self.inner.equals(&other.inner)
    }

    /// Check if this resource and the other belong to the same client
    ///
    /// Always return false if either of them is dead
    pub fn same_client_as<II: Interface>(&self, other: &Resource<II>) -> bool {
        self.inner.same_client_as(&other.inner)
    }

    /// Posts a protocol error to this resource
    ///
    /// The error code can be obtained from the various `Error` enums of the protocols.
    ///
    /// An error is fatal to the client that caused it.
    pub fn post_error(&self, error_code: u32, msg: String) {
        self.inner.post_error(error_code, msg)
    }

    /// Access the UserData associated to this object
    ///
    /// Each wayland object has an associated UserData, that can store
    /// a payload of arbitrary type and is shared by all proxies of this
    /// object.
    ///
    /// See UserData documentation for more details.
    pub fn user_data(&self) -> &Arc<UserData> {
        self.inner.user_data()
    }

    /// Retrieve an handle to the client associated with this resource
    ///
    /// Returns `None` if the resource is no longer alive.
    pub fn client(&self) -> Option<Client> {
        self.inner.client().map(Client::make)
    }

    /// Retrieve the object id of this wayland object
    pub fn id(&self) -> u32 {
        self.inner.id()
    }
}

impl<I: Interface> Resource<I> {
    /// Check whether this resource is managed by the library or not
    ///
    /// See `from_c_ptr` for details.
    ///
    /// NOTE: This method will panic if called while the `native_lib` feature is not
    /// activated
    pub fn is_external(&self) -> bool {
        #[cfg(feature = "native_lib")]
        {
            self.inner.is_external()
        }
        #[cfg(not(feature = "native_lib"))]
        {
            panic!(
                "[wayland-server] C interfacing methods are not available without the `native_lib` feature"
            );
        }
    }

    /// Get a raw pointer to the underlying wayland object
    ///
    /// Retrieve a pointer to the object from the `libwayland-server.so` library.
    /// You will mostly need it to interface with C libraries needing access
    /// to wayland objects (to initialize an opengl context for example).
    ///
    /// NOTE: This method will panic if called while the `native_lib` feature is not
    /// activated
    pub fn c_ptr(&self) -> *mut wl_resource {
        #[cfg(feature = "native_lib")]
        {
            self.inner.c_ptr()
        }
        #[cfg(not(feature = "native_lib"))]
        {
            panic!(
                "[wayland-server] C interfacing methods are not available without the `native_lib` feature"
            );
        }
    }

    /// Create a `Resource` instance from a C pointer to a new object
    ///
    /// Create a `Resource` from a raw pointer to a wayland object from the
    /// C library by taking control of it. You must ensure that this object was newly
    /// created and have never been user nor had any listener associated.
    ///
    /// In order to handle protocol races, invoking it with a NULL pointer will
    /// create an already-dead object.
    ///
    /// NOTE: This method will panic if called while the `native_lib` feature is not
    /// activated
    pub unsafe fn init_from_c_ptr(_ptr: *mut wl_resource) -> Self
    where
        I: From<Resource<I>>,
    {
        #[cfg(feature = "native_lib")]
        {
            Resource {
                _i: ::std::marker::PhantomData,
                inner: ResourceInner::init_from_c_ptr::<I>(_ptr),
            }
        }
        #[cfg(not(feature = "native_lib"))]
        {
            panic!(
                "[wayland-server] C interfacing methods are not available without the `native_lib` feature"
            );
        }
    }

    /// Create a `Resource` instance from a C pointer
    ///
    /// Create a `Resource` from a raw pointer to a wayland object from the
    /// C library.
    ///
    /// If the pointer was previously obtained by the `c_ptr()` method, this
    /// constructs a new resource handle for the same object just like the
    /// `clone()` method would have.
    ///
    /// If the object was created by some other C library you are interfacing
    /// with, it will be created in an "unmanaged" state: wayland-server will
    /// treat it as foreign, and as such most of the safeties will be absent.
    /// Notably the lifetime of the object can't be tracked, so the `alive()`
    /// method will always return `false` and you are responsible of not using
    /// an object past its destruction (as this would cause a protocol error).
    /// You will also be unable to associate any user data pointer to this object.
    ///
    /// In order to handle protocol races, invoking it with a NULL pointer will
    /// create an already-dead object.
    ///
    /// NOTE: This method will panic if called while the `native_lib` feature is not
    /// activated
    pub unsafe fn from_c_ptr(_ptr: *mut wl_resource) -> Self
    where
        I: From<Resource<I>>,
    {
        #[cfg(feature = "native_lib")]
        {
            Resource {
                _i: ::std::marker::PhantomData,
                inner: ResourceInner::from_c_ptr::<I>(_ptr),
            }
        }
        #[cfg(not(feature = "native_lib"))]
        {
            panic!(
                "[wayland-server] C interfacing methods are not available without the `native_lib` feature"
            );
        }
    }

    #[doc(hidden)]
    // This method is for scanner-generated use only
    pub unsafe fn make_child_for<J: Interface + From<Resource<J>>>(&self, _id: u32) -> Option<Resource<J>> {
        #[cfg(feature = "native_lib")]
        {
            self.inner.make_child_for::<J>(_id).map(Resource::wrap)
        }
        #[cfg(not(feature = "native_lib"))]
        {
            panic!(
                "[wayland-server] C interfacing methods are not available without the `native_lib` feature"
            );
        }
    }

    pub fn assign<E>(&self, filter: Filter<E>)
    where
        I: AsRef<Resource<I>> + From<Resource<I>>,
        E: From<(Resource<I>, I::Request)> + 'static,
        I::Request: MessageGroup<Map = crate::ResourceMap>,
    {
        self.inner.assign(filter);
    }

    pub fn assign_mono<F>(&self, mut f: F)
    where
        I: Interface + AsRef<Resource<I>> + From<Resource<I>>,
        F: FnMut(Resource<I>, I::Request) + 'static,
        I::Request: MessageGroup<Map = crate::ResourceMap>,
    {
        self.assign(Filter::new(move |(proxy, event), _| f(proxy, event)))
    }

    pub fn assign_destructor<E>(&self, filter: Filter<E>)
    where
        I: AsRef<Resource<I>> + From<Resource<I>>,
        E: From<Resource<I>> + 'static,
    {
        self.inner.assign_destructor(filter)
    }
}

impl<I: Interface> Clone for Resource<I> {
    fn clone(&self) -> Resource<I> {
        Resource {
            _i: ::std::marker::PhantomData,
            inner: self.inner.clone(),
        }
    }
}
