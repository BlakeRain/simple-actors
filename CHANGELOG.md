# Changelog

## simple-actors **0.3.0** _(2024-04-16)_

### 🚨 Breaking Changes

- Added a Rust version constraint of
  [1.75](https://blog.rust-lang.org/2023/12/28/Rust-1.75.0.html).
- Removed the use of [async_trait](https://docs.rs/async-trait/latest/async_trait/) crate.

## simple-actors **0.2.0** _(2024-04-08)_

### 🚨 Breaking Changes

- Spawning actors is now done using a `Context`, and is no longer optional. This was mostly to
  combat issues where actors spawned other actors, which resulted in `JoinHandle`s that were not
  easily tracked. Making the `Context` mandatory to spawn actors helps mitigate the issue.
- The `get_actor_type_name` on a `Recipient` or `WeakRecipient` is no longer available, and the
  `Debug` trait implementations for those types no longer include the actor type name.

### 🛠 Fixes

- Changed the `tokio` dependency from `1.33` to `1.0`
- Updated the `env_logger` dev dependency from `0.10` to `0.11`

### ⚡️ Features

- The `WeakRecipient` type was brought further into parity with the `WeakHandle` type, including the
  `Default` and `Clone` trait implementations and a `WeakRecipient::clear()` method.
