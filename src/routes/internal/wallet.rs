use actix_web::{HttpResponse, get, post, web};
use rust_decimal::{Decimal, dec};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::database::player::Model as Player;
use crate::database::transaction::{Model as Transaction, TransactionCreateData, TransactionType};
use crate::database::wallet::Model as Wallet;

use crate::database::ModelExt;
use crate::errors::player::PlayerError;
use crate::errors::wallet::WalletError;
use crate::models::krist::addresses::AddressCreationResponse;
use crate::utils::crypto::generate_random_password;
use crate::{AppState, errors::KromerError};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MinecraftUser {
    pub name: String,
    pub uuid: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct GiveMoneyReq {
    pub address: String,
    pub amount: Decimal,
}

#[post("/create")]
async fn wallet_create(
    state: web::Data<AppState>,
    user: web::Json<MinecraftUser>,
) -> Result<HttpResponse, KromerError> {
    let pool = &state.pool;
    let user = user.into_inner();

    let private_key = generate_random_password();

    let mut tx = pool.begin().await?;

    // I really dont like how this is done, oh well lol.
    let player = Player::create(pool, user.uuid, user.name).await?;
    let wallet_verification_response = Wallet::verify_address(pool, &private_key).await?;

    let wallet = wallet_verification_response.model;
    let updated_wallet = wallet.set_balance(&mut *tx, dec!(100)).await?;

    let _updated_player = player
        .add_wallet_to_owned(&mut *tx, &updated_wallet)
        .await?;

    tx.commit().await?;

    let resp = AddressCreationResponse {
        private_key,
        address: updated_wallet.address,
    };

    Ok(HttpResponse::Ok().json(resp))
}

#[post("/give-money")]
async fn wallet_give_money(
    state: web::Data<AppState>,
    data: web::Json<GiveMoneyReq>,
) -> Result<HttpResponse, KromerError> {
    let pool = &state.pool;
    let data = data.into_inner();
    let amount = data.amount.round_dp(2);

    if amount < dec!(0.00) {
        return Err(KromerError::Validation("Invalid amount".into()));
    }

    let mut tx = pool.begin().await?;

    let wallet = Wallet::fetch_by_address(&mut *tx, &data.address)
        .await?
        .ok_or_else(|| KromerError::Wallet(WalletError::NotFound(data.address.clone())))?;
    let updated_wallet = wallet.update_balance(&mut *tx, amount).await?;

    let creation_data = TransactionCreateData {
        from: "serverwelf".into(),
        to: data.address,
        amount: amount,
        transaction_type: TransactionType::Mined,
        ..Default::default()
    };
    let transaction = Transaction::create_no_update(&mut *tx, creation_data).await?; // bitches.
    tracing::info!(
        "Created a transaction for welfare with ID {}",
        transaction.id
    );

    tx.commit().await?;

    Ok(HttpResponse::Ok().json(json!({
        "wallet": updated_wallet
    })))
}

#[get("/by-player/{uuid}")]
async fn wallet_get_by_uuid(
    state: web::Data<AppState>,
    uuid: web::Path<Uuid>,
) -> Result<HttpResponse, KromerError> {
    let uuid = uuid.into_inner();
    let pool = &state.pool;

    let mut tx = pool.begin().await?;

    let player = Player::fetch_by_id(&mut *tx, &uuid)
        .await?
        .ok_or_else(|| KromerError::Player(PlayerError::NotFound))?;
    let owned_wallets = player.owned_wallets(&mut *tx).await?;

    tx.commit().await?;

    // // Maybe not the best? maybe censor? idk.
    Ok(HttpResponse::Ok().json(json!({
        "wallet": owned_wallets
    })))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/wallet")
            .service(wallet_create)
            .service(wallet_give_money)
            .service(wallet_get_by_uuid),
    );
}
