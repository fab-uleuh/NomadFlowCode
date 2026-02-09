"""Authentication middleware for API endpoints."""

from fastapi import Depends, HTTPException, status
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials

from .config import get_settings

security = HTTPBearer(auto_error=False)


async def verify_auth(
    credentials: HTTPAuthorizationCredentials = Depends(security),
) -> None:
    """Verify Bearer token authentication.

    If no secret is configured, authentication is disabled.
    If a secret is configured, the Bearer token must match.
    """
    settings = get_settings()
    if not settings.auth.secret:
        return  # Auth disabled if no secret configured
    if not credentials or credentials.credentials != settings.auth.secret:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Authentication required",
            headers={"WWW-Authenticate": "Bearer"},
        )
