#[macro_use]
extern crate lazy_static;
extern crate libc;

use std::sync::{Once, ONCE_INIT};

mod readline {
    type rl_command_func_t = extern fn(isize, isize) -> isize;

    #[link(name = "readline")]
    extern {
        pub fn rl_add_funmap_entry(name: *const u8, function: rl_command_func_t) -> isize;
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

static INIT: Once = ONCE_INIT;

#[no_mangle]
pub extern fn rl_initialize_funmap() {
    INIT.call_once(|| {
        unsafe{ readline::rl_add_funmap_entry(b"hello\0" as *const u8, custom_command) };
    });
    unsafe{ readline::RL_INITIALIZE_FUNMAP() }
}

extern fn custom_command(count: isize, key: isize) -> isize {
    println!("{:?}", 123);
    0
}
