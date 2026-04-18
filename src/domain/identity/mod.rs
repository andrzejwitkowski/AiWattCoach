mod model;
mod ports;
mod service;

pub use model::{
    assign_roles, authorize_admin_access, is_valid_email, normalize_email, AppUser, AuthSession,
    GoogleIdentity, IdentityError, LoginState, Role, WhitelistEntry,
};
pub use ports::{
    BoxFuture, Clock, GoogleOAuthPort, IdGenerator, LoginStateRepository, SessionRepository,
    UserRepository, WhitelistRepository,
};
pub use service::{
    validate_session_ttl_against_current_time, GoogleLoginOutcome, GoogleLoginStart,
    GoogleLoginSuccess, IdentityService, IdentityServiceConfig, IdentityServiceDependencies,
    IdentityUseCases, MAX_BSON_EPOCH_SECONDS,
};
