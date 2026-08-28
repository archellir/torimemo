//! The HTTP surface.
//!
//! This crate depends on `core` and `embed` and never on `enrich`: the serving
//! path holds no LLM client and opens no outbound connection, which is what
//! makes a query reproducible and offline. A search here is `SQLite` plus local
//! cosine, nothing else.
//!
//! It binds to loopback by default. The intended consumer is the user's own
//! agent on the same machine, and there is no auth layer precisely because
//! there is nothing to authenticate against a socket only local processes can
//! reach — exposing this beyond loopback would need that to change first.

pub mod routes;
pub mod state;
pub mod tools;

pub use routes::router;
pub use state::AppState;
pub use tools::{ToolSpec, catalog};

use torimemo_core::{Error, Result};

/// Serves the API until interrupted.
///
/// The bind address is **derived from whether a token exists**, not
/// configured. With no live service token the archive has no access control at
/// all, so it binds `127.0.0.1` and the socket's reachability *is* the
/// control. Once a token is issued, authentication is what guards the surface
/// and binding `0.0.0.0` becomes safe — which is also the only way a container
/// can serve anything, since a process bound to a container's own loopback is
/// unreachable from outside it.
///
/// Tying the two together means the insecure configuration is unreachable by
/// construction: there is no flag that opens the port without also requiring a
/// credential, and no way to deploy this with the port open and auth off.
pub async fn serve(state: AppState, port: u16) -> Result<()> {
    let guarded = {
        let store = state
            .store
            .lock()
            .map_err(|_| Error::msg("store lock was poisoned before the first request"))?;
        store.has_tokens()?
    };

    let address = bind_address(guarded, port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| Error::with_source(format!("could not bind {address}"), error))?;

    if guarded {
        println!("listening on http://{address} (bearer token required)");
    } else {
        println!(
            "listening on http://{address} (loopback only: no service token exists, \
             so there is nothing to authenticate with — run `torimemo token issue` \
             to enable auth and bind all interfaces)"
        );
    }

    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|error| Error::with_source("server failed", error))
}

/// The address `serve` binds for a given auth state.
///
/// Extracted so the rule can be tested without opening a socket. It is the
/// security property of this crate: the port is only reachable from off-host
/// when there is a credential to demand.
#[must_use]
pub fn bind_address(guarded: bool, port: u16) -> std::net::SocketAddr {
    let host = if guarded { [0, 0, 0, 0] } else { [127, 0, 0, 1] };
    std::net::SocketAddr::from((host, port))
}

#[cfg(test)]
mod tests {
    use super::*;
    use torimemo_core::{Scope, Store};

    #[test]
    fn an_unguarded_store_binds_loopback_only() {
        let address = bind_address(false, 7645);
        assert!(address.ip().is_loopback(), "an archive with no token must not be reachable");
    }

    #[test]
    fn a_guarded_store_binds_every_interface() {
        // Required for a container to serve at all: a process bound to the
        // container's own loopback is unreachable from outside it.
        let address = bind_address(true, 7645);
        assert!(!address.ip().is_loopback());
        assert!(address.ip().is_unspecified());
    }

    #[test]
    fn issuing_a_token_is_what_opens_the_port() {
        let store = Store::open_in_memory().unwrap();
        assert!(!store.has_tokens().unwrap());
        assert!(bind_address(store.has_tokens().unwrap(), 7645).ip().is_loopback());

        store.issue_token("odin", Scope::Read).unwrap();
        assert!(!bind_address(store.has_tokens().unwrap(), 7645).ip().is_loopback());
    }

    #[test]
    fn revoking_the_last_token_closes_the_port_again() {
        let store = Store::open_in_memory().unwrap();
        let issued = store.issue_token("odin", Scope::Read).unwrap();
        store.revoke_token(&issued.id).unwrap();

        assert!(
            bind_address(store.has_tokens().unwrap(), 7645).ip().is_loopback(),
            "an archive whose last token was revoked must close the port, not serve unguarded"
        );
    }
}
