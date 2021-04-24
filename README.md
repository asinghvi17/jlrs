# jlrs

[![Rust Docs](https://docs.rs/jlrs/badge.svg)](https://docs.rs/jlrs)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)


The main goal behind jlrs is to provide a simple and safe interface to the Julia C API that lets you call code written in Julia from Rust and vice versa. Currently this crate is only tested on Linux in combination with Julia 1.6 and is not compatible with earlier versions of Julia.


## Features

An incomplete list of features that are currently supported by jlrs:

 - Access arbitrary Julia modules and their contents.
 - Call arbitrary Julia functions, including functions that take keyword arguments.
 - Include and call your own Julia code.
 - Load a custom system image.
 - Create values that Julia can use, and convert them back to Rust, from Rust.
 - Access the type information and fields of values and check their properties.
 - Create and use n-dimensional arrays.
 - Support for mapping Julia structs to Rust structs which can be generated with `JlrsReflect.jl`.
 - Structs that can be mapped to Rust include those with type parameters and bits unions.
 - Use these features when calling Rust from Julia through `ccall`.
 - Offload long-running functions to another thread and `.await` the result with the (experimental) async runtime.


## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
jlrs = "0.10"
```

This crate depends on jl-sys which contains the raw bindings to the Julia C API, these are generated by bindgen. You can find the requirements for using bindgen in [their User Guide](https://rust-lang.github.io/rust-bindgen/requirements.html).

#### Linux

The recommended way to install Julia is to download the binaries from the official website, which is distributed in an archive containing a directory called `julia-x.y.z`. This directory contains several other directories, including a `bin` directory containing the `julia` executable.

In order to ensure the `julia.h` header file can be found, either `/usr/include/julia/julia.h` must exist, or you have to set the `JULIA_DIR` environment variable to `/path/to/julia-x.y.z`. This environment variable can be used to override the default. Similarly, in order to load `libjulia.so` you must add `/path/to/julia-x.y.z/lib` to the `LD_LIBRARY_PATH` environment variable.

#### Windows

Support for Windows was dropped in jlrs 0.10 due to compilation and dependency issues. 


## Using this crate

The first thing you should do is `use` the `prelude`-module with an asterisk, this will bring all the structs and traits you're likely to need into scope. If you're calling Julia from Rust, Julia must be initialized before you can use it. You can do this by calling `Julia::init`, which provides you with an instance of `Julia`. Note that this method can only be called once, if you drop its result you won't be able to create a new instance but have to restart the application. If you want to use a custom system image, you must call `Julia::init_with_image` instead of `Julia::init`. If you're calling Rust from Julia everything has already been initialized, you can use `CCall` instead.


### Calling Julia from Rust

After initialization you have an instance of `Julia`; `Julia::include` can be used to include files with custom Julia code. In order to call Julia functions and create new values that can be used by these functions, `Julia::scope` and `Julia::scope_with_slots` must be used. These two methods take a closure with two arguments, a `Global` and a mutable reference to a `GcFrame`. `Global` is a token that is used to access Julia modules, their contents and other global values, while `GcFrame` is used to root local values. Rooting a value in a frame prevents it from being freed by the garbage collector until that frame has been dropped. The frame is created when `Julia::scope(_with_slots)` is called and dropped when that method returns.

Julia data is represented as a `Value`. There are several ways to create a new `Value`. The simplest is to call `Value::eval_string`, a method that takes two arguments. The first must implement the `Scope` trait, the second is a string which has to contain valid Julia code. The most important thing to know about the `Scope` trait for now is that it's used by values that create new values to ensure they're rooted. Mutable references to `GcFrame`s implement `Scope`, in this case the `Value` that is returned is rooted in that frame, and is protected from garbage collection until the frame is dropped when that scope ends. In practice, `Value::eval_string` is relatively limited. It can be used to evaluate simple function call like `sqrt(2.0)`, but can't take any arguments. It's important to be aware though that it can also be used to import installed packages by evaluating an `import` or ` using` statement. To create a `Value` directly, without evaluating Julia code, other methods like `Value::new` are available. `Value::new` supports converting primitive types from Rust to Julia, but can also be used with some more complex types like `String`s. New arrays can be created with methods like `Value::new_array`, but it only supports types that can be converted from Rust to Julia with `Value::new`.

Julia functions are `Value`s too. In fact, all `Value`s can be called as functions, whether this will succeed depends on the value actually being a function that is implemented for the arguments it's called with. Note that calling a Julia function from Rust has a significant amount of overhead because it needs to figure out what implementation to dispatch to. It's generally more effective to call a few large Julia function from Rust, than many small ones.

In order to call some `Value` as a Julia function three things are needed: the function you want to call, something that implements `Scope` to root the result, and possibly some arguments the function is called with. The function can be acquired through the module that defines it with `Module::function`; `Module::base` and `Module::core` provide access to Julia's `Base` and `Core` module respectively, while everything you include through `Julia::include` or by `Value::eval_string` is made available relative to the `Main` module, which can be accessed by calling `Module::main`. To actually call the function, one of the trait methods of `Call` must be used. To call a function that takes keyword arguments, `Value::with_keywords` must be used. See the documentation of that method for more information.

Because a `Value` must only be used while it's rooted, it cannot leave the scope it's tied to. In order to return Julia data from a scope, it must first be converted to another type that contains no more data owned by the Julia garbage collector, e.g. `u8` and `String`. To do so, `Value::cast` must be used. This method checks if the value can be converted to the type it's cast to and performs the conversion if it is. This generally amounts to a pointer dereference, but for builtin types like `DataType` and `Array` it's a pointer cast. These builtin types are still owned by Julia, and as such subject to the same lifetime constraints as a `Value` is, but much of their functionality is exposed as methods on the more specific type. For example, in order to access the data in an `Array` from Rust, the `Value` must be cast to an `Array` first.

As a simple example, let's create two values and add them:

```rust
use jlrs::prelude::*;
fn main() {
    let mut julia = unsafe { Julia::init().unwrap() };
    julia.scope(|global, frame| {
        // Create the two arguments. Note that the first argument, something that
        // implements Scope, is taken by value and mutable references don't implement
        // Copy, so it's necessary to mutably reborrow the frame.
        let i = Value::new(&mut *frame, 2u64)?;
        let j = Value::new(&mut *frame, 1u32)?;

        // The `+` function can be found in the base module.
        let func = Module::base(global).function("+")?;

        // Call the function and cast the result to `u64`. The result of the function
        // call is a nested `Result`; the outer error does not contain to any Julia
        // data, while the inner error contains the exception if one is thrown.
        func.call2(&mut *frame, i, j)?
            .into_jlrs_result()?
            .cast::<u64>()
    }).unwrap();
}
```

Scopes can be nested, this is especially useful when you need to create several temporary values to create a new `Value` or call a Julia function because each scope has its own `GcFrame`. This means these temporary values will not be protected from garbage collection after returning from this new scope. There are three methods that create a nested scope, `ScopeExt::scope`, `Scope::value_scope` and `Scope::result_scope`. Like `Scope`, `ScopeExt` is implemented for mutable references to `GcFrame`s. The first is very similar to the previous example and has the same major limitation: its return value can be anything, as long as its guaranteed to live at least as long as it can outlive the current scope. This means you can't create a `Value` or call a Julia function and return its result to the parent scope with this method. The other two methods support those use-cases, in particular `Scope::value_scope` can be used to create a `Value` in an inner scope and root it in an outer one, while `Scope::result_scope` can do the same for the result of a Julia function call.

Another implementation of `Scope` appears here: the closure that `value_scope` and `result_scope` take has two arguments, an `Output` and a mutable reference to a `GcFrame`. The frame can be used to root temporary values, the output must be converted to an `OutputScope` before creating the value that must be rooted in an earlier frame. This `OutputScope` also implements `Scope`, but unlike a `GcFrame` it's implemented for the type itself rather than a mutable reference to it so it can only be used once. The two values from the previous example can be rooted in an inner scope, while their sum is returned to and rooted in the outer scope:

```rust
use jlrs::prelude::*;
fn main() {
    let mut julia = unsafe { Julia::init().unwrap() };
    julia.scope(|global, parent_frame| {
        let sum_value: Value = parent_frame.result_scope(|output, child_frame| {
            // i and j are rooted in `child_frame`...
            let i = Value::new(&mut *child_frame, 1u64)?;
            let j = Value::new(&mut *child_frame, 2i32)?;
            let func = Module::base(global).function("+")?;

            // ... while the result is rooted in `parent_frame`
            // after returning from this closure.
            let output_scope = output.into_scope(child_frame);
            func.call2(output_scope, i, j)
        })?.into_jlrs_result()?;

        assert_eq!(sum_value.cast::<u64>()?, 3);

        Ok(())
    }).unwrap();
}
```

This is only a small example, other things can be done with `Value` as well. Their fields can be accessed with `Value::get_field`, properties of the value's type can be checked with `Value::is`, and `Value::apply_type` lets you construct arbitrary Julia types from Rust, many of which can be instantiated with `Value::instantiate`.


### Calling Rust from Julia

Julia's `ccall` interface can be used to call `extern "C"` functions defined in Rust, for most use cases you shouldn't need jlrs. There are two major ways to use `ccall`, with a pointer to the function or a `(:function, "library")` pair.

A function can be cast to a void pointer and converted to a `Value`:

```rust
use jlrs::prelude::*;
// This function will be provided to Julia as a pointer, so its name can be mangled.
unsafe extern "C" fn call_me(arg: bool) -> isize {
    if arg {
        1
    } else {
        -1
    }
}

fn main() {
let mut julia = unsafe { Julia::init().unwrap() };
julia.scope(|global, frame| {
    // Cast the function to a void pointer
    let call_me_val = Value::new(&mut *frame, call_me as *mut std::ffi::c_void)?;

    // Value::eval_string can be used to create new functions.
    let func = Value::eval_string(
        &mut *frame,
        "myfunc(callme::Ptr{Cvoid})::Int = ccall(callme, Int, (Bool,), true)"
    )?.unwrap();

    // Call the function and unbox the result.
    let output = func.call1(&mut *frame, call_me_val)?
        .into_jlrs_result()?
        .cast::<isize>()?;

    assert_eq!(output, 1);
    
    Ok(())
}).unwrap();
}
```

You can also use functions defined in `dylib` and `cdylib` libraries. In order to create such a library you need to add

```toml
[lib]
crate-type = ["dylib"]
```

or

```toml
[lib]
crate-type = ["cdylib"]
```

respectively to your crate's `Cargo.toml`. Use a `dylib` if you want to use the crate in other Rust crates, but if it's only intended to be called through `ccall` a `cdylib` is the better choice. On Linux, compiling such a crate will be compiled to `lib<crate_name>.so`.

The functions you want to use with `ccall` must be both `extern "C"` functions to ensure the C ABI is used, and annotated with `#[no_mangle]` to prevent name mangling. Julia can find libraries in directories that are either on the default library search path or included by setting the `LD_LIBRARY_PATH` environment variable on Linux. If the compiled library is not directly visible to Julia, you can open it with `Libdl.dlopen` and acquire function pointers with `Libdl.dlsym`. These pointers can be called the same way as the pointer in the previous example.

If the library is visible to Julia you can access it with the library name. If `call_me` is defined in a crate called `foo`, the following should workif the function is annotated with `#[no_mangle]`:

```julia
ccall((:call_me, "libfoo"), Int, (Bool,), false)
```

One important aspect of calling Rust from other languages in general is that panicking across an FFI boundary is undefined behaviour. If you're not sure your code will never panic, wrap it with `std::panic::catch_unwind`.

Most features provided by jlrs including accessing modules, calling functions, and borrowing array data require a `Global` or a frame. You can access these by creating a `CCall` first. Another method provided by `CCall` is `CCall::uv_async_send`, this method can be used in combination with `Base.AsyncCondition`. In particular, it lets you write a `ccall`able function that does its actual work on another thread, return early and `wait` on the async condition, which happens when `CCall::uv_async_send` is called when that work is finished. The advantage of this is that the long-running function will not block the Julia runtime, There's an example available on GitHub that shows how to do this.


### Async runtime

The experimental async runtime runs Julia in a separate thread and allows multiple tasks to run in parallel by offloading functions to a new thread in Julia and waiting for them to complete without blocking the runtime. To use this feature you must enable the `async` feature flag:

```toml
[dependencies]
jlrs = { version = "0.10", features = ["async"] }
```

This features is only supported on Linux.

The struct `AsyncJulia` is exported by the prelude and lets you initialize the runtime in two ways, either as a task or as a thread. The first way should be used if you want to integrate the async runtime into a larger project that uses `async_std`. In order for the runtime to work correctly the `JULIA_NUM_THREADS` environment variable must be set to a value larger than 1.

In order to call Julia with the async runtime you must implement the `JuliaTask` trait. The `run`-method of this trait is similar to the closures that are used in the examples above for the sync runtime; it provides you with a `Global` and an `AsyncGcFrame` which provides mostly the same functionality as `GcFrame`. The `AsyncGcFrame` is required to call `Value::call_async` which calls a Julia function on another thread by using `Base.Threads.@spawn` and returns a `Future`. While awaiting the result the runtime can handle another task. If you don't use `Value::call_async` tasks are executed sequentially.

It's important to keep in mind that allocating memory in Julia uses a lock, so if you execute multiple functions at the same time that allocate new values frequently the performance will drop significantly. The garbage collector can only run when all threads have reached a safepoint, which is the case whenever a function needs to allocate memory. If your function takes a long time to complete but needs to allocate rarely, you should periodically call `GC.safepoint` in Julia to ensure the garbage collector can run.

You can find basic examples in [the examples directory of the repo](https://github.com/Taaitaaiger/jlrs/tree/master/examples).


## Testing

The restriction that Julia can be initialized once must be taken into account when running tests that use `jlrs`. The recommended approach is to create a thread-local static `RefCell`:

```rust
use jlrs::prelude::*;
use std::cell::RefCell;
thread_local! {
    pub static JULIA: RefCell<Julia> = {
        let julia = RefCell::new(unsafe { Julia::init().unwrap() });
        julia.borrow_mut().scope(|_global, _frame| {
            /* include everything you need to use */
            Ok(())
        }).unwrap();
        julia
    };
}
```

Tests that use this construct can only use one thread for testing, so you must use `cargo test -- --test-threads=1`, otherwise the code above will panic when a test tries to call `Julia::init` a second time from another thread.

If these tests also involve the async runtime, the `JULIA_NUM_THREADS` environment variable must be set to a value larger than 1.

If you want to run jlrs's tests, both these requirements must be taken into account: `JULIA_NUM_THREADS=2 cargo test -- --test-threads=1`


## Custom types

In order to map a struct in Rust to one in Julia you can derive `JuliaStruct`. This will implement `Cast`, `JuliaType`, `ValidLayout`, and `JuliaTypecheck` for that type. If the struct in Julia has no type parameters and is a bits type you can also derive `IntoJulia`, which lets you use the type in combination with `Value::new`.

You should not implement these structs manually. The `JlrsReflect.jl` package can generate the correct Rust struct for types that have no tuple or union fields with type parameters. The reason for this restriction is that the layout of tuple and union fields can be very different depending on these parameters in a way that can't be nicely expressed in Rust.

These custom types can also be used when you call Rust from Julia with `ccall`.
