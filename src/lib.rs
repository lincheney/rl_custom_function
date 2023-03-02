extern crate once_cell;
extern crate libc;

use std::os::raw::c_char;

type DynlibResult<T> = Result<T, String>;

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
                Err(concat!("unknown error calling: ", stringify!($func)).to_string())
            } else {
                Err(std::ffi::CStr::from_ptr(error).to_str().unwrap().to_string())
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
    pub use self::lib::rl_parse_and_bind;

    pub fn add_function(name: &[u8], function: lib::rl_command_func_t) -> ::DynlibResult<()> {
        let name = std::ffi::CString::new(name).unwrap();
        unsafe{ (*lib::rl_add_funmap_entry).as_ref()?(name.as_ptr(), function) };
        // readline now owns the string
        std::mem::forget(name);
        Ok(())
    }

    pub fn tilde_expand(string: &str) -> ::DynlibResult<String> {
        let string = std::ffi::CString::new(string).unwrap();
        let string = unsafe{ (*lib::tilde_expand).as_ref()?(string.as_ptr()) };
        let string = unsafe{ std::ffi::CString::from_raw(string) }.into_string();
        string.map_err(|_| "tilde_expand: invalid utf-8".to_string())
    }

    #[allow(non_upper_case_globals, non_camel_case_types)]
    mod lib {
        use std::os::raw::c_char;
        use libc::c_void;
        use once_cell::sync::Lazy;

        pub type rl_command_func_t = extern fn(isize, isize) -> isize;

        struct Lib(*mut c_void);
        unsafe impl Sync for Lib {}
        unsafe impl Send for Lib {}

        static libreadline: Lazy<::DynlibResult<Lib>> = Lazy::new(|| unsafe {
            unsafe extern "C" fn callback(info: *mut libc::dl_phdr_info, _size: usize, data: *mut c_void) -> libc::c_int {
                if let Ok(lib) = dynlib_call!(dlopen((*info).dlpi_name, libc::RTLD_GLOBAL | libc::RTLD_LAZY)) {
                    let symbol: ::DynlibResult<*const c_char> = dlsym!(lib, "rl_library_version");
                    if symbol.is_ok() {
                        *(data as *mut *mut c_void) = lib;
                        return 1
                    }
                }
                0
            }
            let mut lib: *mut c_void = std::ptr::null_mut();
            libc::dl_iterate_phdr(Some(callback), &mut lib as *mut *mut c_void as _);

            if lib.is_null() {
                dynlib_call!(dlopen(b"libreadline.so\0".as_ptr() as _, libc::RTLD_GLOBAL | libc::RTLD_LAZY)).map(|lib| Lib(lib))
            } else {
                Ok(Lib(lib))
            }
        });

        macro_rules! readline_lookup {
            ($name:ident: $type:ty) => {
                readline_lookup!($name: $type; libc::RTLD_DEFAULT);
            };
            ($name:ident: $type:ty; $handle:expr) => {
                pub static $name: Lazy<::DynlibResult<$type>> = Lazy::new(|| unsafe {
                    dlsym!($handle, stringify!($name)).or_else(|_|
                        (*libreadline).as_ref().map_err(|s| s.clone()).and_then(|lib| dlsym!(lib.0, stringify!($name)))
                    )
                });
            }
        }

        readline_lookup!(rl_add_funmap_entry:  unsafe extern fn(*const c_char, rl_command_func_t) -> isize);
        readline_lookup!(rl_parse_and_bind:  unsafe extern fn(*mut c_char) -> isize; libc::RTLD_NEXT);
        readline_lookup!(tilde_expand:  unsafe extern fn(*const c_char) -> *mut c_char);
    }
}

fn add_function(name: &str, path: &str) -> DynlibResult<()> {
    let path = std::ffi::CString::new(path).unwrap();
    let lib = unsafe{ dynlib_call!(dlopen(path.as_ptr() as _, libc::RTLD_LAZY)) }?;
    let func = unsafe{ dlsym!(lib, "rl_custom_function") }?;
    readline::add_function(name.as_bytes(), func)
}

#[no_mangle]
pub extern fn rl_parse_and_bind(string: *mut c_char) -> isize {
    if ! string.is_null() {
        let string = unsafe{ std::ffi::CStr::from_ptr(string) }.to_str().unwrap();
        let mut parts = string.trim_start().splitn(4, char::is_whitespace);
        let directive = parts.next().unwrap_or("");
        let plugin = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("");
        let path = parts.next().unwrap_or("");

        if directive == "$include" && plugin == "function" {
            if let Err(e) = readline::tilde_expand(path).and_then(|s| add_function(name, &s)) {
                eprintln!("Error loading {:?}: {:?}", path, e);
                return 1
            }
            return 0
        }
    }
    unsafe{ dump_error!(&*readline::rl_parse_and_bind, 1)(string) }
}
