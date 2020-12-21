# Overview

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

# Running the Test

You can run the example test the same way you'd run any other Cargo integration
test.

```
$ cargo test
```