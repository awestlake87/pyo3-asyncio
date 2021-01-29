#![forbid(unsafe_code, future_incompatible, rust_2018_idioms)]
#![deny(missing_debug_implementations, nonstandard_style)]
#![recursion_limit = "512"]

mod tokio;

use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn::{
    parse::{Parse, ParseStream, Result},
    spanned::Spanned,
    Attribute,
};

/// Enables an async main function that uses the async-std runtime.
///
/// # Examples
///
/// ```ignore
/// #[pyo3_asyncio::async_std::main]
/// async fn main() -> PyResult<()> {
///     Ok(())
/// }
/// ```
#[cfg(not(test))] // NOTE: exporting main breaks tests, we should file an issue.
#[proc_macro_attribute]
pub fn async_std_main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);

    let ret = &input.sig.output;
    let inputs = &input.sig.inputs;
    let name = &input.sig.ident;
    let body = &input.block;
    let attrs = &input.attrs;
    let vis = &input.vis;

    if name != "main" {
        return TokenStream::from(quote_spanned! { name.span() =>
            compile_error!("only the main function can be tagged with #[async_std::main]"),
        });
    }

    if input.sig.asyncness.is_none() {
        return TokenStream::from(quote_spanned! { input.span() =>
            compile_error!("the async keyword is missing from the function declaration"),
        });
    }

    let result = quote! {
        #vis fn main() {
            #(#attrs)*
            async fn main(#inputs) #ret {
                #body
            }

            pyo3::Python::with_gil(|py| {
                pyo3_asyncio::with_runtime(py, || {
                    pyo3_asyncio::async_std::run_until_complete(py, main())?;

                    Ok(())
                })
                .map_err(|e| {
                    e.print_and_set_sys_last_vars(py);
                })
                .unwrap();
            });
        }
    };

    result.into()
}

/// Enables an async main function that uses the tokio runtime.
///
/// # Arguments
/// * `flavor` - selects the type of tokio runtime ["multi_thread", "current_thread"]
/// * `worker_threads` - number of worker threads, defaults to the number of CPUs on the system
///
/// # Examples
///
/// Default configuration:
/// ```ignore
/// #[pyo3_asyncio::tokio::main]
/// async fn main() -> PyResult<()> {
///     Ok(())
/// }
/// ```
///
/// Current-thread scheduler:
/// ```ignore
/// #[pyo3_asyncio::tokio::main(flavor = "current_thread")]
/// async fn main() -> PyResult<()> {
///     Ok(())
/// }
/// ```
///
/// Multi-thread scheduler with custom worker thread count:
/// ```ignore
/// #[pyo3_asyncio::tokio::main(flavor = "multi_thread", worker_threads = 10)]
/// async fn main() -> PyResult<()> {
///     Ok(())
/// }
/// ```
#[cfg(not(test))] // NOTE: exporting main breaks tests, we should file an issue.
#[proc_macro_attribute]
pub fn tokio_main(args: TokenStream, item: TokenStream) -> TokenStream {
    tokio::main(args, item, true)
}

/// Registers an `async-std` test with the `pyo3-asyncio` test harness.
///
/// This attribute is meant to mirror the `#[test]` attribute and allow you to mark a function for
/// testing within an integration test. Like the `#[async_std::test]` attribute, it will accept
/// `async` test functions, but it will also accept blocking functions as well.
///
/// # Examples
/// ```ignore
/// use std::{time::Duration, thread};
///
/// use pyo3::prelude::*;
///
/// // async test function
/// #[pyo3_asyncio::async_std::test]
/// async fn test_async_sleep() -> PyResult<()> {
///     async_std::task::sleep(Duration::from_secs(1)).await;
///     Ok(())
/// }
///
/// // blocking test function
/// #[pyo3_asyncio::async_std::test]
/// fn test_blocking_sleep() -> PyResult<()> {
///     thread::sleep(Duration::from_secs(1));
///     Ok(())
/// }
/// ```
#[cfg(not(test))] // NOTE: exporting main breaks tests, we should file an issue.
#[proc_macro_attribute]
pub fn async_std_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);

    let sig = &input.sig;
    let name = &input.sig.ident;
    let body = &input.block;
    let vis = &input.vis;

    let fn_impl = if input.sig.asyncness.is_none() {
        quote! {
            #vis fn #name() -> std::pin::Pin<Box<dyn std::future::Future<Output = pyo3::PyResult<()>> + Send>> {
                #sig {
                    #body
                }

                Box::pin(async_std::task::spawn_blocking(move || {
                    #name()
                }))
            }
        }
    } else {
        quote! {
            #vis fn #name() -> std::pin::Pin<Box<dyn std::future::Future<Output = pyo3::PyResult<()>> + Send>> {
                #sig {
                    #body
                }

                Box::pin(#name())
            }
        }
    };

    let result = quote! {
        #fn_impl

        pyo3_asyncio::inventory::submit! {
            #![crate = pyo3_asyncio] {
                pyo3_asyncio::testing::Test {
                    name: format!("{}::{}", std::module_path!(), stringify!(#name)),
                    test_fn: &#name
                }
            }
        }
    };

    result.into()
}

