# Git Hooks (Recommended)

Using the project's githooks are recommended to prevent CI from failing for trivial reasons such as build checks or integration tests failing. Enabling the hooks should run `cargo check --all-targets` for every commit and `cargo test` for every push.

```
git config core.hookspath .githooks
```