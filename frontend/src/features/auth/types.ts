export type AppRole = 'user' | 'admin';

export type CurrentUser = {
  id: string;
  email: string;
  displayName: string | null;
  avatarUrl: string | null;
  roles: AppRole[];
};

export type CurrentUserResponse =
  | { authenticated: false }
  | { authenticated: true; user: CurrentUser };

export type JoinWhitelistResponse = {
  success: boolean;
};

export type AuthStatus = 'loading' | 'authenticated' | 'unauthenticated';
