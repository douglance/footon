export interface AuthorizationIdentity {
  userId: string
  email: string
}

export function authorizationIdentity(
  session: AuthorizationIdentity | null,
  verified: AuthorizationIdentity | null,
): AuthorizationIdentity | null {
  return verified ?? session
}