/// Registers a `tokio` test with the `pyo3-asyncio` test harness.
///
/// This attribute is meant to mirror the `#[test]` attribute and allow you to mark a function for
/// testing within an integration test. Like the `#[tokio::test]` attribute, it will accept `async`
/// test functions, but it will also accept blocking functions as well.
///
/// # Examples
/// ```ignore
/// use std::{time::Duration, thread};
///
/// use pyo3::prelude::*;
///
/// // async test function
/// #[pyo3_asyncio::tokio::test]
/// async fn test_async_sleep() -> PyResult<()> {
///     tokio::time::sleep(Duration::from_secs(1)).await;
///     Ok(())
/// }
///
/// // blocking test function
/// #[pyo3_asyncio::tokio::test]
/// fn test_blocking_sleep() -> PyResult<()> {
///     thread::sleep(Duration::from_secs(1));
///     Ok(())
/// }
/// ```
#[cfg(not(test))] // NOTE: exporting main breaks tests, we should file an issue.
#[proc_macro_attribute]
pub fn tokio_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);

    let sig = &input.sig;
    let name = &input.sig.ident;
    let body = &input.block;
    let vis = &input.vis;

    let fn_impl = if input.sig.asyncness.is_none() {
        quote! {
            #vis fn #name() -> std::pin::Pin<Box<dyn std::future::Future<Output = pyo3::PyResult<()>> + Send>> {
                #sig {
                    #body
                }

                Box::pin(async {
                    match pyo3_asyncio::tokio::get_runtime().spawn_blocking(&#name).await {
                        Ok(result) => result,
                        Err(e) => {
                            assert!(e.is_panic());
                            Err(pyo3::exceptions::PyException::new_err("rust future panicked"))
                        }
                    }
                })
            }
        }
    } else {
        quote! {
            #vis fn #name() -> std::pin::Pin<Box<dyn std::future::Future<Output = pyo3::PyResult<()>> + Send>> {
                #sig {
                    #body
                }

                Box::pin(#name())
            }
        }
    };

    let result = quote! {
        #fn_impl

        pyo3_asyncio::inventory::submit! {
            #![crate = pyo3_asyncio] {
                pyo3_asyncio::testing::Test {
                    name: format!("{}::{}", std::module_path!(), stringify!(#name)),
                    test_fn: &#name
                }
            }
        }
    };

    result.into()
}

enum Item {
    Attribute(Vec<Attribute>),
    String(syn::LitStr),
}

impl Parse for Item {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let lookahead = input.lookahead1();

        if lookahead.peek(syn::Token![#]) {
            Attribute::parse_outer(input).map(Item::Attribute)
        } else {
            input.parse().map(Item::String)
        }
    }
}

struct TestMainArgs {
    attrs: Vec<Attribute>,
    suite_name: String,
}

impl Parse for TestMainArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut args: syn::punctuated::Punctuated<Item, syn::Token![,]> =
            input.parse_terminated(Item::parse)?;

        let suite_name = match args.pop().unwrap() {
            syn::punctuated::Pair::Punctuated(Item::String(s), _)
            | syn::punctuated::Pair::End(Item::String(s)) => s.value(),
            _ => panic!(),
        };

        let attrs = match args.pop().unwrap() {
            syn::punctuated::Pair::Punctuated(Item::Attribute(attrs), _) => attrs,
            _ => panic!(),
        };

        Ok(Self { attrs, suite_name })
    }
}

/// The standard `pyo3-asyncio` test harness `main` fn boilerplate.
///
/// This macro provides the full boilerplate for the `pyo3-asyncio` test harness in one line.
///
/// # Examples
///
/// `test_main!` for the `tokio` runtime:
/// ```ignore
/// # #[cfg(all(feature = "tokio-runtime", feature = "attributes"))]
/// pyo3_asyncio::testing::test_main!(#[pyo3_asyncio::tokio::main], "Example Test Suite");
/// ```
///
/// `test_main!` for the `async-std` runtime:
/// ```ignore
/// # #[cfg(all(feature = "async-std-runtime", feature = "attributes"))]
/// pyo3_asyncio::testing::test_main!(#[pyo3_asyncio::async_std::main], "Example Test Suite");
/// ```
#[cfg(not(test))]
#[proc_macro]
pub fn test_main(args: TokenStream) -> TokenStream {
    let TestMainArgs { attrs, suite_name } = syn::parse_macro_input!(args as TestMainArgs);

    let result = quote! {
        #(#attrs)*
        async fn main() -> pyo3::PyResult<()> {
            pyo3_asyncio::testing::main(#suite_name).await
        }
    };
    result.into()
}
