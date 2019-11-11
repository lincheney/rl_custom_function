#[macro_use]
extern crate lazy_static;
extern crate libc;

use std::env;
use std::ffi::OsStr;
use std::os::unix::ffi::{OsStringExt, OsStrExt};
use std::sync::Once;

type DynlibResult<T> = Result<T, &'static str>;

macro_rules! dump_error {
    ($result:expr, $default:expr) => {
        match $result {
            Ok(x) => x,
            Err(e) => { eprintln!("{}", e); return $default },
        }
    }
}

macro_rules! dynlib_call {
    ($func:ident($($args:expr),*)) => {{
        let ptr = libc::$func($($args),*);
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

    pub fn add_function(name: &[u8], function: lib::rl_command_func_t) -> ::DynlibResult<()> {
        let name = std::ffi::CString::new(name).unwrap();
        unsafe{ (*lib::rl_add_funmap_entry)?(name.as_ptr(), function) };
        // readline now owns the string
        std::mem::forget(name);
        Ok(())
    }

    #[allow(non_upper_case_globals, non_camel_case_types)]
    mod lib {
        pub type rl_command_func_t = extern fn(isize, isize) -> isize;

        macro_rules! readline_lookup {
            ($name:ident: $type:ty) => {
                readline_lookup!($name: $type; libc::RTLD_DEFAULT);
            };
            ($name:ident: $type:ty; $handle:expr) => {
                lazy_static! {
                    pub static ref $name: ::DynlibResult<$type> = unsafe {
                        dlsym!($handle, stringify!($name)).or_else(|_|
                            dynlib_call!(dlopen(b"libreadline.so\0".as_ptr() as _, libc::RTLD_NOLOAD | libc::RTLD_LAZY))
                            .and_then(|lib| dlsym!(lib, stringify!($name)))
                        )};
                }
            }
        }

        readline_lookup!(rl_initialize_funmap: unsafe extern fn(); libc::RTLD_NEXT);
        readline_lookup!(rl_add_funmap_entry:  unsafe extern fn(*const i8, rl_command_func_t) -> isize);
    }
}

fn add_function(name: &[u8]) -> DynlibResult<()> {
    let name_cstr = std::ffi::CString::new(name).unwrap();

    let lib = unsafe{ dynlib_call!(dlopen(name_cstr.as_ptr() as _, libc::RTLD_LAZY)) }?;
    let func = unsafe{ dlsym!(lib, "rl_custom_function") }?;
    // use filename as command name
    let command = std::path::Path::new(OsStr::from_bytes(name)).file_stem().unwrap();
    readline::add_function(command.as_bytes(), func)
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

    unsafe{ dump_error!(*readline::rl_initialize_funmap, ())() }
}
