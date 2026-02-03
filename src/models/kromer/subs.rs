// British people can fight me, I *will* be writing it as "canceled"

#[derive(Debug, Clone, Copy, sqlx::Type)]
#[sqlx(type_name = "contract_status", rename_all = "lowercase")]
pub enum ContractStatus {
    Open,
    Closed,
    Canceled,
}

#[derive(Debug, Clone, Copy, sqlx::Type)]
#[sqlx(type_name = "subscription_status", rename_all = "lowercase")]
pub enum SubStatus {
    Active,
    Pending,
    Canceled,
}
