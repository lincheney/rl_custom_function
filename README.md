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

Custom functions can be loaded via readline init files
(e.g. `~/.inputrc`, or equivalently `rl_parse_bind`)
using a `$include` directive like so:
```
$include function FUNCTION PATH
"\C-g": FUNCTION
```
where `PATH` is a path to shared object that exports a function conforming to:
```c
typedef int rl_command_func_t (int, int);
```

## Example

There is a very simple example in [example/hello_world.rs](example/hello_world.rs).

Compile the function by running:
```bash
rustc example/hello_world.rs -o hello_world.so
```

Then add to your `~/.inputrc`:
```
$include function hello_world /path/to/hello_world.so
"\C-g": hello_world
```

Run something interactive that uses readline, e.g. python:
```bash
LD_PRELOAD=target/release/librl_custom_function.so python
```

... and press control-g.
