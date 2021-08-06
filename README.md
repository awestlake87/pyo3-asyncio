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

## MSRV
Currently the MSRV for this library is 1.46.0, _but_ if you don't need to use the `async-std-runtime`
feature, you can use rust 1.45.0. 
> `async-std` depends on `socket2` which fails to compile under 1.45.0.