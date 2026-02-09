"""FastAPI application entry point with lifespan management."""

from contextlib import asynccontextmanager

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
import uvicorn

from .config import get_settings
from .routers import repos_router, features_router, terminal_router
from .services.tmux_service import TmuxService
from .services.ttyd_service import TtydService


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Manage application lifespan - start/stop tmux and ttyd."""
    settings = get_settings()

    # Save default config if it doesn't exist
    settings.save_default_config()

    # Initialize services
    tmux_service = TmuxService(settings)
    ttyd_service = TtydService(settings)

    # Store services in app state for access in routes
    app.state.tmux_service = tmux_service
    app.state.ttyd_service = ttyd_service

    try:
        # Ensure tmux session exists
        print(f"Ensuring tmux session '{settings.tmux.session}'...")
        await tmux_service.ensure_session()
        print(f"Tmux session '{settings.tmux.session}' ready")

        # Start ttyd
        print(f"Starting ttyd on port {settings.ttyd.port}...")
        await ttyd_service.start()
        print(f"ttyd ready at ws://localhost:{settings.ttyd.port}/ws")

    except Exception as e:
        print(f"Warning: Failed to start services: {e}")
        print("API will still be available, but terminal may not work")

    print(f"\n🚀 NomadFlow API ready at http://localhost:{settings.api.port}")
    print(f"📡 Terminal WebSocket at ws://localhost:{settings.ttyd.port}/ws")
    print(f"📁 Data directory: {settings.paths.expanded_base_dir}\n")

    yield

    # Cleanup on shutdown
    print("\nShutting down...")
    await ttyd_service.stop()
    print("Goodbye!")


def create_app() -> FastAPI:
    """Create and configure the FastAPI application."""
    app = FastAPI(
        title="NomadFlow API",
        description="Git worktree and tmux session management for mobile development",
        version="0.1.0",
        lifespan=lifespan,
    )

    # Configure CORS
    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],  # Allow all origins for mobile app
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )

    # Include routers
    app.include_router(repos_router)
    app.include_router(features_router)
    app.include_router(terminal_router)

    @app.get("/")
    async def root():
        """Root endpoint with API info."""
        return {
            "name": "NomadFlow API",
            "version": "0.1.0",
            "status": "running",
        }

    @app.get("/health")
    async def health():
        """Health check endpoint."""
        settings = get_settings()
        return {
            "status": "healthy",
            "tmux_session": settings.tmux.session,
            "ttyd_port": settings.ttyd.port,
            "api_port": settings.api.port,
        }

    return app


app = create_app()


def main():
    """Run the server."""
    settings = get_settings()
    uvicorn.run(
        "nomadflow.main:app",
        host=settings.api.host,
        port=settings.api.port,
        reload=True,
    )


if __name__ == "__main__":
    main()
