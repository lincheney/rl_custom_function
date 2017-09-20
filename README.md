# rl_custom_function

This is a `LD_PRELOAD` hack to enable you to inject custom
functions in to any readline application.

http://web.mit.edu/gnu/doc/html/rlman_2.html#SEC23

Custom functions are usually added to readline at compile-time
of the application, but this becomes tricky if you want to add
a custom readline function to many readline-enabled applications
(python, irb etc) without patching and compiling everything from
source yourself.

## Building

Run:
```bash
cargo build --release
```

The resulting lib should be at: `target/release/librl_custom_function.so`

## Usage

Custom functions are loaded from the environment variable
`READLINE_CUSTOM_FUNCTION_LIBS`.
It should be a colon delimited list of paths to shared objects
containing the custom function.

The shared object should export a function conforming to:
```c
typedef int rl_command_func_t (int, int);
```

The name the function will be bound to is the filename of the .so
(excepting the extension).

## Example

There is a very simple example in `example/hello_world.rs`.

Compile it by running:
```bash
rustc example/hello_world.rs -o hello_world.so
```

Then add a binding to your `~/.inputrc`:
```
"\C-g": hello_world
```

Run a something interactive that uses readline, e.g. python:
```bash
LD_PRELOAD=target/release/librl_custom_function.so \
READLINE_CUSTOM_FUNCTION_LIBS=./hello_world.so \
python
```

... and press control-g.
