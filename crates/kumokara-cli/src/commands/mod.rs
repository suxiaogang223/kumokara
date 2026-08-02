pub mod local;
pub mod server;

use kumokara_auth::AuthManager;

fn configure_auth(require_token: bool) -> Option<AuthManager> {
    if !require_token {
        println!("→ Authentication disabled (development default)");
        return None;
    }

    let auth = AuthManager::new();
    println!("→ Token: {}", auth.server_token());
    Some(auth)
}
