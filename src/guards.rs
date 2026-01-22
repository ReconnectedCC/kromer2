use crate::get_args;
use actix_web::guard::GuardContext;
pub const INTERNAL_KEY_HEADER: &str = "Kromer-Key";

pub fn internal_key_guard(ctx: &GuardContext) -> bool {
    let args = get_args();
    let kromer_key = args.key.clone().unwrap_or_else(|| {
        std::env::var("INTERNAL_KEY")
            .expect("No INTERNAL_KEY set in .env file or command line argument (--key)")
    });
    ctx.head()
        .headers()
        .get(INTERNAL_KEY_HEADER)
        .is_some_and(|it| it.as_bytes() == kromer_key.as_bytes())
}
