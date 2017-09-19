#[macro_use]
extern crate lazy_static;
extern crate libc;

use std::env;
use std::ffi::{OsStr, OsString, CStr, CString};
use std::os::unix::ffi::{OsStringExt, OsStrExt};
use std::sync::{Once, ONCE_INIT};

mod readline {
    pub type rl_command_func_t = extern fn(isize, isize) -> isize;

    #[link(name = "readline")]
    extern {
        fn rl_add_funmap_entry(name: *const u8, function: rl_command_func_t) -> isize;
    }

    pub fn add_function(name: &[u8], function: rl_command_func_t) {
        let name = ::CString::new(name).unwrap();
        unsafe{ rl_add_funmap_entry(name.as_ptr() as *const u8, function) };
        // readline now owns the string
        ::std::mem::forget(name);
    }

    lazy_static! {
        pub static ref RL_INITIALIZE_FUNMAP: unsafe extern fn() = {
            let name = "rl_initialize_funmap\0";
            let func = unsafe{ ::libc::dlsym(::libc::RTLD_NEXT, name.as_ptr() as *const i8) };
            if func.is_null() {
                panic!("could not find symbol {}", name);
            }
            unsafe{ ::std::mem::transmute(func) }
        };
    }
}

fn add_function(name: &[u8]) -> Result<(), OsString> {
    let name_cstr = CString::new(name).unwrap();

    let lib = unsafe{ ::libc::dlopen(name_cstr.as_ptr() as *const i8, ::libc::RTLD_NOW) };
    if ! lib.is_null() {
        let func = unsafe{ ::libc::dlsym(lib, b"rl_custom_function\0".as_ptr() as *const i8) };
        if ! func.is_null() {
            let func = unsafe{ ::std::mem::transmute(func) };
            // use filename as command name
            let command = std::path::Path::new(OsStr::from_bytes(name)).file_stem().unwrap();
            readline::add_function(command.as_bytes(), func);
            return Ok(());
        }
    }

    let error = unsafe{ ::libc::dlerror() };
    let error = unsafe{ CStr::from_ptr(error) }.to_bytes();
    let error = OsStr::from_bytes(error).to_os_string();
    return Err(error)
}

static INIT: Once = ONCE_INIT;

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

    unsafe{ readline::RL_INITIALIZE_FUNMAP() }
}
