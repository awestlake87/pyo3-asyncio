# PyO3 Asyncio

[![Actions Status](https://github.com/awestlake87/pyo3-asyncio/workflows/CI/badge.svg)](https://github.com/awestlake87/pyo3-asyncio/actions)
[![codecov](https://codecov.io/gh/awestlake87/pyo3-asyncio/branch/master/graph/badge.svg)](https://codecov.io/gh/awestlake87/pyo3-asyncio)
[![crates.io](http://meritbadge.herokuapp.com/pyo3-asyncio)](https://crates.io/crates/pyo3-asyncio)
[![minimum rustc 1.45](https://img.shields.io/badge/rustc-1.45+-blue.svg)](https://rust-lang.github.io/rfcs/2495-min-rust-version.html)

[Rust](http://www.rust-lang.org/) bindings for [Python](https://www.python.org/). This includes running and interacting with Python code from a Rust binary, as well as writing native Python modules.

* API Documentation: [stable](https://docs.rs/pyo3-asyncio/)

* Contributing Notes: [github](https://github.com/awestlake87/pyo3-asyncio/blob/master/Contributing.md)

## Overview

This project contains an example of Python 3 asyncio and Tokio interop. It's
designed to allow Python complete control over the main thread and facilitate
interactions between rust and python coroutines.

In addition, the `test` module contains a primitive test harness that will run
a series of rust tests. We cannot use the default test harness because it would
not allow Python to have control over the main thread. Instead we convert our
test harness to an `asyncio.Future` and tell the Python `asyncio` event loop to
drive the future to completion.

Allowing Python to have control over the main thread also allows signals to work
properly. CTRL-C will exit the event loop just as you would expect.

Rust futures can be converted into python coroutines and vice versa. When a
python coroutine raises an exception, the error should be propagated back with
a `PyResult` as expected. Panics in Rust are translated into python Exceptions
to propagate through the Python layers. This allows panics in tests to exit the
event loop like before, but may cause unexpected behaviour if a Rust future
awaits a Python coroutine that awaits a Rust future that panics.

> These scenarios are currently untested in this crate.

## Running the Test

You can run the example test the same way you'd run any other Cargo integration
test.

```
$ cargo test
```