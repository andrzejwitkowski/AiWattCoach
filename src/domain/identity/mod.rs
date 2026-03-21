mod model;
mod ports;
mod service;

pub use model::{
    assign_roles, authorize_admin_access, normalize_email, AppUser, AuthSession, GoogleIdentity,
    IdentityError, LoginState, Role,
};
pub use ports::{
    BoxFuture, Clock, GoogleOAuthPort, IdGenerator, LoginStateRepository, SessionRepository,
    UserRepository,
};
pub use service::{
    validate_session_ttl_against_current_time, GoogleLoginStart, GoogleLoginSuccess,
    IdentityService, IdentityServiceConfig, IdentityUseCases, MAX_BSON_EPOCH_SECONDS,
};
