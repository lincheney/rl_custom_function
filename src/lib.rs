#[macro_use]
extern crate lazy_static;
extern crate libc;

use std::env;
use std::ffi::{OsStr, OsString, CString};
use std::os::unix::ffi::{OsStringExt, OsStrExt};
use std::sync::Once;

macro_rules! dynlib_call {
    ($func:ident($($args:expr),*)) => {{
        let ptr = {
            use ::libc::$func;
            $func($($args),*)
        };
        if ptr.is_null() {
            use std::os::unix::ffi::OsStrExt;
            let error = ::libc::dlerror();
            let error = ::std::ffi::CStr::from_ptr(error).to_bytes();
            let error = ::std::ffi::OsStr::from_bytes(error).to_os_string();
            Err(error)
        } else {
            Ok(ptr)
        }
    }}
}

macro_rules! dlsym_lookup {
    ($handle:expr, $name:expr) => {
        dlsym_lookup!($handle, $name, _)
    };
    ($handle:expr, $name:expr, $type:ty) => {{
        let name = concat!($name, "\0");
        #[allow(clippy::transmute_ptr_to_ptr)]
        dynlib_call!(dlsym($handle, name.as_ptr() as _)).map(|sym|
            std::mem::transmute::<_, $type>(sym)
        )
    }}
}

macro_rules! readline_dylsym {
    ($name:expr) => {
        readline_dylsym!($name, _)
    };
    ($name:expr, $type:ty) => {
        dynlib_call!(dlopen(b"libreadline.so\0".as_ptr() as _, ::libc::RTLD_LAZY)).and_then(|lib|
            dlsym_lookup!(lib, $name)
        )
    }
}

mod readline {
    pub use self::lib::rl_initialize_funmap;

    pub fn add_function(name: &[u8], function: lib::rl_command_func_t) {
        let name = ::CString::new(name).unwrap();
        unsafe{ lib::rl_add_funmap_entry(name.as_ptr(), function) };
        // readline now owns the string
        ::std::mem::forget(name);
    }

    #[allow(non_upper_case_globals)]
    mod lib {
        #[allow(non_camel_case_types)]
        pub type rl_command_func_t = extern fn(isize, isize) -> isize;

        lazy_static! {
            pub static ref rl_initialize_funmap: unsafe extern fn() = unsafe{ readline_dylsym!("rl_initialize_funmap") }.unwrap();
            pub static ref rl_add_funmap_entry: unsafe extern fn(*const i8, rl_command_func_t) -> isize = unsafe{ readline_dylsym!("rl_add_funmap_entry") }.unwrap();
        }
    }
}

fn add_function(name: &[u8]) -> Result<(), OsString> {
    let name_cstr = CString::new(name).unwrap();

    unsafe{ dynlib_call!(dlopen(name_cstr.as_ptr(), ::libc::RTLD_LAZY)) }.and_then(|lib|
        unsafe{ dlsym_lookup!(lib, "rl_custom_function") }.map(|func| {
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
