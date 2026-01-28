pub mod storage;

pub use storage::Database;

pub async fn init_database() -> crate::core::errors::Result<Database> {
    Database::new("sqlite:onchain_beast.db").await
}
