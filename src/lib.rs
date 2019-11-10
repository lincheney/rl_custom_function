#[macro_use]
extern crate lazy_static;
extern crate libc;

use std::env;
use std::ffi::{OsStr, OsString, CStr, CString};
use std::os::unix::ffi::{OsStringExt, OsStrExt};
use std::sync::Once;

macro_rules! dlsym_lookup {
    ($handle:expr, $name:expr) => {
        dlsym_lookup!($handle, $name, _)
    };
    ($handle:expr, $name:expr, $result:ty) => {{
        let name = concat!($name, "\0");
        let func = libc::dlsym($handle, name.as_ptr() as *const i8);
        if func.is_null() {
            None
        } else {
            Some(std::mem::transmute::<_, $result>(func))
        }
    }}
}

mod readline {
    use libc::{RTLD_NEXT, RTLD_DEFAULT};

    #[allow(non_camel_case_types)]
    pub type rl_command_func_t = extern fn(isize, isize) -> isize;

    fn rl_add_funmap_entry(name: *const u8, function: rl_command_func_t) -> isize {
        let func = unsafe{ dlsym_lookup!(RTLD_DEFAULT, "rl_add_funmap_entry", fn(*const u8, rl_command_func_t)->isize) };
        let func = func.expect("could not find symbol rl_add_funmap_entry");
        func(name, function)
    }

    pub fn add_function(name: &[u8], function: rl_command_func_t) {
        let name = ::CString::new(name).unwrap();
        rl_add_funmap_entry(name.as_ptr() as *const u8, function);
        // readline now owns the string
        ::std::mem::forget(name);
    }

    lazy_static! {
        pub static ref RL_INITIALIZE_FUNMAP: unsafe extern fn() = {
            unsafe{ dlsym_lookup!(RTLD_NEXT, "rl_initialize_funmap") }.expect("could not find rl_initialize_funmap")
        };
    }
}

fn add_function(name: &[u8]) -> Result<(), OsString> {
    let name_cstr = CString::new(name).unwrap();

    let lib = unsafe{ ::libc::dlopen(name_cstr.as_ptr() as *const i8, ::libc::RTLD_NOW) };
    if ! lib.is_null() {
        if let Some(func) = unsafe{ dlsym_lookup!(lib, "rl_custom_function") } {
            // use filename as command name
            let command = std::path::Path::new(OsStr::from_bytes(name)).file_stem().unwrap();
            readline::add_function(command.as_bytes(), func);
            return Ok(());
        }
    }

    let error = unsafe{ ::libc::dlerror() };
    let error = unsafe{ CStr::from_ptr(error) }.to_bytes();
    let error = OsStr::from_bytes(error).to_os_string();
    Err(error)
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

    unsafe{ readline::RL_INITIALIZE_FUNMAP() }
}
