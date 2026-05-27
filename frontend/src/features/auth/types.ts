import { z } from 'zod';

export const AppRoleSchema = z.enum(['user', 'admin']);
export type AppRole = z.infer<typeof AppRoleSchema>;

export const CurrentUserSchema = z.object({
  id: z.string(),
  email: z.string(),
  displayName: z.string().nullable(),
  avatarUrl: z.string().nullable(),
  roles: z.array(AppRoleSchema)
});
export type CurrentUser = z.infer<typeof CurrentUserSchema>;

export const CurrentUserResponseSchema = z.discriminatedUnion('authenticated', [
  z.object({ authenticated: z.literal(false) }),
  z.object({
    authenticated: z.literal(true),
    user: CurrentUserSchema
  })
]);
export type CurrentUserResponse = z.infer<typeof CurrentUserResponseSchema>;

export const JoinWhitelistResponseSchema = z.object({
  success: z.boolean()
});

export type JoinWhitelistResponse = z.infer<typeof JoinWhitelistResponseSchema>;

export const AdminSystemInfoSchema = z.object({
  appName: z.string(),
  mongoDatabase: z.string()
});
export type AdminSystemInfo = z.infer<typeof AdminSystemInfoSchema>;

export type AuthStatus = 'loading' | 'authenticated' | 'unauthenticated';
