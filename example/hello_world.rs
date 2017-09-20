#![crate_type = "dylib"]

#[no_mangle]
pub extern fn rl_custom_function(_: isize, _: isize) -> isize {
    println!("Hello World!");
    0
}
