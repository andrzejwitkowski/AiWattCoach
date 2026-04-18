import { z } from 'zod';

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

export const JoinWhitelistResponseSchema = z.object({
  success: z.boolean()
});

export type JoinWhitelistResponse = z.infer<typeof JoinWhitelistResponseSchema>;

export type AuthStatus = 'loading' | 'authenticated' | 'unauthenticated';
