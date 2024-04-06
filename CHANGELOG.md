# Changelog

## simple-actors **0.0.2** *(2024-04-08)*

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
