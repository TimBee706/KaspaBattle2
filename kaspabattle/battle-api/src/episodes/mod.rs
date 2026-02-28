use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
#[allow(dead_code)]
pub trait EpisodeTrait: Sized {
    type Context;
    type Error;

    async fn initialize(ctx: &Self::Context, id: Uuid) -> Result<Self, Self::Error>;
    async fn execute(&mut self) -> Result<(), Self::Error>;
    async fn rollback(&mut self) -> Result<(), Self::Error>;
    async fn poll(&mut self) -> Result<bool, Self::Error>;
}

pub mod match_episode;
