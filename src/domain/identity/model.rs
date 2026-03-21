#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Admin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityError {
    EmailNotVerified,
    InvalidLoginState,
    Forbidden,
    Repository(String),
    External(String),
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmailNotVerified => write!(f, "Google account email must be verified"),
            Self::InvalidLoginState => write!(f, "Login state is invalid or expired"),
            Self::Forbidden => write!(f, "User does not have the required role"),
            Self::Repository(message) | Self::External(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for IdentityError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppUser {
    pub id: String,
    pub google_subject: String,
    pub email: String,
    pub email_normalized: String,
    pub roles: Vec<Role>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub email_verified: bool,
}

impl AppUser {
    pub fn new(
        id: String,
        google_subject: String,
        email: String,
        roles: Vec<Role>,
        display_name: Option<String>,
        avatar_url: Option<String>,
        email_verified: bool,
    ) -> Self {
        Self {
            id,
            google_subject,
            email_normalized: normalize_email(&email),
            email,
            roles,
            display_name,
            avatar_url,
            email_verified,
        }
    }

    pub fn is_admin(&self) -> bool {
        self.roles.contains(&Role::Admin)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoogleIdentity {
    pub subject: String,
    pub email: String,
    pub email_normalized: String,
    pub email_verified: bool,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl GoogleIdentity {
    pub fn new(
        subject: &str,
        email: &str,
        email_verified: bool,
        display_name: Option<String>,
        avatar_url: Option<String>,
    ) -> Result<Self, IdentityError> {
        if !email_verified {
            return Err(IdentityError::EmailNotVerified);
        }

        Ok(Self {
            subject: subject.to_string(),
            email: email.to_string(),
            email_normalized: normalize_email(email),
            email_verified,
            display_name,
            avatar_url,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthSession {
    pub id: String,
    pub user_id: String,
    pub expires_at_epoch_seconds: i64,
    pub created_at_epoch_seconds: i64,
}

impl AuthSession {
    pub fn new(
        id: String,
        user_id: String,
        expires_at_epoch_seconds: i64,
        created_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            id,
            user_id,
            expires_at_epoch_seconds,
            created_at_epoch_seconds,
        }
    }

    pub fn is_expired(&self, now_epoch_seconds: i64) -> bool {
        self.expires_at_epoch_seconds <= now_epoch_seconds
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginState {
    pub id: String,
    pub return_to: Option<String>,
    pub expires_at_epoch_seconds: i64,
    pub created_at_epoch_seconds: i64,
}

impl LoginState {
    pub fn new(
        id: String,
        return_to: Option<String>,
        expires_at_epoch_seconds: i64,
        created_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            id,
            return_to,
            expires_at_epoch_seconds,
            created_at_epoch_seconds,
        }
    }

    pub fn is_expired(&self, now_epoch_seconds: i64) -> bool {
        self.expires_at_epoch_seconds <= now_epoch_seconds
    }
}

pub fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

pub fn assign_roles(email: &str, admin_emails: &[String]) -> Vec<Role> {
    let normalized_email = normalize_email(email);
    let mut roles = vec![Role::User];

    if admin_emails
        .iter()
        .any(|admin_email| normalize_email(admin_email) == normalized_email)
    {
        roles.push(Role::Admin);
    }

    roles
}

pub fn authorize_admin_access(user: &AppUser) -> Result<(), IdentityError> {
    if user.is_admin() {
        Ok(())
    } else {
        Err(IdentityError::Forbidden)
    }
}
