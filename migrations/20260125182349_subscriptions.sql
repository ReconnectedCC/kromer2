-- Open: Possible to subscripe to. 
-- Closed: Not possible to subscribe to, but existing contractees continue payments.
-- Cancelled: Not possible to subscribe to, and contractees make no more payments.
CREATE TYPE contract_status AS ENUM ('open', 'closed', 'canceled');

-- Contracts that (depending on status) anyone can subscribe to. Updating fields
-- such as the amount and cron_expr is not allowed. Rather, a new contract must be made.
CREATE TABLE contract_offers (
  contract_id SERIAL PRIMARY KEY,
  owner_id INT NOT NULL,
  status contract_status NOT NULL DEFAULT 'open',

  title VARCHAR(64) NOT NULL,
  description VARCHAR(255),
  cron_expr TEXT NOT NULL,
  price NUMERIC(16, 2) NOT NULL,

  max_subscribers INT,
  allow_list INT[], 

  created_at TIMESTAMPTZ NOT NULL DEFAULT now (),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now (),

  CONSTRAINT subscribers_pos check (max_subscribers > 0) ,
  CONSTRAINT amount_pos check (price > 0),
  FOREIGN KEY (owner_id) REFERENCES wallets (id)
);


CREATE TYPE subscription_status AS ENUM ('active', 'pending', 'canceled');

CREATE TABLE subscriptions (
  subscription_id SERIAL PRIMARY KEY,
  contract_id INT NOT NULL,
  wallet_id INT NOT NULL,
  status subscription_status NOT NULL DEFAULT 'active',

  lapsed_at TIMESTAMPTZ,
  started_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),

  CONSTRAINT unique_instance UNIQUE (contract_id, wallet_id),
  FOREIGN KEY (contract_id) REFERENCES contract_offers (contract_id),
  FOREIGN KEY (wallet_id) REFERENCES wallets (id)
);

CREATE INDEX contracts_wallet_idx ON contract_offers (owner_id);
CREATE INDEX subscriptions_wallet_idx ON subscriptions (wallet_id);
CREATE INDEX subscriptions_contract_idx ON subscriptions (contract_id);
