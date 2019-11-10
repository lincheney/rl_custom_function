#[macro_use]
extern crate lazy_static;
extern crate libc;

use std::env;
use std::ffi::OsStr;
use std::os::unix::ffi::{OsStringExt, OsStrExt};
use std::sync::Once;

macro_rules! dynlib_call {
    ($func:ident($($args:expr),*)) => {{
        let ptr = {
            use libc::$func;
            $func($($args),*)
        };
        if ptr.is_null() {
            let error = libc::dlerror();
            if error.is_null() {
                Err(concat!("unknown error calling: ", stringify!($func)))
            } else {
                Err(std::ffi::CStr::from_ptr(error).to_str().unwrap())
            }
        } else {
            Ok(ptr)
        }
    }}
}

macro_rules! dlopen {
    ($name:expr) => { dlopen!($name, libc::RTLD_LAZY) };
    ($name:expr, $flags:expr) => { dynlib_call!(dlopen($name.as_ptr() as _, $flags)) };
}

macro_rules! dlsym {
    ($handle:expr, $name:expr) => {
        dlsym!($handle, $name, _)
    };
    ($handle:expr, $name:expr, $type:ty) => {{
        let name = concat!($name, "\0");
        #[allow(clippy::transmute_ptr_to_ptr)]
        dynlib_call!(dlsym($handle, name.as_ptr() as _)).map(|sym|
            std::mem::transmute::<_, $type>(sym)
        )
    }}
}

mod readline {
    pub use self::lib::rl_initialize_funmap;

    pub fn add_function(name: &[u8], function: lib::rl_command_func_t) {
        let name = std::ffi::CString::new(name).unwrap();
        unsafe{ lib::rl_add_funmap_entry(name.as_ptr(), function) };
        // readline now owns the string
        std::mem::forget(name);
    }

    #[allow(non_upper_case_globals, non_camel_case_types)]
    mod lib {
        use std::marker::PhantomData;
        pub type rl_command_func_t = extern fn(isize, isize) -> isize;

        pub struct Pointer<T>(usize, PhantomData<T>);
        impl<T> Pointer<T> {
            pub fn new(ptr: *mut T)    -> Self { Self(ptr as _, PhantomData) }
            pub fn ptr(&self)        -> *mut T { self.0 as *mut T }
            pub unsafe fn set(&self, value: T) { *self.ptr() = value; }
        }

        lazy_static! {
            pub static ref libreadline: Pointer<libc::c_void> = Pointer::new(unsafe {
                if dlsym!(libc::RTLD_DEFAULT, "rl_initialize", usize).is_ok() {
                    libc::RTLD_DEFAULT
                } else {
                    dlopen!(b"libreadline.so\0").unwrap()
                }
            });
        }
        macro_rules! readline_lookup {
            ($name:ident: $type:ty) => {
                readline_lookup!($name: $type, libreadline.ptr());
            };
            ($name:ident: $type:ty, $handle:expr) => {
                lazy_static! { pub static ref $name: $type = unsafe{ dlsym!($handle, stringify!($name)) }.unwrap(); }
            }
        }

        readline_lookup!(rl_initialize_funmap: unsafe extern fn(),
            if libreadline.ptr() == libc::RTLD_DEFAULT { libc::RTLD_NEXT } else { libreadline.ptr() });
        readline_lookup!(rl_add_funmap_entry:  unsafe extern fn(*const i8, rl_command_func_t) -> isize);
    }
}

fn add_function(name: &[u8]) -> Result<(), &str> {
    let name_cstr = std::ffi::CString::new(name).unwrap();

    unsafe{ dlopen!(name_cstr) }.and_then(|lib|
        unsafe{ dlsym!(lib, "rl_custom_function") }.map(|func| {
            // use filename as command name
            let command = std::path::Path::new(OsStr::from_bytes(name)).file_stem().unwrap();
            readline::add_function(command.as_bytes(), func);
        })
    )
}

static INIT: Once = Once::new();

#[no_mangle]
pub extern fn rl_initialize_funmap() {
    INIT.call_once(|| {
        if let Some(value) = env::var_os("READLINE_CUSTOM_FUNCTION_LIBS") {
            let value = value.into_vec();

            for part in value.split(|c| *c == b':') {
                if let Err(e) = add_function(part) {
                    let part = OsStr::from_bytes(part);
                    eprintln!("Error loading {:?}: {:?}", part, e);
                }
            }
        }
    });

    unsafe{ readline::rl_initialize_funmap() }
}
