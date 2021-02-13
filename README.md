# PyO3 Asyncio

[![Actions Status](https://github.com/awestlake87/pyo3-asyncio/workflows/CI/badge.svg)](https://github.com/awestlake87/pyo3-asyncio/actions)
[![codecov](https://codecov.io/gh/awestlake87/pyo3-asyncio/branch/master/graph/badge.svg)](https://codecov.io/gh/awestlake87/pyo3-asyncio)
[![crates.io](http://meritbadge.herokuapp.com/pyo3-asyncio)](https://crates.io/crates/pyo3-asyncio)
[![minimum rustc 1.45](https://img.shields.io/badge/rustc-1.45+-blue.svg)](https://rust-lang.github.io/rfcs/2495-min-rust-version.html)

[Rust](http://www.rust-lang.org/) bindings for [Python](https://www.python.org/)'s [Asyncio Library](https://docs.python.org/3/library/asyncio.html). This crate facilitates interactions between Rust Futures and Python Coroutines and manages the lifecycle of their corresponding event loops.

* API Documentation: [stable](https://docs.rs/pyo3-asyncio/) | [master](https://awestlake87.github.io/pyo3-asyncio/master/doc)

* Contributing Notes: [github](https://github.com/awestlake87/pyo3-asyncio/blob/master/Contributing.md)

> PyO3 Asyncio is a _brand new_ part of the broader PyO3 ecosystem. Feel free to open any issues for feature requests or bugfixes for this crate.

## Known Problems

This library can give spurious failures during finalization prior to PyO3 release `v0.13.2`. Make sure your PyO3 dependency is up-to-date!

## Quickstart

Here we initialize the runtime, import Python's `asyncio` library and run the given future to completion using Python's default `EventLoop` and `async-std`. Inside the future, we convert `asyncio` sleep into a Rust future and await it.

More details on the usage of this library can be found in the [API docs](https://awestlake87.github.io/pyo3-asyncio/master/doc).

```rust
use pyo3::prelude::*;

#[pyo3_asyncio::async_std::main]
async fn main() -> PyResult<()> {
    let fut = Python::with_gil(|py| {
        let asyncio = py.import("asyncio")?;

        // convert asyncio.sleep into a Rust Future
        pyo3_asyncio::into_future(asyncio.call_method1("sleep", (1.into_py(py),))?)
    })?;

    fut.await?;

    Ok(())
}
```