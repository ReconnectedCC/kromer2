use crate::get_args;
use std::env;
use uuid::Uuid;

use crate::errors::KromerError;

pub fn make_url(uuid: Uuid) -> Result<String, KromerError> {
    let args = get_args();
    let force_insecure = env::var("FORCE_WS_INSECURE").unwrap_or("true".to_owned());
    let schema = if force_insecure == "true" || args.insecure {
        "ws"
    } else {
        "wss"
    };

    let server_url = args.url.clone().unwrap_or_else(|| {
        env::var("PUBLIC_URL")
            .unwrap_or_else(|_| env::var("SERVER_URL").unwrap_or("localhost:8080".to_owned()))
    });

    Ok(format!(
        "{schema}://{server_url}/api/krist/ws/gateway/{uuid}"
    ))
}
