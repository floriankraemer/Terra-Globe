//! In-memory test doubles for every port. They live in the library (not
//! behind `cfg(test)`) so downstream crates can use them in their own tests.

pub mod clock;
pub mod dialogs;
pub mod files;
pub mod http;
pub mod ids;
pub mod in_memory_repository;
pub mod preferences;
pub mod secrets;

pub use clock::FakeClock;
pub use dialogs::FakeFileDialogs;
pub use files::InMemoryFileIo;
pub use http::FakeHttpClient;
pub use ids::SequentialIdGenerator;
pub use in_memory_repository::InMemoryPlacesRepository;
pub use preferences::InMemoryPreferencesStore;
pub use secrets::InMemorySecretStore;
