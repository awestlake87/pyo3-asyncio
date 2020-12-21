# Overview

This project contains an example of Python 3 asyncio and Tokio interop. It's
designed to allow Python complete control over the main thread and facilitate
interactions between rust and python coroutines.

In addition, the `test` module contains a primitive test harness that will run
a series of rust tests. We cannot use the default test harness because it would
not allow Python to have control over the main thread. Instead we convert our
test harness to an `asyncio.Future` and tell the Python `asyncio` event loop to
drive the future to completion.

# Running the Test

You can run the example test the same way you'd run any other Cargo integration
test.

```
$ cargo test
```