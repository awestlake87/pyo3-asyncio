# PyO3 Asyncio

[![Actions Status](https://github.com/awestlake87/pyo3-asyncio/workflows/CI/badge.svg)](https://github.com/awestlake87/pyo3-asyncio/actions)
[![codecov](https://codecov.io/gh/awestlake87/pyo3-asyncio/branch/master/graph/badge.svg)](https://codecov.io/gh/awestlake87/pyo3-asyncio)
[![crates.io](https://img.shields.io/crates/v/pyo3-asyncio)](https://crates.io/crates/pyo3-asyncio)
[![minimum rustc 1.46](https://img.shields.io/badge/rustc-1.46+-blue.svg)](https://rust-lang.github.io/rfcs/2495-min-rust-version.html)

[Rust](http://www.rust-lang.org/) bindings for [Python](https://www.python.org/)'s [Asyncio Library](https://docs.python.org/3/library/asyncio.html). This crate facilitates interactions between Rust Futures and Python Coroutines and manages the lifecycle of their corresponding event loops.

* PyO3 Project: [Homepage](https://pyo3.rs/) | [GitHub](https://github.com/PyO3/pyo3)

* PyO3 Asyncio API Documentation: [stable](https://docs.rs/pyo3-asyncio/) | [master](https://awestlake87.github.io/pyo3-asyncio/master/doc)

* Guide for Async / Await [stable](https://pyo3.rs/v0.13.2/ecosystem/async-await.html) | [main](https://pyo3.rs/main/ecosystem/async-await.html)

* Contributing Notes: [github](https://github.com/awestlake87/pyo3-asyncio/blob/master/Contributing.md)

> PyO3 Asyncio is a _brand new_ part of the broader PyO3 ecosystem. Feel free to open any issues for feature requests or bugfixes for this crate.

## Known Problems

This library can give spurious failures during finalization prior to PyO3 release `v0.13.2`. Make sure your PyO3 dependency is up-to-date!

## Quickstart

### Rust Applications
Here we initialize the runtime, import Python's `asyncio` library and run the given future to completion using Python's default `EventLoop` and `async-std`. Inside the future, we convert `asyncio` sleep into a Rust future and await it.


```toml
# Cargo.toml dependencies
[dependencies]
pyo3 = { version = "0.13" }
pyo3-asyncio = { version = "0.13", features = ["attributes", "async-std-runtime"] }
async-std = "1.9"
```

```rust
//! main.rs

use pyo3::prelude::*;

#[pyo3_asyncio::async_std::main]
async fn main() -> PyResult<()> {
    let fut = Python::with_gil(|py| {
        let asyncio = py.import("asyncio")?;
        // convert asyncio.sleep into a Rust Future
        pyo3_asyncio::async_std::into_future(asyncio.call_method1("sleep", (1.into_py(py),))?)
    })?;

    fut.await?;

    Ok(())
}
```

The same application can be written to use `tokio` instead using the `#[pyo3_asyncio::tokio::main]`
attribute.

```toml
# Cargo.toml dependencies
[dependencies]
pyo3 = { version = "0.13" }
pyo3-asyncio = { version = "0.13", features = ["attributes", "tokio-runtime"] }
tokio = "1.4"
```

```rust
//! main.rs

use pyo3::prelude::*;

#[pyo3_asyncio::tokio::main]
async fn main() -> PyResult<()> {
    let fut = Python::with_gil(|py| {
        let asyncio = py.import("asyncio")?;
        // convert asyncio.sleep into a Rust Future
        pyo3_asyncio::tokio::into_future(asyncio.call_method1("sleep", (1.into_py(py),))?)
    })?;

    fut.await?;

    Ok(())
}
```

More details on the usage of this library can be found in the [API docs](https://awestlake87.github.io/pyo3-asyncio/master/doc).

### PyO3 Native Rust Modules

PyO3 Asyncio can also be used to write native modules with async functions.

Add the `[lib]` section to `Cargo.toml` to make your library a `cdylib` that Python can import.
```toml
[lib]
name = "my_async_module"
crate-type = ["cdylib"]
```

Make your project depend on `pyo3` with the `extension-module` feature enabled and select your
`pyo3-asyncio` runtime:

For `async-std`:
```toml
[dependencies]
pyo3 = { version = "0.13", features = ["extension-module"] }
pyo3-asyncio = { version = "0.13", features = ["async-std-runtime"] }
async-std = "1.9"
```

For `tokio`:
```toml
[dependencies]
pyo3 = { version = "0.13", features = ["extension-module"] }
pyo3-asyncio = { version = "0.13", features = ["tokio-runtime"] }
tokio = "1.4"
```

Export an async function that makes use of `async-std`:

```rust
//! lib.rs

use pyo3::{prelude::*, wrap_pyfunction};

#[pyfunction]
fn rust_sleep(py: Python) -> PyResult<&PyAny> {
    pyo3_asyncio::async_std::future_into_py(py, async {
        async_std::task::sleep(std::time::Duration::from_secs(1)).await;
        Ok(Python::with_gil(|py| py.None()))
    })
}

#[pymodule]
fn my_async_module(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(rust_sleep, m)?)?;

    Ok(())
}

```

If you want to use `tokio` instead, here's what your module should look like:

```rust
//! lib.rs

use pyo3::{prelude::*, wrap_pyfunction};

#[pyfunction]
fn rust_sleep(py: Python) -> PyResult<&PyAny> {
    pyo3_asyncio::tokio::future_into_py(py, async {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Ok(Python::with_gil(|py| py.None()))
    })
}

#[pymodule]
fn my_async_module(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(rust_sleep, m)?)?;

    Ok(())
}

```

Build your module and rename `libmy_async_module.so` to `my_async_module.so`
```bash
cargo build --release && mv target/release/libmy_async_module.so target/release/my_async_module.so
```

Now, point your `PYTHONPATH` to the directory containing `my_async_module.so`, then you'll be able 
to import and use it:

```bash
$ PYTHONPATH=target/release python3
Python 3.8.5 (default, Jan 27 2021, 15:41:15) 
[GCC 9.3.0] on linux
Type "help", "copyright", "credits" or "license" for more information.
>>> import asyncio
>>> from my_async_module import rust_sleep
>>> 
>>> # should sleep for 1s
>>> asyncio.get_event_loop().run_until_complete(rust_sleep())
>>>
```

> Note that we are using `EventLoop.run_until_complete` here instead of the newer `asyncio.run`. That is because `asyncio.run` will set up its own internal event loop that `pyo3_asyncio` will not be aware of. For this reason, running `pyo3_asyncio` conversions through `asyncio.run` is not currently supported.
> 
> This restriction may be lifted in a future release.

## PyO3 Asyncio Primer

If you are working with a Python library that makes use of async functions or wish to provide 
Python bindings for an async Rust library, [`pyo3-asyncio`](https://github.com/awestlake87/pyo3-asyncio)
likely has the tools you need. It provides conversions between async functions in both Python and 
Rust and was designed with first-class support for popular Rust runtimes such as 
[`tokio`](https://tokio.rs/) and [`async-std`](https://async.rs/). In addition, all async Python 
code runs on the default `asyncio` event loop, so `pyo3-asyncio` should work just fine with existing 
Python libraries.

In the following sections, we'll give a general overview of `pyo3-asyncio` explaining how to call 
async Python functions with PyO3, how to call async Rust functions from Python, and how to configure
your codebase to manage the runtimes of both.

## Awaiting an Async Python Function in Rust

Let's take a look at a dead simple async Python function:

```python
# Sleep for 1 second
async def py_sleep():
    await asyncio.sleep(1)
```

**Async functions in Python are simply functions that return a `coroutine` object**. For our purposes, 
we really don't need to know much about these `coroutine` objects. The key factor here is that calling
an `async` function is _just like calling a regular function_, the only difference is that we have
to do something special with the object that it returns.

Normally in Python, that something special is the `await` keyword, but in order to await this 
coroutine in Rust, we first need to convert it into Rust's version of a `coroutine`: a `Future`. 
That's where `pyo3-asyncio` comes in. 
[`pyo3_asyncio::into_future`](https://docs.rs/pyo3-asyncio/latest/pyo3_asyncio/fn.into_future.html) 
performs this conversion for us:


```rust no_run
use pyo3::prelude::*;

#[pyo3_asyncio::tokio::main]
async fn main() -> PyResult<()> {
    let future = Python::with_gil(|py| -> PyResult<_> {
        // import the module containing the py_sleep function
        let example = py.import("example")?;

        // calling the py_sleep method like a normal function 
        // returns a coroutine
        let coroutine = example.call_method0("py_sleep")?;

        // convert the coroutine into a Rust future using the 
        // tokio runtime
        pyo3_asyncio::tokio::into_future(coroutine)
    })?;

    // await the future
    future.await?;

    Ok(())
}
```

> If you're interested in learning more about `coroutines` and `awaitables` in general, check out the 
> [Python 3 `asyncio` docs](https://docs.python.org/3/library/asyncio-task.html) for more information.

## Awaiting a Rust Future in Python

Here we have the same async function as before written in Rust using the 
[`async-std`](https://async.rs/) runtime:

```rust
/// Sleep for 1 second
async fn rust_sleep() {
    async_std::task::sleep(std::time::Duration::from_secs(1)).await;
}
```

Similar to Python, Rust's async functions also return a special object called a
`Future`:

```rust compile_fail
let future = rust_sleep();
```

We can convert this `Future` object into Python to make it `awaitable`. This tells Python that you 
can use the `await` keyword with it. In order to do this, we'll call 
[`pyo3_asyncio::async_std::future_into_py`](https://docs.rs/pyo3-asyncio/latest/pyo3_asyncio/async_std/fn.future_into_py.html):

```rust
use pyo3::prelude::*;

async fn rust_sleep() {
    async_std::task::sleep(std::time::Duration::from_secs(1)).await;
}

#[pyfunction]
fn call_rust_sleep(py: Python) -> PyResult<&PyAny> {
    pyo3_asyncio::async_std::future_into_py(py, async move {
        rust_sleep().await;
        Ok(Python::with_gil(|py| py.None()))
    })
}
```

In Python, we can call this pyo3 function just like any other async function:

```python
from example import call_rust_sleep

async def rust_sleep():
    await call_rust_sleep()
```

## Managing Event Loops

Python's event loop requires some special treatment, especially regarding the main thread. Some of
Python's `asyncio` features, like proper signal handling, require control over the main thread, which
doesn't always play well with Rust.

Luckily, Rust's event loops are pretty flexible and don't _need_ control over the main thread, so in
`pyo3-asyncio`, we decided the best way to handle Rust/Python interop was to just surrender the main
thread to Python and run Rust's event loops in the background. Unfortunately, since most event loop 
implementations _prefer_ control over the main thread, this can still make some things awkward.

### PyO3 Asyncio Initialization

Because Python needs to control the main thread, we can't use the convenient proc macros from Rust
runtimes to handle the `main` function or `#[test]` functions. Instead, the initialization for PyO3 has to be done from the `main` function and the main 
thread must block on [`pyo3_asyncio::run_forever`](https://docs.rs/pyo3-asyncio/latest/pyo3_asyncio/fn.run_forever.html) or [`pyo3_asyncio::async_std::run_until_complete`](https://docs.rs/pyo3-asyncio/latest/pyo3_asyncio/async_std/fn.run_until_complete.html).

Because we have to block on one of those functions, we can't use [`#[async_std::main]`](https://docs.rs/async-std/latest/async_std/attr.main.html) or [`#[tokio::main]`](https://docs.rs/tokio/1.1.0/tokio/attr.main.html)
since it's not a good idea to make long blocking calls during an async function.

> Internally, these `#[main]` proc macros are expanded to something like this:
> ```rust compile_fail
> fn main() {
>     // your async main fn
>     async fn _main_impl() { /* ... */ }
>     Runtime::new().block_on(_main_impl());   
> }
> ```
> Making a long blocking call inside the `Future` that's being driven by `block_on` prevents that
> thread from doing anything else and can spell trouble for some runtimes (also this will actually 
> deadlock a single-threaded runtime!). Many runtimes have some sort of `spawn_blocking` mechanism 
> that can avoid this problem, but again that's not something we can use here since we need it to 
> block on the _main_ thread.

For this reason, `pyo3-asyncio` provides its own set of proc macros to provide you with this 
initialization. These macros are intended to mirror the initialization of `async-std` and `tokio` 
while also satisfying the Python runtime's needs.

Here's a full example of PyO3 initialization with the `async-std` runtime:
```rust no_run
use pyo3::prelude::*;

#[pyo3_asyncio::async_std::main]
async fn main() -> PyResult<()> {
    // PyO3 is initialized - Ready to go

    let fut = Python::with_gil(|py| -> PyResult<_> {
        let asyncio = py.import("asyncio")?;

        // convert asyncio.sleep into a Rust Future
        pyo3_asyncio::async_std::into_future(
            asyncio.call_method1("sleep", (1.into_py(py),))?
        )
    })?;

    fut.await?;

    Ok(())
}
```

## PyO3 Asyncio in Cargo Tests

The default Cargo Test harness does not currently allow test crates to provide their own `main` 
function, so there doesn't seem to be a good way to allow Python to gain control over the main
thread.

We can, however, override the default test harness and provide our own. `pyo3-asyncio` provides some
utilities to help us do just that! In the following sections, we will provide an overview for 
constructing a Cargo integration test with `pyo3-asyncio` and adding your tests to it.

### Main Test File
First, we need to create the test's main file. Although these tests are considered integration
tests, we cannot put them in the `tests` directory since that is a special directory owned by
Cargo. Instead, we put our tests in a `pytests` directory.

> The name `pytests` is just a convention. You can name this folder anything you want in your own
> projects.

We'll also want to provide the test's main function. Most of the functionality that the test harness needs is packed in the [`pyo3_asyncio::testing::main`](https://docs.rs/pyo3-asyncio/latest/pyo3_asyncio/testing/fn.main.html) function. This function will parse the test's CLI arguments, collect and pass the functions marked with [`#[pyo3_asyncio::async_std::test]`](https://docs.rs/pyo3-asyncio/latest/pyo3_asyncio/async_std/attr.test.html) or [`#[pyo3_asyncio::tokio::test]`](https://docs.rs/pyo3-asyncio/latest/pyo3_asyncio/tokio/attr.test.html) and pass them into the test harness for running and filtering.

`pytests/test_example.rs` for the `tokio` runtime:
```rust
#[pyo3_asyncio::tokio::main]
async fn main() -> pyo3::PyResult<()> {
    pyo3_asyncio::testing::main().await
}
```

`pytests/test_example.rs` for the `async-std` runtime:
```rust
#[pyo3_asyncio::async_std::main]
async fn main() -> pyo3::PyResult<()> {
    pyo3_asyncio::testing::main().await
}
```

### Cargo Configuration
Next, we need to add our test file to the Cargo manifest by adding the following section to the
`Cargo.toml`

```toml
[[test]]
name = "test_example"
path = "pytests/test_example.rs"
harness = false
```

Also add the `testing` and `attributes` features to the `pyo3-asyncio` dependency and select your preferred runtime:

```toml
pyo3-asyncio = { version = "0.13", features = ["testing", "attributes", "async-std-runtime"] }
```

At this point, you should be able to run the test via `cargo test`

### Adding Tests to the PyO3 Asyncio Test Harness

We can add tests anywhere in the test crate with the runtime's corresponding `#[test]` attribute:

For `async-std` use the [`pyo3_asyncio::async_std::test`](https://docs.rs/pyo3-asyncio/latest/pyo3_asyncio/async_std/attr.test.html) attribute:
```rust
mod tests {
    use std::{time::Duration, thread};

    use pyo3::prelude::*;

    // tests can be async
    #[pyo3_asyncio::async_std::test]
    async fn test_async_sleep() -> PyResult<()> {
        async_std::task::sleep(Duration::from_secs(1)).await;
        Ok(())
    }

    // they can also be synchronous
    #[pyo3_asyncio::async_std::test]
    fn test_blocking_sleep() -> PyResult<()> {
        thread::sleep(Duration::from_secs(1));
        Ok(())
    }
}

#[pyo3_asyncio::async_std::main]
async fn main() -> pyo3::PyResult<()> {
    pyo3_asyncio::testing::main().await
}
```

For `tokio` use the [`pyo3_asyncio::tokio::test`](https://docs.rs/pyo3-asyncio/latest/pyo3_asyncio/tokio/attr.test.html) attribute:
```rust
mod tests {
    use std::{time::Duration, thread};

    use pyo3::prelude::*;

    // tests can be async
    #[pyo3_asyncio::tokio::test]
    async fn test_async_sleep() -> PyResult<()> {
        tokio::time::sleep(Duration::from_secs(1)).await;
        Ok(())
    }

    // they can also be synchronous
    #[pyo3_asyncio::tokio::test]
    fn test_blocking_sleep() -> PyResult<()> {
        thread::sleep(Duration::from_secs(1));
        Ok(())
    }
}

#[pyo3_asyncio::tokio::main]
async fn main() -> pyo3::PyResult<()> {
    pyo3_asyncio::testing::main().await
}
```

## MSRV
Currently the MSRV for this library is 1.46.0, _but_ if you don't need to use the `async-std-runtime`
feature, you can use rust 1.45.0. 
> `async-std` depends on `socket2` which fails to compile under 1.45.0.
