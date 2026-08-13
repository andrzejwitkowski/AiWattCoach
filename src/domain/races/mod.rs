mod authoritative;
mod model;
mod ports;
mod projection_cleanup;
mod service;
#[cfg(test)]
mod tests;

pub use authoritative::AuthoritativeRaceRepository;
pub use model::{
    imported_intervals_race_id, CreateRace, Race, RaceDiscipline, RaceError, RacePriority,
    RaceResult, UpdateRace,
};
pub use ports::{BoxFuture, RaceRepository, RaceUseCases};
pub use projection_cleanup::{NoopRaceProjectionCleanup, RaceProjectionCleanupPort};
pub use service::RaceService;
